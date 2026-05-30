# POS Daily-Driver Completion — Design

**Date:** 2026-05-30 · **Lane:** POS (Tauri client) · **Branch base:** `feature/erp-parity`
**Status:** approved design → implementation plan pending

## Problem

The Tauri client's POS view (`client/src/views/pos.ts`) registers sales but is
incomplete as a daily counter tool. The server already supports more than the
client uses:

- **Loyalty loop is broken at the till.** `PosSaleRequest` accepts `customer` /
  `customer_name` / `customer_phone` and the response returns
  `loyalty_points_awarded`, but the POS view never attaches a customer, so
  loyalty points never accrue from a sale. The Clientes view shows points that
  POS can never grant.
- **No receipt / boleta.** `GET /api/v1/orders/{id}/receipt` returns a complete
  `ReceiptDto` (folio, items, totals, change, loyalty, footer). The client never
  calls it — there is no ticket to show or print.
- **Slow entry.** No autofocus, no Enter-to-add, focus is lost after each add —
  bad for a keyboard/scanner-driven counter.
- **No change calc.** Cash sales don't capture amount tendered or show vuelto.

This is **client-only** work: every endpoint already exists and is merged.

## Scope

In scope (4 features), all in the POS lane:

1. **Customer attach + loyalty at till**
2. **Receipt / boleta (show + print)**
3. **Scan-fast keyboard entry**
4. **Quick-cash + change (vuelto)**

Explicitly **out of scope** (noted, deferred):

- Prescriptions-at-POS (`PosSaleRequest.prescriptions` / `PosPrescriptionInput`)
  — controlados attached at sale. Coordinate with the Recetas lane; phase 2.
- `discount` field on the sale — phase 2.
- Mixed tender (`pos_mixed`, cash+card on one sale) — server supports it; phase 2.
- **True barcode-by-code lookup** — **server gap**: `ProductDto` has no
  `barcode`/`sku` field (confirmed). Scan-fast (feature 3) is keyboard-search +
  Enter, not code lookup. Real barcode needs a server field first.
- Returns (`POST /api/v1/pos/returns`) — phase 2.

## Server contracts (confirmed, do not re-guess)

```
POST /api/v1/pos/sale
  req  PosSaleRequest { items[], payment_method, cash_amount?, card_amount?,
                        discount?, customer?, customer_name?, customer_phone?,
                        notes?, external_ref?, prescriptions[] }
  resp PosSaleResponse { order: OrderDto{ id, status, payment_method, total, ... },
                         items[], stock_movements[], prescriptions[],
                         loyalty_points_awarded: i64,
                         interaction_warnings[]?, low_stock_alerts[]? }
GET  /api/v1/orders/{id}/receipt
  resp ReceiptDto { order_id, folio_or_number, datetime, tenant_name, items[]
                    {name, qty, unit_price, line_total}, subtotal, discount,
                    total, payment_method, cash_amount?, card_amount?, change?,
                    loyalty_points_awarded, cashier?, footer_note }
GET  /api/v1/customers/search?q=   (already wrapped: customer_search → Customer[])
```

Money is always a **STRING** (`rust_decimal::serde::str`). Parse/format in the
webview via `format.ts` (`clp`, `toNumber`, `num`); never send a JS number back.

## Architecture

Three layers, matching the existing client pattern:

```
pos.ts (view)
  ├─ customerSearch()  ── existing wrapper ─┐
  ├─ posSale(+customer) ── api.ts wrapper ──┤→ #[tauri::command] in lib.rs → reqwest → server
  └─ getReceipt(id)     ── NEW api wrapper ─┘
```

### Files touched (POS lane only)

| File | Change |
|------|--------|
| `client/src-tauri/src/lib.rs` | `pos_sale`: add `customer: Option<String>` param, forward to body. NEW `get_receipt` command + `Receipt`/`ReceiptItem` structs. Register `get_receipt` in `invoke_handler!`. |
| `client/src/api.ts` | `posSale`: add `customer?` param; change return type `unknown → PosSaleResult`. NEW `getReceipt` + `Receipt`/`ReceiptItem`/`PosSaleResult`/`LowStockAlert` interfaces. |
| `client/src/views/pos.ts` | Customer picker, quick-cash+vuelto, scan-fast keyboard, post-sale receipt modal + print, loyalty in toast. |
| `client/src/styles.css` | `.receipt-modal`, `.customer-chip`, `.cash-input`, `@media print` block. |

Do **not** touch `inventory.ts` (only imports `tableSkeleton`/`asMessage`/
`escapeHtml` from it — keep using them), nor other lanes' files.

## Feature designs

### 1. Customer attach + loyalty at till

- **lib.rs** `pos_sale`: new `customer: Option<String>` arg; when present set
  `body["customer"] = customer`. (Send the record id only; the server enriches
  name from the record. `customer_name`/`phone` stay for walk-ins — not used here.)
- **api.ts** `posSale(..., customer?)` → invoke arg `customer`.
- **pos.ts**: a customer search box near checkout → debounced `customerSearch` →
  result list → pick sets `selectedCustomer = { id, name, loyalty_points }`,
  shown as a removable chip ("Cliente: Nombre · N pts"). On charge pass
  `selectedCustomer?.id`. After sale, surface `loyalty_points_awarded` in the
  toast/receipt ("+N puntos").
- **Soft-degrade:** `customer_search` rejects with `CUSTOMERS_MODULE_MISSING`
  when the server lacks the module → hide the picker with a muted note, sale
  still works without a customer. (Same sentinel the Clientes view uses.)

### 2. Receipt / boleta

- **lib.rs** NEW `get_receipt(server_url, id) -> Receipt` — GET
  `/api/v1/orders/{id}/receipt`, bearer. `Receipt`/`ReceiptItem` mirror
  `ReceiptDto`/`ReceiptItem` (money as `String`).
- **api.ts** `getReceipt(serverUrl, id): Promise<Receipt>` + interfaces. Type
  `posSale` return as `PosSaleResult { order_id, loyalty_points_awarded,
  low_stock_alerts }` (read from `response.order.id` etc. in the wrapper).
- **pos.ts** on success: `getReceipt(order_id)` → render a **boleta modal**
  (tenant, folio/número, fecha, items, subtotal/total, método, efectivo/vuelto,
  loyalty, cashier, footer). Buttons: **Imprimir** (`window.print()`), **Cerrar**,
  **Nueva venta** (clears cart + refocus search). If `getReceipt` fails, fall back
  to the current toast (sale already succeeded — never block on the ticket).
- **Print:** `@media print` hides everything except `.receipt-modal`.

### 3. Scan-fast keyboard entry

- Autofocus `#pos-search` on render and **return focus after each add**.
- **Enter** in the search adds the **top in-stock result** then clears the box.
  (Honest limit: matches name/ingredient, not a scanned barcode — see scope.)
- Charge stays reachable; cart `+/-` already keyboard-clickable.

### 4. Quick-cash + change (vuelto)

- Method = Efectivo → reveal **Monto recibido** input. Display-only
  `vuelto = recibido − total` (JS number, never sent). Pass `cash_amount =
  recibido` (string) to `posSale` (already supported). Card → `card_amount = total`.
- Optional quick chips (exact, next 1.000 / 5.000 / 10.000) to speed common cash.
- Authoritative `change` comes back on the receipt (server-computed); the
  pre-charge vuelto is a display aid only.

## Error handling

- Reuse `parseSaleError` (`"CODE|message"`): keep the `INSUFFICIENT_STOCK`
  special-case; surface `low_stock_alerts` from the response as a non-blocking
  note after a successful sale.
- Receipt fetch failure → toast fallback, never blocks the sale.
- Customer module missing → soft-degrade (above).
- Connection/HTTP errors keep the existing Spanish copy from `lib.rs`.

## Testing

- **Rust GATE:** `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace` (thin reqwest wrappers;
  no new unit tests — matches the existing command style; existing suite must pass).
- **Client GATE:** `cd client && npm run build` (tsc strict + vite must pass).
- **Manual smoke:** run `pharma-api` + `npm run tauri dev`; do a sale with a
  customer → assert loyalty toast + receipt modal with vuelto; print preview shows
  only the boleta. (No JS test harness exists in the client — do not add one here.)

## Risks / notes

- `posSale` return type changes `unknown → PosSaleResult`; the Rust side keeps
  returning the full JSON `Value`, the TS wrapper narrows it. Defensive parse
  (`response?.order?.id`) so a contract drift degrades to the toast fallback.
- Shared registry edits (`invoke_handler!` list, `api.ts` exports) are
  append-only — trivial rebase against the Recetas / Inventario lanes.
