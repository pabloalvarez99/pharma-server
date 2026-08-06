# PR6 — Operator path (RutBusiness Free Web)

Engineer on **RutBusiness** (`pharma-server`). Seam done (PR1–PR3). No desktop client
in this lane — the operator path is **API + docs**. Ship a verified operator runbook.

## Setup

```powershell
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web"; git pull
```

## Deliverable: `docs/strategy/publicar-mi-web.md` (Spanish, operator-facing, ≤80 lines)

Checklist "Publicar mi web en 15 minutos" — every step a real, tested call:

1. Marcar productos visibles: `PATCH /api/v1/products/{id}` body `{"online_visible":true}` — **verify first** with `rg -n "online_visible" crates/api/src` that catalog update accepts it (PR1 added the field; if UpdateProduct lacks it, ADD `online_visible: Option<bool>`, `online_price`, `online_title`, `online_description`, `online_sort` to `UpdateProduct` in `crates/domain/src/catalog/model.rs` + repo UPDATE — small code change, gate + commit).
2. Datos de tienda: `PUT /api/v1/settings/web.store_name` etc. (`web.whatsapp_e164`, `web.hours_label`, `web.address_line`, `web.pickup_instructions`).
3. Credencial storefront: `POST /api/v1/admin/web/keys` → guardar key+secret UNA vez.
4. Publicar: `PUT /api/v1/settings/web.published {"value":"true"}`.
5. Probar: `node scripts/web-sync/pull-catalog.mjs` + `push-order.mjs`.
6. Atender pedidos: `GET /api/v1/orders?channel=web` → `POST /api/v1/admin/orders/{id}/transition` (`preparing` → `ready_for_pickup` → cliente retira → `completed`; `cancelled` libera stock).
7. Apagar: `web.published=false` → web 404.

Each step: PowerShell `Invoke-RestMethod` one-liner with `$h = @{Authorization="Bearer $JWT"}`.
**Run the whole checklist against a local seeded server before committing** — the doc is
the test. Fix anything broken (small diffs OK, gate applies).

## Ship

```powershell
cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace   # only if code touched
git add -A; git commit -m "docs(web): PR6 runbook publicar-mi-web (verificado end-to-end)"; git push
```

Done → `✅ PR6 LISTO — runbook verificado · next: pr8-polish.md`.
