#!/usr/bin/env node
// pull-catalog.mjs — cliente del seam Free Web (ADR-0020).
//
// Consume el catálogo público integrado del pharma-server:
//   GET /api/v1/public/{slug}/store
//   GET /api/v1/public/{slug}/catalog?limit&offset   (pagina hasta next_offset null)
//
// Sin API key: las rutas de lectura son públicas cuando el tenant publicó su
// web (`web.published = "true"`). Si no está publicada, TODO responde 404
// uniforme (404-oscuridad, ADR-0005/0020) — este script lo reporta como tal.
//
// Sin dependencias externas. Sólo `node:*`. Node >= 20.
//
// Uso:
//   ERP_ORIGIN=http://127.0.0.1:8080 RB_SLUG=demo node scripts/web-sync/pull-catalog.mjs
//
// Env:
//   ERP_ORIGIN  opcional, default http://127.0.0.1:8080 (sin trailing slash).
//   RB_SLUG     requerido. Slug del tenant (segmento {slug} de la URL).
//   OUTPUT_JSON opcional, default ./catalog.json.
//
// Salida:
//   - Tabla (name, price_clp, availability) por stdout.
//   - catalog.json con { store, items, pulled_at }.
//
// Exit codes:
//   0 OK · 1 config · 2 red/HTTP (incluye 404 no-publicada) · 3 shape inesperado.
//
// NOTA: no confundir con pull-catalog-sql.mjs (seam Tu Farmacia, ADR-0012,
// emite SQL para Cloud SQL). Este script es del storefront Free Web.

import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { argv, env, exit, stderr, stdout } from "node:process";
import { pathToFileURL } from "node:url";

const PAGE_LIMIT = 100;
const HTTP_TIMEOUT_MS = 15_000;

function die(code, msg) {
  stderr.write(`[pull-catalog] ${msg}\n`);
  exit(code);
}

function log(msg) {
  stdout.write(`[pull-catalog] ${msg}\n`);
}

export function loadConfig(environment = env) {
  const origin = (environment.ERP_ORIGIN || "http://127.0.0.1:8080").replace(/\/+$/, "");
  if (!/^https?:\/\//.test(origin)) {
    die(1, `ERP_ORIGIN must start with http:// or https:// — got: ${origin}`);
  }
  const slug = (environment.RB_SLUG || "").trim();
  if (!slug) die(1, "Missing required env var: RB_SLUG");
  return {
    origin,
    slug,
    outputFile: resolve(environment.OUTPUT_JSON || "./catalog.json"),
  };
}

async function getJson(url) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), HTTP_TIMEOUT_MS);
  let res;
  try {
    res = await fetch(url, {
      headers: { Accept: "application/json", "User-Agent": "rb-web-sync/1.0" },
      signal: ctrl.signal,
    });
  } catch (err) {
    die(2, `Network error fetching ${url}: ${err?.message ?? err}`);
  } finally {
    clearTimeout(timer);
  }
  if (res.status === 404) {
    die(
      2,
      `HTTP 404 from ${url} — tenant desconocido O web no publicada ` +
        `(PUT /api/v1/settings/web.published {"value":"true"} con JWT admin).`,
    );
  }
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    die(2, `HTTP ${res.status} from ${url}\n${body.slice(0, 500)}`);
  }
  try {
    return await res.json();
  } catch (err) {
    die(3, `Response is not valid JSON: ${err?.message ?? err}`);
  }
}

// Render rows as a fixed-width table: name · price_clp · availability.
export function renderTable(items) {
  const headers = ["name", "price_clp", "availability"];
  const rows = items.map((it) => [
    String(it.name ?? ""),
    String(it.price_clp ?? ""),
    String(it.availability ?? ""),
  ]);
  const widths = headers.map((h, i) =>
    Math.max(h.length, ...rows.map((r) => r[i].length)),
  );
  const line = (cols) => cols.map((c, i) => c.padEnd(widths[i])).join("  ");
  return [line(headers), line(widths.map((w) => "-".repeat(w))), ...rows.map(line)].join(
    "\n",
  );
}

async function main() {
  const cfg = loadConfig();
  log(`Source: ${cfg.origin}/api/v1/public/${cfg.slug}`);

  const store = await getJson(`${cfg.origin}/api/v1/public/${cfg.slug}/store`);
  if (!store || typeof store.name !== "string") {
    die(3, `Unexpected /store shape: ${JSON.stringify(store).slice(0, 300)}`);
  }
  log(`Store: ${store.name} (currency=${store.currency})`);

  const items = [];
  let offset = 0;
  let page = 0;
  for (;;) {
    page += 1;
    const url = new URL(`${cfg.origin}/api/v1/public/${cfg.slug}/catalog`);
    url.searchParams.set("limit", String(PAGE_LIMIT));
    url.searchParams.set("offset", String(offset));
    const payload = await getJson(url);
    if (!Array.isArray(payload.items)) {
      die(3, `Payload.items is not an array (page ${page})`);
    }
    items.push(...payload.items);
    log(`Page ${page}: +${payload.items.length} items (total=${items.length})`);
    if (payload.next_offset === null || payload.next_offset === undefined) break;
    offset = payload.next_offset;
  }

  stdout.write(`\n${renderTable(items)}\n\n`);

  const out = { store, items, pulled_at: new Date().toISOString() };
  await writeFile(cfg.outputFile, `${JSON.stringify(out, null, 2)}\n`, "utf8");
  log(`Wrote ${items.length} items to ${cfg.outputFile}`);
}

if (argv[1] && import.meta.url === pathToFileURL(argv[1]).href) {
  await main();
}
