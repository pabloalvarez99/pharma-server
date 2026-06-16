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

## 4. Teardown

```bash
# close the window (clean exit), then:
kill %1            # pharma-api
rm -rf "$(dirname "$DATA")"
```

Each run uses a throwaway `mktemp` DB — repeatable, no lock contention with a
dev service on `./data/surreal`.
