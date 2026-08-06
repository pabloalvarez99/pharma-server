# PR5 — Storefront beachhead (RutBusiness Free Web)

Frontend-craft engineer for **RutBusiness**. The ERP (repo `pharma-server`) already
serves, per published tenant: keyless catalog reads and HMAC-protected pickup-order
writes (contracts below). Ship a **Next.js 14 App Router storefront** — ONE tenant,
ONE theme, phone-first — in a NEW folder `D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\rutbusiness-storefront`
(own git repo, `git init`, NOT inside pharma-server). Execute fully.

## Non-negotiables

- API key + HMAC secret live ONLY server-side (route handler proxy). Never in client bundle.
- Prices arrive as strings — render as CLP (`$1.290`), never parseFloat for money math; totals come from the server response.
- Spanish UI. Mobile-width checkout must work. No purple-gradient AI slop: pick a warm, trustworthy palette; visible trust row (address, hours, WhatsApp link `wa.me/<e164>`).
- Errors from ERP arrive as `{"error":{"code","message"}}` — surface `message` (already Spanish).

## Env (`.env.local`, hard-wired single tenant)

```
ERP_ORIGIN=http://127.0.0.1:8080      # or tunnel URL
RB_SLUG=demo
RB_API_KEY=rb_live_…
RB_HMAC_SECRET=whsec_…
```

## ERP contracts (exact)

```
GET  {ERP_ORIGIN}/api/v1/public/{RB_SLUG}/store
  → { name, slug, currency:"CLP", whatsapp_e164?, address_line?, hours_label?, pickup_enabled, pickup_instructions? }
GET  …/catalog?q&category&limit&offset
  → { store, items:[{ id, slug, name, description_short?, price_clp:"1290", image_url?, category_slug?, availability:"in_stock"|"low"|"out_of_stock" }], next_offset? }
GET  …/catalog/{product_slug} → one item
404 (any) = store unpublished → render "Tienda no disponible" page.

POST …/orders/web   (SERVER-SIDE ONLY, from the Next route handler)
  headers: Authorization: Bearer {RB_API_KEY} · Idempotency-Key: <uuid> ·
           X-Rb-Timestamp: <unix s> · X-Rb-Signature: hex(HMAC_SHA256(RB_HMAC_SECRET, `${ts}.POST.${path}.${sha256_hex(body)}`))  // skew ±300s
  body: { customer:{name,phone}, fulfillment:{type:"pickup",notes?}, items:[{product_id,qty}] }
  → 201 { order_id, pickup_code:"RET-XXXX", status:"reserved", currency, total:"2580", expires_at }
  errors: 422 INSUFFICIENT_STOCK / PRODUCT_NOT_AVAILABLE · 400 VALIDATION_ERROR · 401/403 auth
```

## Pages (App Router)

`/` home (store header, hero, category chips, product grid) · `/producto/[slug]` ·
`/carrito` (localStorage cart, qty steppers) · `/checkout` (name+phone+notes → POST via
`/api/order` route handler) · `/pedido/[code]` confirmation (BIG pickup_code, hours,
address, WhatsApp CTA, "paga en caja al retirar"). ISR/revalidate 60s on catalog reads;
cart+checkout client components. Out-of-stock: visible but disabled "Agotado" badge;
"low" → "¡Quedan pocas!".

## `/api/order` route handler (the only writer)

`crypto` from `node:crypto` for sha256+hmac; `randomUUID()` idempotency key stored in
sessionStorage client-side and passed through so retries replay safely; on 422 map
code → message ("Stock insuficiente…" from envelope). Never log secrets.

## Acceptance

- `npm run dev` + local seeded ERP: full phone-width flow catalog → cart → checkout →
  real ERP order (verify via `GET /api/v1/orders?channel=web` or pull-catalog script).
- `npm run build` clean. Lighthouse mobile ≥ 90 perf reasonable effort.
- README: env setup + demo flow + tunnel pointer (`pharma-server/docs/strategy/web-interop.md`).

## Ship

Own repo: `git init`, commit `feat: storefront beachhead — catálogo + retiro en tienda`.
If founder's GitHub wanted: `gh repo create pabloalvarez99/rutbusiness-storefront --private --source . --push` (ask only if gh fails).
Done → `✅ PR5 LISTO — storefront corre contra ERP local · next: pr8-polish.md`.
