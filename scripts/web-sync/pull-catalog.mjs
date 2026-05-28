#!/usr/bin/env node
// pull-catalog.mjs — Patrón A (Pull) de ADR-0012.
//
// Corre en el ENTORNO DEL WEB (no en pharma-server). Pull del endpoint público
// /api/v1/public/catalog del pharma-server, transforma cada item al shape de la
// tabla `products` del Cloud SQL del web, y emite un archivo SQL con UPSERTs
// idempotentes que el web aplica vía `psql` o equivalente.
//
// Sin dependencias externas. Sólo `node:*`. Node >= 20 (fetch + AbortController
// built-in, top-level await).
//
// Uso:
//   PHARMA_SERVER_URL=https://farmacia-acme.trycloudflare.com \
//   PHARMA_TENANT_SLUG=coquimbo-centro \
//   PHARMA_PUBLIC_READ_KEY=pk_live_xxxx \
//   OUTPUT_SQL_FILE=./out/catalog_upsert.sql \
//   node scripts/web-sync/pull-catalog.mjs
//
// Exit codes:
//   0  OK, SQL generado.
//   1  Error de configuración (env vars faltantes/inválidas).
//   2  Error de red / HTTP (con detalle en stderr).
//   3  Respuesta del server con shape inesperado.

import { writeFile, mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { argv, env, exit, stderr, stdout } from "node:process";
import { pathToFileURL } from "node:url";

const REQUIRED_ENV = ["PHARMA_SERVER_URL", "PHARMA_TENANT_SLUG", "PHARMA_PUBLIC_READ_KEY"];
const DEFAULT_OUTPUT = "./out/catalog_upsert.sql";
const PAGE_LIMIT = 200;
const HTTP_TIMEOUT_MS = 15_000;
const SCHEMA_VERSION_SUPPORTED = "1.0";

function die(code, msg) {
  stderr.write(`[pull-catalog] ${msg}\n`);
  exit(code);
}

function log(msg) {
  stdout.write(`[pull-catalog] ${msg}\n`);
}

function loadConfig() {
  const missing = REQUIRED_ENV.filter((k) => !env[k] || env[k].trim() === "");
  if (missing.length > 0) {
    die(1, `Missing required env vars: ${missing.join(", ")}`);
  }
  const baseUrl = env.PHARMA_SERVER_URL.replace(/\/+$/, "");
  if (!/^https?:\/\//.test(baseUrl)) {
    die(1, `PHARMA_SERVER_URL must start with http:// or https:// — got: ${baseUrl}`);
  }
  return {
    baseUrl,
    tenant: env.PHARMA_TENANT_SLUG.trim(),
    apiKey: env.PHARMA_PUBLIC_READ_KEY.trim(),
    outputFile: resolve(env.OUTPUT_SQL_FILE || DEFAULT_OUTPUT),
    dryRun: argv.includes("--dry-run"),
  };
}

async function fetchPage({ baseUrl, tenant, apiKey }, cursor) {
  const url = new URL(`${baseUrl}/api/v1/public/catalog`);
  url.searchParams.set("tenant", tenant);
  url.searchParams.set("limit", String(PAGE_LIMIT));
  if (cursor) url.searchParams.set("cursor", cursor);

  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), HTTP_TIMEOUT_MS);

  let res;
  try {
    res = await fetch(url, {
      headers: {
        Authorization: `Bearer ${apiKey}`,
        Accept: "application/json",
        "User-Agent": "pharma-web-sync/1.0",
      },
      signal: ctrl.signal,
    });
  } catch (err) {
    die(2, `Network error fetching ${url}: ${err?.message ?? err}`);
  } finally {
    clearTimeout(timer);
  }

  if (!res.ok) {
    const body = await res.text().catch(() => "");
    die(2, `HTTP ${res.status} from ${url}\n${body.slice(0, 500)}`);
  }

  let payload;
  try {
    payload = await res.json();
  } catch (err) {
    die(3, `Response is not valid JSON: ${err?.message ?? err}`);
  }
  return payload;
}

function validatePayload(payload) {
  if (!payload || typeof payload !== "object") {
    die(3, "Payload is not an object");
  }
  if (payload.schema_version !== SCHEMA_VERSION_SUPPORTED) {
    die(
      3,
      `Unsupported schema_version: ${payload.schema_version} ` +
        `(this script supports ${SCHEMA_VERSION_SUPPORTED})`,
    );
  }
  if (!Array.isArray(payload.items)) {
    die(3, "Payload.items is not an array");
  }
}

// SQL-escape a string literal for Postgres. Doubles single quotes; rejects NUL.
export function sqlString(value) {
  if (value === null || value === undefined) return "NULL";
  const s = String(value);
  if (s.includes("\0")) {
    die(3, "Value contains NUL byte — refusing to emit unsafe SQL");
  }
  return `'${s.replace(/'/g, "''")}'`;
}

export function sqlInt(value) {
  if (value === null || value === undefined) return "NULL";
  const n = Number(value);
  if (!Number.isFinite(n) || !Number.isInteger(n)) {
    die(3, `Expected integer, got: ${JSON.stringify(value)}`);
  }
  return String(n);
}

// Transform pharma-server catalog item -> SQL upsert row for the web's
// `products` table. Shape assumed by the web side (documented in
// docs/strategy/web-interop.md §Patrón A step 4):
//   products(sku TEXT PK, name TEXT, price_clp INT, category TEXT,
//            image_url TEXT, stock_status TEXT, source_tenant TEXT,
//            updated_at TIMESTAMPTZ DEFAULT now())
export function itemToUpsert(item, tenant) {
  const required = ["sku", "name", "price_clp"];
  for (const k of required) {
    if (item[k] === undefined || item[k] === null) {
      die(3, `Item missing required field '${k}': ${JSON.stringify(item)}`);
    }
  }
  const stockStatus = item.stock_status ?? "unknown";
  const allowedStatus = ["in_stock", "low_stock", "out_of_stock", "unknown"];
  if (!allowedStatus.includes(stockStatus)) {
    die(3, `Invalid stock_status '${stockStatus}' for sku ${item.sku}`);
  }
  return (
    `(${sqlString(item.sku)}, ${sqlString(item.name)}, ${sqlInt(item.price_clp)}, ` +
    `${sqlString(item.category ?? null)}, ${sqlString(item.image_url ?? null)}, ` +
    `${sqlString(stockStatus)}, ${sqlString(tenant)})`
  );
}

// Build the catalog upsert SQL.
//
// `items` is the original structured array from the server (used to derive the
// tombstone NOT IN list — we MUST NOT parse skus back out of the already-
// rendered `valueRows` because skus can contain commas, which breaks regex
// extraction and silently degrades the tombstone clause to a no-op).
export function buildSql(items, valueRows, tenant, generatedAt) {
  const header = [
    `-- catalog_upsert.sql`,
    `-- Generated by scripts/web-sync/pull-catalog.mjs at ${new Date().toISOString()}`,
    `-- Source tenant: ${tenant}`,
    `-- Source generated_at: ${generatedAt}`,
    `-- Rows: ${valueRows.length}`,
    `-- Apply with: psql "$DATABASE_URL" -f out/catalog_upsert.sql`,
    ``,
    `BEGIN;`,
    ``,
  ];

  if (valueRows.length === 0) {
    header.push(
      `-- No items returned from pharma-server. No-op transaction.`,
      ``,
      `COMMIT;`,
      ``,
    );
    return header.join("\n");
  }

  // Tombstone list: build directly from the typed sku field of each item.
  // skuLiterals is the list of already-SQL-escaped sku string literals.
  const skuLiterals = items.map((it) => sqlString(it.sku));

  const body = [
    `INSERT INTO products (sku, name, price_clp, category, image_url, stock_status, source_tenant) VALUES`,
    valueRows.map((r, i) => `  ${r}${i === valueRows.length - 1 ? "" : ","}`).join("\n"),
    `ON CONFLICT (sku) DO UPDATE SET`,
    `  name          = EXCLUDED.name,`,
    `  price_clp     = EXCLUDED.price_clp,`,
    `  category      = EXCLUDED.category,`,
    `  image_url     = EXCLUDED.image_url,`,
    `  stock_status  = EXCLUDED.stock_status,`,
    `  source_tenant = EXCLUDED.source_tenant,`,
    `  updated_at    = now();`,
    ``,
    `-- Tombstone any SKU that disappeared from the source tenant. Cheap soft-delete:`,
    `-- mark as out_of_stock instead of DELETE so links/SEO don't 404.`,
    `UPDATE products`,
    `   SET stock_status = 'out_of_stock', updated_at = now()`,
    ` WHERE source_tenant = ${sqlString(tenant)}`,
    `   AND sku NOT IN (`,
    `     ${skuLiterals.join(", ")}`,
    `   );`,
    ``,
    `COMMIT;`,
    ``,
  ];

  return header.concat(body).join("\n");
}

async function main() {
  const cfg = loadConfig();
  log(`Source: ${cfg.baseUrl} (tenant=${cfg.tenant})`);
  log(`Output: ${cfg.dryRun ? "(dry-run, no file)" : cfg.outputFile}`);

  const allItems = [];
  let cursor = null;
  let firstGeneratedAt = null;
  let page = 0;

  do {
    page += 1;
    const payload = await fetchPage(cfg, cursor);
    validatePayload(payload);
    if (page === 1) firstGeneratedAt = payload.generated_at ?? new Date().toISOString();
    allItems.push(...payload.items);
    cursor = payload.pagination?.next_cursor ?? null;
    log(`Page ${page}: +${payload.items.length} items (total=${allItems.length})`);
  } while (cursor);

  const rows = allItems.map((it) => itemToUpsert(it, cfg.tenant));
  const sql = buildSql(allItems, rows, cfg.tenant, firstGeneratedAt);

  if (cfg.dryRun) {
    stdout.write(sql);
    log(`Dry-run complete. ${rows.length} rows would be written.`);
    return;
  }

  await mkdir(dirname(cfg.outputFile), { recursive: true });
  await writeFile(cfg.outputFile, sql, "utf8");
  log(`Wrote ${rows.length} upserts to ${cfg.outputFile}`);
}

// Only execute main when run as the entry point. Allows the file to be
// imported by tests (or via stdin/REPL where argv[1] is undefined) without
// performing IO at import time.
if (argv[1] && import.meta.url === pathToFileURL(argv[1]).href) {
  await main();
}
