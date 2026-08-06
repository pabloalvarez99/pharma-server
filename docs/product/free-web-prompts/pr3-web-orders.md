# PR3 — Public pickup orders (RutBusiness Free Web)

You are a principal engineer on **RutBusiness** (repo `pharma-server`): Rust ERP,
offline-first. Lane already has: PR1 public catalog (`/api/v1/public/{slug}/…`,
`resolve_published_tenant`), PR2 API keys (`RequireApiKey` → `WebApiKeyCtx`,
`require_scope`, `ensure_key_matches_tenant`, key row has `hmac_secret`).
This session ships **web pickup order creation** with idempotency + HMAC + stock
reservation, plus admin transitions. Execute fully: code → tests → gates → push.

## Setup (PowerShell)

```powershell
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web"
git pull
$env:CARGO_TARGET_DIR = "D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-server\target"
```

## Laws

1. Stock truth = ERP. Never oversell: reservation inside ONE SurrealQL BEGIN/COMMIT tx.
2. Prices decimal strings; totals computed server-side from product price (ignore any client price).
3. Migrations append-only; `DEFINE FIELD` in a NEW migration file may extend an existing table (that's how status list grows).
4. Unpublished tenant ⇒ 404 even with valid key.
5. Errors: existing envelope; reuse code `INSUFFICIENT_STOCK` (already exists as `DomainError::InsufficientStock` → 422); new codes below via `ApiError::new`.
6. Gate: fmt --check + clippy -D warnings + test --workspace.

## Verified repo facts (2026-07-20)

- Next migration: expect **0020** (`ls migrations | sort | tail -3` to confirm) → `0020_web_orders.surql`.
- `order` table (mig 0007) ALREADY has: `status` ASSERT `['pending','paid','completed','reserved','cancelled','refunded']`; `payment_method` ASSERT incl. `'store','transfer'`; `customer_name`, `customer_phone`, `notes`, `external_ref`, `subtotal`, `discount`, `total`; `order_item {order, product, product_name, quantity, unit_price, subtotal}`. Missing only: channel, pickup fields, new statuses.
- `idempotency_key` table exists: `{tenant, key, response_json, status_code, expires_at}` unique (tenant,key). POS pattern (`crates/domain/src/sales/service.rs` + `crates/api/src/v1/sales.rs`): domain signals replay via `DomainError::Conflict("IDEMPOTENCY_CACHED:<json>")`; handler intercepts → 200 with cached body. **Copy that pattern**; namespace web keys as `format!("web:{raw_key}")` so POS keys never collide.
- Multi-statement tx pattern: `domain::sales::service::post_sale` (BEGIN/COMMIT, stock checks). Read it before writing `create_web_order`.
- `hmac`/`sha2`/`hex` deps added in PR2 (verify `rg "hmac" Cargo.toml`).

## Migration 0020 (exact)

```surql
-- pharma-server 0020: web orders — channel/pickup on order, stock_reserved (Free Web PR3)
DEFINE FIELD status ON order TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending','paid','completed','reserved','preparing','ready_for_pickup','cancelled','refunded'];
DEFINE FIELD channel          ON order TYPE string DEFAULT 'pos' ASSERT $value IN ['pos','web','agent'];
DEFINE FIELD pickup_code      ON order TYPE option<string>;
DEFINE FIELD fulfillment_type ON order TYPE option<string> ASSERT $value == NONE OR $value IN ['pickup','delivery'];
DEFINE FIELD expires_at       ON order TYPE option<datetime>;
DEFINE FIELD ready_at         ON order TYPE option<datetime>;
DEFINE INDEX order_tenant_channel ON order FIELDS tenant, channel, created_at;
DEFINE FIELD stock_reserved ON product TYPE int DEFAULT 0;
```

## HMAC contract (storefront → server; PR4 scripts + PR5 proxy implement client side)

```
Headers: Authorization: Bearer rb_live_…   Idempotency-Key: <uuid>
         X-Rb-Timestamp: <unix seconds>    X-Rb-Signature: <hex hmac>
canonical = "{ts}.{METHOD}.{path}.{sha256_hex(raw_body)}"   // path = "/api/v1/public/{slug}/orders/web"
sig = hex(HMAC_SHA256(hmac_secret_of_key, canonical));  reject if |now-ts| > 300s
```

Errors: bad sig → 401 `SIGNATURE_INVALID` ("Firma inválida."); skew → 401
`TIMESTAMP_SKEW` ("Marca de tiempo fuera de rango."). Verify INSIDE the handler
(body bytes needed): accept `axum::body::Bytes`, verify, then `serde_json::from_slice`.

## Route + contract

`POST /api/v1/public/{slug}/orders/web` — layer: `RequireApiKey`; then
`require_scope(&ctx, "orders:write")`, `ensure_key_matches_tenant(&ctx, &tenant)`,
`resolve_published_tenant` (404 dark when unpublished).

```json
// request
{ "customer": { "name": "Ana Pérez", "phone": "+56987654321" },
  "fulfillment": { "type": "pickup", "notes": "después de 18:00" },
  "items": [ { "product_id": "product:abc", "qty": 2 } ] }
// response 201 (WebOrderResponse — PR4/PR5 depend on these exact fields)
{ "order_id": "order:xyz", "pickup_code": "RET-7K2Q", "status": "reserved",
  "currency": "CLP", "total": "2580", "expires_at": "2026-07-21T23:59:59Z" }
```

Validation: name/phone non-empty (400 `VALIDATION_ERROR`, "Datos de cliente inválidos.");
1..=50 items, qty 1..=999.

## Service (new fns in `crates/domain/src/sales/service.rs` + model.rs DTOs)

`create_web_order(db, &tenant, idempotency_key: Option<&str>, req: WebOrderRequest) -> DomainResult<WebOrderResponse>`

Single tx: for each item — product must be `active && online_visible` (else
`DomainError::Invalid` mapped in handler to 422 `PRODUCT_NOT_AVAILABLE`
("Producto no disponible.") — use a distinguishable message marker like the
IDEMPOTENCY_CACHED pattern, or add a `DomainError` variant if cleaner);
`stock - stock_reserved >= qty` else `DomainError::InsufficientStock`;
`stock_reserved += qty`. Create order: `channel='web'`, `status='reserved'`,
`payment_method='store'`, `fulfillment_type='pickup'`, customer_name/phone, notes,
`pickup_code = "RET-" + 4 chars` from `ABCDEFGHJKMNPQRSTUVWXYZ23456789` (no 0/O/1/I/L;
derive from uuid bytes), `expires_at = now + 24h`, unit_price = `online_price ?? price`,
subtotal/total computed. Insert order_items. Idempotency: same POS pattern, key
namespaced `web:`, TTL 24h, cache 201 body.

`transition_web_order(db, &tenant, order_id, to) -> DomainResult<OrderDto>` — allowed:
reserved→preparing→ready_for_pickup→completed; any pre-completed→cancelled.
cancelled/expired ⇒ `stock_reserved -= qty` per item (floor 0). completed ⇒ release
reserve AND decrement `stock` (simple decrement + release in same tx; POS-paid
integration refined later). Invalid transition → `DomainError::Invalid`.

Admin route (JWT, role admin/owner/cashier — copy sales.rs role consts) in
`crates/api/src/v1/admin_web.rs` (exists since PR2):

```
POST /api/v1/admin/orders/{id}/transition   {"to":"preparing"|"ready_for_pickup"|"completed"|"cancelled"}
GET  /api/v1/orders?channel=web   → extend existing OrderFilters with optional channel field (crates/domain/src/sales/model.rs OrderFilters + repo WHERE)
```

Update PR1 availability: public catalog repo now computes availability from
`stock - stock_reserved`.

## Tests (`crates/api/tests/public_web_orders.rs`) — harness copy as before; helper to sign requests

1. happy path → 201, `pickup_code` matches `^RET-[A-HJ-NP-Z2-9]{4}$`, total string "2580", product.stock_reserved incremented
2. replay same Idempotency-Key → 200, identical body, NO double reservation
3. qty > stock-reserved → 422 `INSUFFICIENT_STOCK`
4. product not online_visible → 422 `PRODUCT_NOT_AVAILABLE`
5. unpublished tenant → 404 (valid key)
6. tampered body vs signature → 401 `SIGNATURE_INVALID`; stale ts → 401 `TIMESTAMP_SKEW`
7. missing scope / wrong-tenant key → 403
8. transition flow reserved→preparing→ready_for_pickup→completed releases reserve and decrements stock; cancel releases reserve
9. admin `GET /orders?channel=web` returns only web orders

## Gate + ship

```powershell
cargo fmt --all
cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
git add -A
git commit -m "feat(web): PR3 pickup orders — mig 0020, HMAC+idempotency create, stock reserve, admin transitions"
git push
```

Done → print `✅ PR3 LISTO — pushed · core seam completo · next: pr4-tooling.md o pr5-storefront.md`.
