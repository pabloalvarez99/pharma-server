#!/usr/bin/env node
// push-order.mjs — crear pedido de retiro (pickup) contra el seam Free Web
// (ADR-0020, PR3): POST /api/v1/public/{slug}/orders/web.
//
// Autenticación en capas (todas obligatorias):
//   1. Authorization: Bearer rb_live_…            (scope orders:write)
//   2. X-Rb-Timestamp: <unix secs>  (skew ±300s)
//   3. X-Rb-Signature: hex(HMAC_SHA256(secret, canonical))
//      canonical = `${ts}.POST.${path}.${sha256_hex(rawBody)}`
//      path      = /api/v1/public/{slug}/orders/web
//   4. Idempotency-Key: uuid (reintento seguro; mismo key ⇒ 200 + payload cacheado)
//
// Sin dependencias externas. Sólo `node:crypto` + fetch. Node >= 20.
//
// Uso:
//   ERP_ORIGIN=http://127.0.0.1:8080 RB_SLUG=demo \
//   RB_API_KEY=rb_live_xxx RB_HMAC_SECRET=whsec_xxx \
//   node scripts/web-sync/push-order.mjs --product product:abc --qty 2 \
//     [--name "Juana Pérez"] [--phone +56911111111] [--replay]
//
// --replay: tras el 201, re-envía el MISMO body con la MISMA Idempotency-Key
// (firma recalculada — el timestamp cambia) y muestra el 200 cacheado.
//
// Exit codes:
//   0 OK · 1 config/args · 2 red/HTTP.

import { createHash, createHmac, randomUUID } from "node:crypto";
import { argv, env, exit, stderr, stdout } from "node:process";
import { pathToFileURL } from "node:url";

const HTTP_TIMEOUT_MS = 15_000;

function die(code, msg) {
  stderr.write(`[push-order] ${msg}\n`);
  exit(code);
}

function log(msg) {
  stdout.write(`[push-order] ${msg}\n`);
}

export function parseArgs(args) {
  const out = { name: "Cliente Web", phone: "+56900000000", replay: false };
  for (let i = 0; i < args.length; i += 1) {
    const a = args[i];
    const next = () => {
      i += 1;
      if (i >= args.length) die(1, `Flag ${a} expects a value`);
      return args[i];
    };
    if (a === "--product") out.product = next();
    else if (a === "--qty") out.qty = Number(next());
    else if (a === "--name") out.name = next();
    else if (a === "--phone") out.phone = next();
    else if (a === "--replay") out.replay = true;
    else die(1, `Unknown flag: ${a}`);
  }
  if (!out.product) die(1, "Missing required flag: --product <product:id>");
  if (!Number.isInteger(out.qty) || out.qty < 1) {
    die(1, "Missing/invalid flag: --qty <n> (entero >= 1)");
  }
  return out;
}

function loadConfig() {
  const origin = (env.ERP_ORIGIN || "http://127.0.0.1:8080").replace(/\/+$/, "");
  const missing = ["RB_SLUG", "RB_API_KEY", "RB_HMAC_SECRET"].filter(
    (k) => !env[k] || env[k].trim() === "",
  );
  if (missing.length > 0) die(1, `Missing required env vars: ${missing.join(", ")}`);
  return {
    origin,
    slug: env.RB_SLUG.trim(),
    apiKey: env.RB_API_KEY.trim(),
    hmacSecret: env.RB_HMAC_SECRET.trim(),
  };
}

// Firma del contrato PR3: canonical = `${ts}.${method}.${path}.${sha256_hex(body)}`.
export function signRequest(secret, method, path, body, ts) {
  const bodyHash = createHash("sha256").update(body).digest("hex");
  const canonical = `${ts}.${method}.${path}.${bodyHash}`;
  return createHmac("sha256", secret).update(canonical).digest("hex");
}

async function postOrder(cfg, path, body, idempotencyKey) {
  const ts = Math.floor(Date.now() / 1000);
  const signature = signRequest(cfg.hmacSecret, "POST", path, body, ts);

  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), HTTP_TIMEOUT_MS);
  let res;
  try {
    res = await fetch(`${cfg.origin}${path}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        Authorization: `Bearer ${cfg.apiKey}`,
        "X-Rb-Timestamp": String(ts),
        "X-Rb-Signature": signature,
        "Idempotency-Key": idempotencyKey,
        "User-Agent": "rb-web-sync/1.0",
      },
      body,
      signal: ctrl.signal,
    });
  } catch (err) {
    die(2, `Network error POSTing ${path}: ${err?.message ?? err}`);
  } finally {
    clearTimeout(timer);
  }

  const text = await res.text().catch(() => "");
  let json = null;
  try {
    json = JSON.parse(text);
  } catch {
    // non-JSON error body: reported below
  }
  if (!res.ok) {
    die(
      2,
      `HTTP ${res.status} from ${path}: ${text.slice(0, 400)}\n` +
        `Pistas: 401 SIGNATURE_INVALID/TIMESTAMP_SKEW = clave/secreto/reloj; ` +
        `403 = scope o tenant; 404 = web no publicada; 422 = producto/stock.`,
    );
  }
  return { status: res.status, json };
}

async function main() {
  const cfg = loadConfig();
  const args = parseArgs(argv.slice(2));

  const path = `/api/v1/public/${cfg.slug}/orders/web`;
  const body = JSON.stringify({
    customer: { name: args.name, phone: args.phone },
    items: [{ product_id: args.product, qty: args.qty }],
  });
  const idempotencyKey = randomUUID();

  log(`POST ${cfg.origin}${path}`);
  log(`Idempotency-Key: ${idempotencyKey}`);

  const first = await postOrder(cfg, path, body, idempotencyKey);
  const o = first.json ?? {};
  log(
    `HTTP ${first.status} — pickup_code=${o.pickup_code} status=${o.status} ` +
      `total=${o.total} ${o.currency ?? ""} order_id=${o.order_id} expires_at=${o.expires_at}`,
  );

  if (args.replay) {
    log("Replaying same Idempotency-Key (espera HTTP 200 + payload cacheado)…");
    const second = await postOrder(cfg, path, body, idempotencyKey);
    const r = second.json ?? {};
    log(
      `HTTP ${second.status} — pickup_code=${r.pickup_code} status=${r.status} total=${r.total}`,
    );
    if (second.status !== 200 || r.order_id !== o.order_id) {
      die(2, "Replay did NOT return the cached order — idempotency broken?");
    }
    log("Idempotencia OK: mismo order_id, sin pedido duplicado.");
  }
}

if (argv[1] && import.meta.url === pathToFileURL(argv[1]).href) {
  await main();
}
