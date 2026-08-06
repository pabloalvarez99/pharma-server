# PR4 — Tooling: web-sync scripts + interop doc (RutBusiness Free Web)

Engineer on **RutBusiness** (`pharma-server`). Lane has PR1–PR3: public catalog
(`GET /api/v1/public/{slug}/store|catalog|catalog/{pslug}`, keyless) and pickup orders
(`POST /api/v1/public/{slug}/orders/web`, Bearer `rb_live_…` + Idempotency-Key + HMAC).
Ship **zero-dependency Node scripts** exercising the seam end-to-end + a 5-step tunnel
doc. Node built-ins only (`node:crypto`, `fetch`). Execute fully.

## Setup

```powershell
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web"
git pull
```

## HMAC contract (server verifies — implement exactly)

```
canonical = `${ts}.${METHOD}.${path}.${sha256_hex(rawBody)}`   // ts unix seconds; path e.g. /api/v1/public/demo/orders/web
X-Rb-Signature = hex(HMAC_SHA256(HMAC_SECRET, canonical));  X-Rb-Timestamp = ts;  skew ±300s
```

## Files

**`scripts/web-sync/pull-catalog.mjs`** — env `ERP_ORIGIN` (default `http://127.0.0.1:8080`),
`RB_SLUG`. Fetches store + full catalog (paginate `offset` until `next_offset` null),
prints table (name, price_clp, availability) + writes `catalog.json`.

**`scripts/web-sync/push-order.mjs`** — env `ERP_ORIGIN`, `RB_SLUG`, `RB_API_KEY`,
`RB_HMAC_SECRET`; args `--product <id> --qty <n> [--name --phone]`. Builds signed
request (uuid Idempotency-Key via `crypto.randomUUID()`), POSTs, prints
`pickup_code/status/total`. `--replay` flag re-sends same key to demo idempotency.

**`scripts/web-sync/README.md`** — usage, env table, demo script:
publish (`PUT /api/v1/settings/web.published {"value":"true"}` with admin JWT) →
pull-catalog → push-order → replay → admin list `GET /api/v1/orders?channel=web` →
unpublish → catalog 404.

**`docs/strategy/web-interop.md`** — Cloudflare Tunnel in 5 steps (install cloudflared,
`cloudflared tunnel create rb-<slug>`, route DNS, ingress → `http://127.0.0.1:8080`
**restricted to path prefix `/api/v1/public/`**, run as service) + troubleshooting
(404 = unpublished; 401 = key/signature; skew = clock) + threat note (admin/POS routes
never exposed; only /public through tunnel).

## Acceptance

Scripts run green against a locally seeded server (seed via existing test-style flow or
`pharma` CLI if a seed command exists — check `cargo run -p cli -- --help`). No npm deps.

## Ship

```powershell
git add -A; git commit -m "feat(web): PR4 web-sync scripts + interop doc"; git push
```

Done → `✅ PR4 LISTO — pushed · next: pr5-storefront.md`.
