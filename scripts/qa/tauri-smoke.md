# Tauri REAL desktop smoke — `pharma-client.exe`

The vitest journeys + the `pos-runtime-qa.sh` / `pos-payments-fidelidad-qa.sh`
harnesses all exercise **logic** and the **HTTP contract**. None of them open the
actual desktop window. This runbook drives the **real Tauri binary** (WebView2
window + the `invoke` IPC bridge) — the layer nobody had booted until ola 6.

## Capability on this box — CONFIRMED (2026-06-16)

The real binary **does** build and run here. Verified end-to-end:

| Check | Result |
|-------|--------|
| `npm run build` (tsc + vite) | dist built, 33 modules |
| `cargo build --release` (src-tauri) | clean, 13m39s, 0 errors |
| WebView2 runtime | present (`pv 149.0.4022.69`, HKLM) |
| launch `pharma-client.exe` | process alive, console session |
| WebView2 child procs | 8 spawned → renderer up |
| `MainWindowTitle` | `"Pharma Client"` (matches `tauri.conf.json`) |
| window render | dark login/launcher card visible (not blank) |
| `CloseMainWindow()` | exit code 0 → **no crash on close** |

So a GUI box (or this one, with an interactive desktop session) can run the full
manual cashier day. The blocker that mattered — *"does the desktop app boot at
all?"* — is answered **yes**.

> Headless caveat: launching needs an interactive Windows session (Console or
> RDP with a desktop). A pure non-interactive/service session has no display for
> WebView2 to draw into. CI is billing-walled anyway, so this stays a **local /
> manual** gate, same as the e2e harness.

## Prereqs

```bash
# server binaries (debug is fine for a smoke)
cargo build --bin pharma --bin pharma-api      # → target/debug/{pharma,pharma-api}.exe

# the desktop client
cd client && npm install && npm run build       # → client/dist
cargo build --release --manifest-path client/src-tauri/Cargo.toml
# binary lands at  <worktree>/target/release/pharma-client.exe
```

WebView2 must be installed (Win 11 ships it; else the Evergreen bootstrapper).

## 1. Boot a live backend + seed a demo pharmacy

```bash
DATA="$(mktemp -d)/surreal"
export PHARMA__DB__PATH="$DATA"
export PHARMA__BIND="127.0.0.1:8097"
export PHARMA__JWT__SECRET="tauri-smoke-secret"
export PHARMA_PASSWORD="qa-secret-123"

./target/debug/pharma.exe migrate --dir ./migrations
./target/debug/pharma.exe tenant-create "Demo" --slug demo
./target/debug/pharma.exe user-create --tenant demo --email owner@demo.local --roles owner
./target/debug/pharma.exe seed-demo --tenant demo --vertical pharmacy --force
#                                                   ^^^^^^^^ swap to `minimarket` for the other vertical

# CLI must exit (releases the SurrealKv file lock) BEFORE the server boots —
# server + CLI cannot hold ./data/surreal at the same time.
./target/debug/pharma-api.exe &        # serves on 127.0.0.1:8097
curl -fsS http://127.0.0.1:8097/health/ready    # → {"status":"ok","checks":{"db":"ok"}}
```

## 2. Launch the desktop client

```bash
./target/release/pharma-client.exe
# or, for hot-reload dev (spawns vite + a debug build):
cd client && npm run tauri dev
```

The window opens on the **login** screen. The server URL field defaults are in
`client/src/views/login.ts`; point it at `http://127.0.0.1:8097`.

## 3. Manual cashier day (the real journey — keyboard-first)

Run it as a pharmacist would, **keyboard only** where the POS allows (perf budget
is <100 ms add-item):

1. **Login** — tenant `demo`, `owner@demo.local`, `qa-secret-123`.
2. **Caja → abrir** — register name + opening cash (e.g. `50000`).
3. **POS → venta**
   - scan/manual add a seeded SKU (pharmacy seed = 12 products), watch add-item
     latency feel instant;
   - **multi-tender**: split efectivo + tarjeta (cabled in #210) — confirm the
     change (`vuelto`) is right;
   - apply a **descuento** (línea + global) — total never goes negative;
   - **Cobrar** → boleta/ticket renders.
4. **Devolución** — pick the order just sold, partial refund + restock, confirm
   stock returns.
5. **Caja → arqueo** — expected vs counted; enter a count, see the discrepancy.
6. **Caja → cerrar** — close summary persists.

### What to hunt (things vitest cannot see)

- render glitches / broken layout / clipped controls;
- an `invoke()` that throws — IPC arg-name mismatch (JS camelCase ↔ Rust
  snake_case), or a serde shape drift between client wire-types and `crates/api`;
- keyboard focus / tab order in the POS;
- real add-item lag on a 12-SKU cart;
- stale UI after a mutation (sale/refund not refreshing stock or totals);
- crash on window close.

Log anything found in `teamwork_op.txt` → BUG LOG. Any pharmacy-only assumption
that leaks into minimarket → MULTI-RUBRO FINDINGS (recetas / controlados /
principio-activo must stay hidden when `business.vertical=minimarket`).

## 3b. Surface deep-dives (ola 6b — devoluciones · caja · clientes)

§3 walks the happy day. These three drive the cashier-owned surfaces to their
edges in the **real window**, where a mutation has to repaint correctly and the
`invoke` IPC has to round-trip the exact serde shape. Each subsection lists the
manual steps, what must be **visible on screen** after the action (the stale-UI
trap vitest mocks away), and the edges to push. Run every one in **both**
verticals where the seed allows.

> IPC seam note (verified static, ola 6b): `api.ts` sends camelCase keys
> (`registerName`, `openingCash`, `closingCashCounted`, `metodoReembolso`); Tauri
> v2 maps them to the snake_case Rust params in `src-tauri/src/lib.rs`
> (`register_name`, `opening_cash`, `closing_cash_counted`, `metodo_reembolso`).
> Money is a STRING end-to-end (`Decimal`); the client re-emits it verbatim and
> only parses to `Number` for display. If a field arrives as a JS number the
> `clp()`/`toNumber()` path still renders, but a refund/close POST must send the
> string — watch the network tab if a total renders as `NaN`.

### 3b.1 Devoluciones (`devoluciones.ts` → `get_receipt` + `create_refund`)

1. Sell something first (§3 step 3) and note the **order id** from the ticket.
2. **Devoluciones → + Nueva devolución**. Type the order id, **Cargar boleta**
   (or Enter in the field).
   - ON SCREEN: the modal must swap the skeleton for one row per sold line, each
     showing `vendido N · $precio c/u` and a qty input capped at the sold qty.
3. **Parcial**: return *some* units on *one* line.
   - ON SCREEN: the **Tipo** badge stays `Parcial` (pill-warn) the instant you
     edit a qty — it is derived live from the inputs, never picked by hand.
   - Enter a **Motivo** (required), pick a **Método de reembolso**, **Confirmar**.
   - After save: the recent-devoluciones table repaints with the new row —
     fecha, orden (short), the motivo badge, método, and `Total` in CLP.
4. **Total**: load the same/another boleta, set **every** line to its full sold
   qty.
   - ON SCREEN: the badge flips to `Total` (pill-danger) — only when *all* lines
     are returned in full.
5. Restock reality check: the **"Reingresar al stock"** toggle is **disabled**
   with the note that the boleta carries no product id — refunds record money +
   flip the order to `refunded` but do **not** restock. Confirm the note is
   visible and the box cannot be checked. (Restock is done via Inventario →
   Ajustar stock — out of this surface's scope, by design.)
6. EDGES to push in the window:
   - bad/unknown order id → **Cargar boleta** shows a Spanish error in the items
     pane, the **Confirmar** button stays disabled (no blank 500 toast);
   - empty motivo → inline "El motivo es obligatorio.", no POST fired;
   - all-zero qtys → `validateRefund` blocks with a Spanish reason;
   - the wire `tipo` is the MOTIVO axis (`venta` default), **not** total/parcial
     — sending the scope here was BUG-paul-001 (every return 500'd). The
     Total/Parcial badge is a pre-submit hint ONLY. If a return 500s again,
     this is the first thing to check in the network tab.
7. MULTI-RUBRO: nothing pharmacy-specific here — devoluciones is universal. The
   minimarket seed must refund identically (no recetas/controlados gate).

### 3b.2 Caja (`caja.ts` → `cash_sessions` · `open_cash_session` · `cash_arqueo` · `close_cash_session`)

1. Fresh tenant → **Caja** shows the **"Sin caja abierta"** empty state with the
   pulse "Abrir caja" CTA (not a blank grid).
2. **Abrir caja**: name (`caja-1` default) + opening cash (e.g. `50000`).
   - ON SCREEN: toast "Caja abierta"; the body repaints to one **caja-card**
     with register name, "Abierta" badge, monto inicial in CLP, apertura
     datetime, optional notes, and a "Cerrar caja" button.
3. **Movimientos** that feed the arqueo (drive them from the sibling surfaces so
   the expected-cash math has something to reconcile):
   - a **cash sale** in POS (efectivo) → raises `cash_sales`;
   - a **gasto** paid cash against the open session (Gastos surface, marvin's,
     but it lands in this arqueo) → raises `movements_out` (BUG-marvin-003 fix
     in #211 makes the retiro atomic — verify the egreso actually lowers the
     expected).
4. **Multi-caja**: hit **"Abrir otra caja"** → a second card. The server allows
   N open registers/tenant; each card's "Cerrar caja" is scoped to its own id.
5. **Cerrar caja → arqueo**: the close modal first fetches the arqueo and shows
   the breakdown — Apertura, Ventas efectivo, Ingresos, −Egresos, **Esperado en
   caja** (= opening + cash_sales + movements_in − movements_out).
6. Enter **Efectivo contado** and watch the **discrepancia** recompute live:
   - counted == expected → `cuadró exacto` (neutral);
   - counted  > expected → `sobrante $X`;
   - counted  < expected → `faltante $X`.
   Test all three. **Cerrar caja** → toast carries the discrepancy verdict and
   the card disappears (or the count drops) on repaint.
7. EDGES: empty/negative counted blocked with a Spanish error, confirm disabled
   until a value is typed; arqueo fetch failure shows an inline error but the
   counted field still works (degrades, doesn't trap).
8. MULTI-RUBRO: caja is universal — identical in minimarket.

### 3b.3 Clientes + fidelidad (`clientes.ts` → `customer_search` · `customer_detail` · `customer_history` · `create_customer`)

> Degrades gracefully: if the server lacks the `customers-loyalty` surface, the
> Tauri commands reject with `CUSTOMERS_MODULE_MISSING` and the view shows a
> friendly "módulo requiere merge de customers-loyalty" note instead of a hard
> error. Confirm in the window which branch you're on before chasing a "bug".

1. **Clientes → + Nuevo cliente**: name (required), RUT, teléfono, email.
   - RUT field has a live mod-11 advisory (`attachRutAdvisory`) — type an invalid
     RUT and confirm the hint warns but does not hard-block; the canonical RUT is
     what gets sent.
   - Save → the search box is set to the new name and re-runs, so the customer
     appears in **Resultados** with `0 pts`.
2. **Search** by name / RUT / phone (debounced 240 ms). Click a result → it goes
   **active** and the **Detalle** panel paints: Puntos, Total comprado (CLP),
   Visitas, contact line, and Historial de compras.
3. **Fidelidad accrual** (the headline check): with this customer selected in
   **POS**, ring a sale attributed to them, then come back to Clientes, re-search,
   open the detail.
   - ON SCREEN: **Puntos** must have gone up, **Total comprado** must include the
     new sale, **Visitas** +1, and the sale must appear as a new **Historial**
     row (fecha, pago label, ítems, total, estado "Pagada").
   - This is the stale-UI trap: the detail panel must reflect the mutation after
     a re-open (the view reloads detail on selection, it does not live-subscribe).
4. **Editar**: the detail's Editar button carries the fields as data-attrs
   (survives detail re-render). PATCH only changed fields; on save the detail
   panel reloads for that id.
5. EDGES: search with no hits → "Sin clientes para «q»."; empty name on
   create → inline "El nombre es obligatorio.", no POST; history fetch failure is
   swallowed to an empty list (secondary), but a module-missing sentinel still
   surfaces the friendly note.
6. MULTI-RUBRO: clientes + fidelidad is universal — a minimarket keeps customers
   and points the same way; nothing pharmacy-specific in this surface.

## 3c. Live HTTP contract probe (the layer the IPC forwards to)

The `invoke` commands in `src-tauri/src/lib.rs` are thin: auth + `reqwest` to the
same `crates/api` endpoints. Hitting those endpoints directly with the smoke
backed up (§1) confirms the **serde shapes** the desktop commands deserialize
into — a drift here is exactly the `invoke() throws` failure §3's "what to hunt"
warns about. `scripts/qa/tauri-contract-probe.sh` runs the cashier-owned subset
(devolución, caja open→arqueo→close, cliente create→search→detail) over the live
server and diffs the response keys against the structs in `lib.rs`. Run it after
§1, before driving the window, to fail fast on a shape mismatch.

## 4. Teardown

```bash
# close the window (clean exit), then:
kill %1            # pharma-api
rm -rf "$(dirname "$DATA")"
```

Each run uses a throwaway `mktemp` DB — repeatable, no lock contention with a
dev service on `./data/surreal`.
