# AGENT BRIEFING — Free Web @ Shopify Grade (RutBusiness)

> **SUPERSEDED FOR EXECUTION (2026-07-20).** Use the token-lean prompt pack:
> [`free-web-prompts/README.md`](./free-web-prompts/README.md) — one standalone file
> per PR for fresh sessions, ground truth verified. Corrections vs this doc: next free
> ADR = **0016** (not 0018) · keys PR now BEFORE orders PR (no dev-token hack) · error
> code = existing `INSUFFICIENT_STOCK` · public reads keyless via `/public/{slug}/…`
> 404-darkness · `order` already has `reserved`/`store`/`transfer`/customer fields.
> This file stays as context appendix only.

**Paste this whole file as message 1.** It is optimized for **cold start → first green PR** with minimal thrash.

```
READ ORDER (strict):  §A START → §B MEMORY → §C BOOT → §D BUILD QUEUE
                      Then §E–§G only as needed while coding.
SKIP nothing in §A–§D. Do not build themes before PR1–PR3 seam.
```

---

# A. START HERE (first 5 minutes)

## A.1 You are

**Principal PE** shipping RutBusiness **Free Web**: Shopify-grade face + ERP stock truth + pickup orders.  
Not a theme farm. Not a Shopify clone. Not a strategist who never merges code.

## A.2 Mission (one line)

> Free storefront so good a Chilean SME picks it **over** Shopify — because catalog/orders/stock live in the **same offline ERP** they already run free.

## A.3 Goals when conflicting (highest wins)

`G0 offline ERP` > `G1 free real commerce` > `G2 craft` > `G3 single brain ERP` > `G4 15-min publish` > `G5 paid=growth` > `G6 one rubro deep`

## A.4 v1 wedge (only happy path that matters)

```
catalog → cart → PICKUP → name+phone → RET-XXXX → ERP queue → ready → POS pay
```

No shipping / no Webpay / no multi-theme portfolio until seam + one theme work.

## A.5 Hard laws (fail = wrong product)

| # | Law |
|---|---|
| 1 | Public web **opt-in**; POS works offline forever (ADR-0005) |
| 2 | Free web is **ungated** (no 402 on catalog/order basic) |
| 3 | Stock + money truth = **ERP**; web projects/intakes only |
| 4 | API keys **never** in browser; HMAC+Idempotency on writes |
| 5 | `web.published=false` ⇒ public **404** darkness |
| 6 | Prices = **decimal strings**; **never** leak `cost_price` |
| 7 | Migrations **append-only**; multi-tenant on new tables |
| 8 | Prove existence with `rg` — ADR “exists” ≠ this branch |

## A.6 Non-goals (do not start)

Shopify apps · MSI public host · monorepo DSS merge · shared DB · GraphQL · Free crippleware · 8 mediocre themes · LLM store operator.

## A.7 Done for the whole mission (checkbox north star)

- [ ] curl: publish → catalog → order → admin sees `channel=web` + pickup code  
- [ ] stock policy respected (no silent oversell)  
- [ ] storefront beachhead craft (mobile) OR seam+docs if UI deferred by user  
- [ ] Free generous; paid keys only for growth  
- [ ] ADR + strategy gap doc + demo script  
- [ ] `fmt` + `clippy -D warnings` + `test` green  

---

# B. WORKING MEMORY CARD (keep loaded)

| Item | Value |
|---|---|
| Product | **RutBusiness** (repo name `pharma-server` is historical) |
| Workspace | `rutbusiness/` → `pharma-server/` (main), `pharma-license-server/` (paid keys only) |
| Branch | `feature/free-web-shopify-parity` |
| ERP stack | Rust 1.85 · axum 0.8 · SurrealDB kv-surrealkv · rust_decimal strings · JWT ops auth |
| Storefront stack | Next 14 App Router · Vercel · Zod · server-side order proxy · CF Tunnel to ERP |
| Seam | `GET /api/v1/public/*` + `POST .../orders/web` · API key scopes · tunnel WAN |
| Free vs paid | Free = 1 site + catalog + pickup orders; Paid = domain, deep brand, card, multi-site, agent |
| Persona | Sandra: 1 local, WhatsApp CRM, pickup > courier, 15 min or fail |
| Snapshot truth | **No `public*` routes** · migs through **0017** · **no `client/`** · **no `scripts/web-sync`** |
| Next mig | **`0018_web_storefront.surql`** (re-check max before write) |
| Wire-in | `crates/api/src/v1/mod.rs` → `.merge(public_web::router(...))` |
| Patterns to copy | `v1/sales.rs` (Idempotency-Key) · `v1/catalog.rs` (router split) · `error.rs` envelope · `0007_sales.surql` ASSERTs |
| Supersede | ADR-0014 freemium “web=paid” → new ADR Free Web as Core; **keep** HTTP seam |

**Architecture:**

```
Phone browser → Next storefront (edge) --API key+HMAC--> CF Tunnel → pharma-server :LAN
Cashier ← ERP admin/POS (JWT) ← same DB (stock/orders)
```

---

# C. BOOT SEQUENCE (run before any design essay)

## C.1 Shell (copy-paste)

```powershell
cd "D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-server"
git status -sb
git branch --show-current
git log -3 --oneline
# ground truth
rg -n "public/catalog|orders/web|order_channel|online_visible|web_api_key|stock_reserved" crates migrations
Get-ChildItem migrations | Sort-Object Name | Select-Object -Last 8 Name
# orientation (read these files, don't guess)
# crates/api/src/v1/mod.rs
# crates/api/src/error.rs
# crates/api/src/v1/sales.rs   (idempotency)
# crates/api/src/v1/catalog.rs (router style)
# crates/domain/src/sales/model.rs
# migrations/0007_sales.surql
# migrations/0003_catalog.surql
```

## C.2 Branch

```powershell
git checkout -b feature/free-web-shopify-parity  # if not already
```

## C.3 Turn-1 output ONLY (template — fill, then stop unless told “execute”)

Emit this structure (≤60 lines total). No storefront code yet.

```markdown
## P0 Gap Report
- Branch / HEAD:
- public/* exists?: yes/no (paths)
- Next mig number:
- order fields usable today:
- product fields missing for online:
- client/ present?: yes/no
- Recommended stock policy: S1 reserved (default) | S0 reject-only (temp)
- Risks (top 3):
- PR1 file list (concrete):
- Open questions for founder (0–2 max):
```

Then start **PR1** automatically if user said execute/implement; else wait.

---

# D. BUILD QUEUE (ultra-plan — execute in order)

Do **one PR at a time**. Each row = shippable. Stop when gates red.

## D.0 Queue overview

| PR | Name | Outcome | Depends |
|---|---|---|---|
| **0** | Docs lock | gap + ADR draft + this plan ack | — |
| **1** | Public catalog | read path + online flags + 404 darkness | 0 |
| **2** | Public order | pickup create + idempotency + stock | 1 |
| **3** | Keys + auth | API keys, scopes, rate limit, HMAC | 2 (HMAC can land with 2) |
| **4** | Tooling | `scripts/web-sync` + `web-interop.md` | 3 |
| **5** | Storefront | Next beachhead pickup UI | 3 |
| **6** | Operator UX | settings + order queue (API-only OK if no client) | 2–3 |
| **7** | License matrix | paid `web.*` keys only | docs |
| **8** | Polish | SEO, demo script, craft pass | 5 |

**Efficiency rule:** PR1–3 = 80% of value. PR5 without PR3 = demo debt.

---

## PR0 — Docs (30–60 min)

**Create/update:**

| File | Content |
|---|---|
| `docs/strategy/free-web-shopify-parity.md` | Gap table + Free/Paid + metrics (short) |
| `docs/adr/0018-free-web-as-core.md` *(or next free ADR #)* | Supersede 0014 freemium clause; keep seam |
| optional patch | freemium-master-plan web rows |

**AC:** founder can approve policy without reading code.

---

## PR1 — Public catalog (first code PR)

### Intent
Storefront can pull a **safe** catalog when published.

### Files to add/touch (canonical)

```
migrations/0018_web_storefront.surql     # or split: 0018 products online fields only
crates/domain/src/catalog/model.rs       # PublicProductDto / online fields
crates/domain/src/catalog/service.rs     # list_public_catalog(tenant, …)
crates/domain/src/catalog/repo.rs        # query online_visible
crates/api/src/v1/public_web.rs          # NEW handlers
crates/api/src/middleware/public_auth.rs # stub: accept later full keys; PR1 may use temporary header if sequenced—prefer ship keys in PR3 with catalog behind "published + key"
crates/api/src/v1/mod.rs                 # merge router
crates/api/tests/public_web_catalog.rs   # NEW
```

**Recommended sequencing for speed:** implement **published flag + online_visible filter + public DTOs** in PR1; if keys not ready, use dev-only `X-Dev-Public-Token` **behind `cfg(test)` / env `RB_PUBLIC_DEV_TOKEN`** — remove before release. Prefer finishing PR3 same day.

### Schema (product) — add fields

```surql
DEFINE FIELD online_visible ON product TYPE bool DEFAULT false;
DEFINE FIELD online_title ON product TYPE option<string>;
DEFINE FIELD online_description ON product TYPE option<string>;
DEFINE FIELD online_sort ON product TYPE int DEFAULT 0;
DEFINE FIELD online_price ON product TYPE option<decimal>;
DEFINE INDEX product_tenant_online ON product FIELDS tenant, online_visible, active, online_sort;
```

### Settings (admin_setting keys)

`web.published` (`"true"`/`"false"`), `web.slug`, `web.store_name`, `web.whatsapp_e164`, `web.hours_label`, `web.address_line`, `web.pickup_instructions`

### Routes

```
GET /api/v1/public/store
GET /api/v1/public/catalog?limit&cursor&q&category
GET /api/v1/public/catalog/{slug}
```

### AC (must pass)

- [ ] unpublished → 404 on all three  
- [ ] only `active && online_visible`  
- [ ] response has **no** `cost_price` / cost fields  
- [ ] `price_clp` is **string**  
- [ ] pharmacy: exclude controlled / unsafe prescription types (use existing controlled helpers if any)  
- [ ] tenant isolation test  

### Gates

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p api --test public_web_catalog
# or cargo test --workspace
```

---

## PR2 — Public web order (pickup)

### Intent
Customer places pickup order; ERP stores it; stock not lied about.

### Files

```
migrations/0019_web_orders.surql         # if not bundled in 0018
crates/domain/src/sales/model.rs         # channel, pickup_code, …
crates/domain/src/sales/service.rs       # create_web_order tx
crates/api/src/v1/public_web.rs          # POST handler
crates/api/tests/public_web_orders.rs
```

### Order fields (extend ASSERT carefully — read 0007 first)

```
channel: pos|web|… (default pos)
pickup_code: option string  # RET-XXXX
fulfillment_type: pickup|delivery
expires_at, ready_at
# status: add preparing, ready_for_pickup if needed via new ASSERT list
# payment_method unpaid pickup: store | transfer
```

### Stock policy **S1 (default)**

```
available = stock - stock_reserved
on create: if qty>available → STOCK_INSUFFICIENT else stock_reserved += qty
on cancel/expire: stock_reserved -= qty
on complete/POS: release reserve + decrement stock (or integrate post_sale path later)
```

Add `stock_reserved` on product (default 0) in mig.  
Expiry: job later OK; for PR2 set `expires_at` + manual cancel endpoint admin.

### Route

```
POST /api/v1/public/orders/web
Headers: Authorization Bearer rb_live_… | Idempotency-Key | X-Rb-Timestamp | X-Rb-Signature
```

### Idempotency

Copy sales POS pattern; **namespace** key with `web_order:` prefix or separate purpose field so POS keys never collide. TTL 24h.

### HMAC

```
canonical = `${ts}.{METHOD}.{path}.{sha256_hex(body)}`
sig = hex(HMAC_SHA256(secret, canonical)); skew ±300s
```

### AC

- [ ] happy path → `reserved` + `pickup_code` + total string  
- [ ] replay same Idempotency-Key → same order id / body  
- [ ] insufficient stock → `STOCK_INSUFFICIENT` ES message  
- [ ] offline product / not online_visible → `PRODUCT_NOT_AVAILABLE`  
- [ ] unpublished → 404  
- [ ] invalid signature → `SIGNATURE_INVALID`  

---

## PR3 — API keys + middleware (iron door)

### Files

```
migrations/00xx_web_api_key.surql        # if not in 0018
crates/domain/... or api local repo for keys
crates/api/src/middleware/public_auth.rs
crates/api/src/v1/admin_web.rs           # settings + keys CRUD
crates/api/tests/public_web_auth.rs
```

### `web_api_key` table

`tenant, name, key_prefix, key_hash, scopes[], active, created_at, last_used_at`  
Plaintext `rb_live_…` **once** on create; store argon2id/blake3 hash.

### Scopes

`store:read` · `catalog:read` · `orders:write`

### Admin JWT routes

```
GET|PATCH /api/v1/admin/web/settings
POST /api/v1/admin/web/keys
POST /api/v1/admin/web/keys/{id}/rotate
DELETE /api/v1/admin/web/keys/{id}
GET /api/v1/admin/orders?channel=web   # or filter existing list
POST /api/v1/admin/orders/{id}/transition  {to: preparing|ready_for_pickup|cancelled|completed}
```

### AC

- [ ] wrong key → 401  
- [ ] missing scope → 403/SCOPE_DENIED  
- [ ] rotate invalidates old (or dual-key grace documented)  
- [ ] rate limit doesn’t brick localhost tests (test cfg bypass OK)  

---

## PR4 — Tooling (fast leverage)

```
scripts/web-sync/pull-catalog.mjs
scripts/web-sync/push-order.mjs
scripts/web-sync/README.md
docs/strategy/web-interop.md    # tunnel 5 steps + troubleshooting
```

Zero-dep Node preferred. Demo script section in README = §F.

---

## PR5 — Storefront (only after PR3 green)

**Stack:** Next 14 App Router · server route proxies order · ISR catalog 30–60s · localStorage cart · one theme (`pharmacy` or `minimarket`).

**Routes:** `/{slug}` home · catalog · product · cart · checkout · order confirmation.

**Craft fail conditions:** AI slop purple gradient · no trust row · broken mobile checkout · English-only errors.

**AC:** phone-width flow creates real ERP order via tunnel or local origin env.

**Beachhead efficiency:** hard-wire one tenant env (`ERP_ORIGIN`, `ERP_API_KEY`, `ERP_HMAC`) before multi-tenant host router.

---

## PR6 — Operator path

If `client/` missing: **admin API + OpenAPI + short UI in static admin if any** is enough.  
If client appears later: wizard “Publicar mi web” checklist (products → publish → copy URL → test order).

---

## PR7 — License (do not block P1–3)

`pharma-license-server/src/lib/feature-catalog.ts` append **paid only**:

`web.custom_domain` · `web.branding_advanced` · `web.payments_online` · `web.marketing_automation`

Never gate Free catalog/order on license.

---

## PR8 — Polish

sitemap/robots/json-ld · demo §F recorded · craft pass · bitácora milestone.

---

# E. CONTRACTS (copy-paste normative)

## E.1 Error envelope (existing)

```json
{ "error": { "code": "STOCK_INSUFFICIENT", "message": "No hay stock suficiente.", "details": {} } }
```

Codes: `WEB_DISABLED`(map to 404), `INVALID_API_KEY`, `SCOPE_DENIED`, `SIGNATURE_INVALID`, `TIMESTAMP_SKEW`, `STOCK_INSUFFICIENT`, `PRODUCT_NOT_AVAILABLE`, `VALIDATION_ERROR`, `IDEMPOTENCY_CONFLICT`.

## E.2 Catalog response shape

```json
{
  "store": {
    "name": "Farmacia Demo",
    "slug": "farmacia-demo",
    "vertical": "pharmacy",
    "currency": "CLP",
    "whatsapp_e164": "+56912345678",
    "address_line": "Av. Ejemplo 100, Coquimbo",
    "hours_label": "Lun–Sáb 9:00–20:00",
    "pickup_enabled": true,
    "pickup_instructions": "Retira en mesón con tu código"
  },
  "items": [
    {
      "id": "product:abc",
      "slug": "paracetamol-500",
      "name": "Paracetamol 500mg 16 comp",
      "description_short": "Analgésico",
      "price_clp": "1290",
      "image_url": "https://cdn.example/p.jpg",
      "category_slug": "analgesicos",
      "availability": "in_stock",
      "stock_badge": "available"
    }
  ],
  "next_cursor": null
}
```

## E.3 Create order

```json
// request
{
  "customer": { "name": "Ana Pérez", "phone": "+56987654321", "email": null, "rut": null },
  "fulfillment": { "type": "pickup", "notes": "después de 18:00" },
  "items": [{ "product_id": "product:abc", "qty": 2 }],
  "client_meta": { "storefront_version": "1.0.0" }
}
// response
{
  "order_id": "order:xyz",
  "pickup_code": "RET-7K2Q",
  "status": "reserved",
  "currency": "CLP",
  "total": "2580",
  "expires_at": "2026-07-20T23:59:59Z"
}
```

## E.4 Availability projection

```
available <= 0 → out_of_stock
available <= low_threshold (default 5) → low
else → in_stock
```

Prefer enum in cached HTML over raw qty.

---

# F. DEMO SCRIPT (PR3+ acceptance)

```powershell
# Assume API at http://127.0.0.1:8080 and ADMIN_JWT, API_KEY, HMAC set
$h = @{ Authorization = "Bearer $ADMIN_JWT" }
# 1 publish
Invoke-RestMethod -Method PATCH -Uri http://127.0.0.1:8080/api/v1/admin/web/settings -Headers $h -ContentType application/json -Body '{"published":true}'
# 2 catalog
Invoke-RestMethod -Uri http://127.0.0.1:8080/api/v1/public/catalog -Headers @{ Authorization = "Bearer $API_KEY" }
# 3 order (add HMAC headers in real script)
# 4 replay idempotency
# 5 admin list channel=web
# 6 PATCH published false → catalog 404
```

Node scripts in PR4 should automate HMAC.

---

# G. CONTEXT APPENDIX (read on demand only)

## G.1 Why this product bet

Sandra’s JTBD: online face without a second brain; WhatsApp chaos → structured pickup; Free ERP already has stock/POS — web is projection.  
Moat: Shopify is web-first; RutBusiness is **local-first free** + pro web free.  
Freemium: Free = breathe; Paid = grow (domain, brand, card, multi-site, agent).

## G.2 Doc map

| Path | Use |
|---|---|
| `CLAUDE.md` | Project law |
| `docs/adr/0005-*.md` | Free invariants |
| `docs/adr/0014-*.md` | Seam (not freemium clause) |
| `docs/strategy/freemium-master-plan.md` | Tiers |
| `docs/strategy/rubro-catalog.md` | Verticals |
| `pharma-license-server/src/lib/feature-catalog.ts` | Entitlements |

## G.3 Stack rejects

MSI public site · shared DB · float money · keys in JS bundle · Redis required · GraphQL day-1.

## G.4 Threat cheatsheet

| Threat | Fix |
|---|---|
| Key leak | hash + rotate |
| Replay | HMAC + skew + idempotency |
| Oversell | tx + stock_reserved |
| WAN admin exposure | tunnel path limits + keys; public router only |

## G.5 Status report (after each PR)

```
## Hito PR#
- Paths:
- Demo:
- Tests:
- G0 offline ok? G1 free generous?
- Next:
```

## G.6 Decision tree (2 seconds)

```
Helps proud catalog (T2) or order-in-ERP (T3)?
  no → drop
  yes → breaks offline/leak/cripple Free? → redesign : ship smallest tested slice
```

## G.7 Mantras

Offline > cosplay · Free generous · One brain ERP · Pickup-first · Keys server-side · Decimal strings · 404 when off · Prove with rg+tests.

---

# H. DEFAULT ACTION AFTER PASTE

```
1) Run §C.1 boot
2) Fill §C.3 P0 Gap Report
3) Implement PR1 file list (or PR0 docs first if ADR missing — max 1 commit docs then PR1)
4) Stop at red gates; fix before PR2
5) Do not open Next app until PR3 AC green (unless user overrides)
```

**Efficiency target:** working `GET /public/catalog` + tests **same session** as briefing paste; orders **same or next session**; UI after iron seam.
