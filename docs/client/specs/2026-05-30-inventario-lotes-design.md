# Inventario writes + Lotes/Vencimientos — client design

**Lane**: INVENTARIO (s3). **Scope**: CLIENT-ONLY (Tauri 2 + vanilla TS). Server
contracts already merged on `feature/erp-parity`. Date: 2026-05-30.
Branch: `feat/client-inventario-lotes`.

## Problem

`client/src/views/inventory.ts` is read-only (KPIs + product table). The nav
labels it "Stock y lotes" but shows neither stock writes nor lotes. Expiry
management is money saved + legal (caducados). Server supports all of it.

## Goal

Turn Inventario operable + deliver lotes/vencimientos:
1. **Ajustar stock** per product — `POST /products/{id}/stock` (`StockAdjust`:
   `set|delta` + `reason`). Returns `ProductDto`.
2. **Crear producto** — `POST /products` (`NewProduct`: name+price req, rest opt).
3. **Lotes** per product — `GET /batches?product=` + `POST /batches`
   (`NewBatch`: product, batch_code, expiry_date RFC3339, stock, cost?, notes?).
4. **Próximos a vencer** — `GET /reports/near-expiry?days=N` → rows with
   `days_to_expiry` + `expired` flag. Day window 30/60/90.

**YAGNI / explicitly deferred**: faltas, bulk-price, categorías CRUD, ABC,
stock-movements history, reorder, product edit/delete, CSV import/export UI.

## Contracts (read from source, not guessed)

- `StockAdjust { set: Option<i64>, delta: Option<i64>, reason: Option<String> }`
- `NewProduct { name, price(STRING dec), cost_price?(STRING), stock=0, category?,
  laboratory?, active_ingredient?, prescription_type?, presentation?, ... }`
- `ProductDto` — full; money (`price`,`cost_price`) are STRINGS.
- `BatchDto { id, product, batch_code, expiry_date(DateTime), stock, cost?(STRING),
  notes?, active, created_at, updated_at }`
- `NewBatch { product, batch_code, expiry_date(DateTime<Utc>), stock=0, cost?, notes? }`
- `GET /batches` query: `product?, only_available?, expiring_within_days?, limit?, offset?`
- `NearExpiryRow { product_id, product_name, batch_id, batch_code, expiry_date,
  stock, days_to_expiry(neg=expired), expired }`; query `days?` (default 30).

**Money = STRING** (`rust_decimal::serde::str`) — never f64. **expiry_date** is
RFC3339; `<input type=date>` → `${value}T00:00:00Z`. **Writes require admin+**
(server `admin_plus`) → client surfaces server 403 "Permiso denegado…" verbatim
(no role threading; renderInventory keeps `(host, serverUrl)` signature).

## Files (own scope only)

1. `client/src-tauri/src/lib.rs` — +6 commands (`create_product`,
   `product_detail`, `adjust_product_stock`, `list_batches`, `create_batch`,
   `near_expiry`) + DTO structs (`ProductDetail`, `Batch`, `NearExpiryRow`).
   Append to `invoke_handler!` (append-only registry — trivial rebase vs POS/Recetas).
2. `client/src/api.ts` — append typed wrappers + interfaces.
3. `client/src/views/inventory.ts` — tabs + product-detail modal + forms +
   near-expiry panel. **Preserve exports** `tableSkeleton/asMessage/escapeHtml`
   (+ `kpiCard/kpiSkeleton/errorCard`) — pos.ts/recetas.ts import them.

## UX

Tab bar: **Productos** | **Próximos a vencer** (scoped `<style>` injected in
inventory.ts; reuse caja.ts modal/field/btn/pill/toast classes — no styles.css edit).

- **Productos**: KPI cards + search (existing) + `+ Nuevo producto`. Rows become
  clickable → **product-detail modal**: detail + "Ajustar stock" inline form
  (Fijar/Sumar mode + reason) + **Lotes** sub-section (list + "Agregar lote" form).
- **Próximos a vencer**: 30/60/90 día chips → near-expiry table (Producto, Lote,
  Vence, Stock, Días, Estado). Caducado=danger, ≤30d=warn. Empty state.

## Build sequence

lib.rs (commands compile) → api.ts (wrappers) → inventory.ts (UI) → GATE
(`cargo fmt --check` + `clippy -D warnings` + `cargo test --workspace` +
`cd client && npm run build`) → commit + push + PR vs `feature/erp-parity`.

## Success criteria

- New product, adjust stock (set+delta), add lote, list lotes per product all
  round-trip to the server and refresh the view.
- Near-expiry panel filters by window + flags caducados.
- Non-admin gets the Spanish 403 message, not a crash.
- GATE green; client `npm run build` green; helper export signatures unchanged.
