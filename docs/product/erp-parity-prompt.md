# PROMPT: Llevar pharma-server al nivel de ERP de Tu Farmacia

> Pega este archivo entero como primer mensaje en una sesión Claude Code abierta en `C:\Users\Administrator\Documents\GitHub\pharma-server`. Trabajar en branch `feature/erp-parity` (crearla si no existe).

---

## 0. Contexto y objetivo

**Quién eres**: Senior dev trabajando en `pharma-server` — servidor Rust on-prem, single MSI, axum + SurrealDB embedded (kv-surrealkv), multi-tenant, offline-first, target Windows. Producto vendible a farmacias chilenas.

**Estado actual** (verificar antes de empezar con `git log`, `ls crates/`):
- Branch base: `feature/pharma-server-scaffold` (o `main` si ya merged).
- Crates: `core` (config + Error + TenantId), `db` (SurrealDB + migrations runner), `api` (axum bin `pharma-api`), `auth` (JWT HS256 + argon2id), `cli` (bin `pharma`: `migrate/config/tenant-create/user-create`), `service` (Windows service), `jobs` (cron vacío), `telemetry`.
- Migración única: `migrations/0001_init.surql` → tablas `tenant`, `user`, `session`.
- API rutas reales: `POST /api/login`, `GET /api/me`, `/health/*`, `/metrics`, `/openapi.json`, swagger. Nada de dominio aún.
- Pre-commit obligatorio (igual que CI con `-D warnings`):
  ```powershell
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

**Objetivo de esta tarea**: alcanzar paridad funcional de **API + dominio ERP** con la app live de Tu Farmacia (`C:\Users\Administrator\Documents\GitHub\build-and-deploy-webdev-asap\pharmacy-ecommerce\apps\web`). NO portar el frontend Next.js — `pharma-server` expone API HTTP/JSON versionada (`/api/v1/...`) y el frontend va en repo aparte después. Sí servir Swagger UI completo y un mini admin embebido HTML opcional (Fase 8).

**Restricciones duras del producto** (NO violar):
1. Offline-first, LAN-only. Sin dependencias cloud obligatorias.
2. Multi-tenant: TODA tabla de dominio lleva `tenant: record<tenant>` + índice compuesto que incluya `tenant`. Filtrado por JWT claim `tenant_id` en handler. Sin excepciones.
3. Migraciones append-only (`migrations/NNNN_descripcion.surql`). NUNCA editar migraciones aplicadas.
4. Errores user-facing en español (códigos internos en inglés OK).
5. POS endpoints budget: <50ms p99 en i3+SSD+8GB.
6. Performance: no romper compat DB sin migración automática que preserve datos.
7. SemVer estricto en `Cargo.toml workspace.package.version`.
8. Pre-commit (fmt + clippy `-D warnings` + tests) DEBE pasar antes de cada commit. CI igual.
9. CLI-first. Si falta una CLI → `cargo install` o `choco install` y seguir.
10. Bitácora dual obligatoria por hito: `bitacora.md` repo + `C:/Users/Administrator/Documents/obsidian-mind/work/active/pharma-server/bitacora.md` + actualizar `decisions-log-index.md`.

---

## 1. Inventario de paridad (Tu Farmacia → pharma-server)

### 1.1 Modelos Prisma a portar (31 tablas Postgres → tablas SurrealDB)

Origen: `pharmacy-ecommerce/apps/web/prisma/schema.prisma` (503 líneas, leer entero antes de modelar).

Lista exhaustiva (NO inventar — son los `model X {}` reales):

| # | Prisma model | SurrealDB table | Notas |
|---|---|---|---|
| 1 | `profiles` | `customer` | Cliente final farmacia. `loyalty_points`, `rut`, `phone`, `role`. |
| 2 | `push_subscriptions` | `push_subscription` | WebPush. **Opcional Fase 9** (offline-first → push remoto no es core). |
| 3 | `categories` | `category` | Catálogo. `slug` UNIQUE por tenant. |
| 4 | `products` | `product` | Núcleo. Campos: `name, slug, price, cost_price, stock, category, image_url, active, external_id, laboratory, therapeutic_action, active_ingredient, prescription_type, presentation, discount_percent`. |
| 5 | `barcode_catalog` | `barcode_catalog` | Mapping global `external_id → barcode`. **Sin tenant** (catálogo Chile compartido — confirmar regla, ver §4). |
| 6 | `product_barcodes` | `product_barcode` | N:1 con product. `barcode` UNIQUE por tenant. |
| 7 | `orders` | `order` | Venta. `status, total, payment_provider, cash_amount, card_amount, pickup_code, tracking_token, sold_by_user_id`. Quitar Mercado Pago/Stripe (irrelevante on-prem; conservar campos `transbank_*` opcionales). |
| 8 | `order_items` | `order_item` | Líneas. |
| 9 | `therapeutic_category_mapping` | `therapeutic_category_mapping` | Tabla puente lookup. |
| 10 | `admin_settings` | `setting` | Key/value tenant-scoped. |
| 11 | `stock_movements` | `stock_movement` | Auditoría stock. `delta, reason, admin`. |
| 12 | `suppliers` | `supplier` | Proveedor. `rut, contact_*, default_invoice_format`. |
| 13 | `purchase_orders` | `purchase_order` | OC. `invoice_number, invoice_date, status, total_cost, subtotal_net, tax_amount, paid, due_date, payment_method_ap`. |
| 14 | `purchase_order_items` | `purchase_order_item` | Líneas OC con `batch_code, expiry_date`. |
| 15 | `supplier_product_mappings` | `supplier_product_mapping` | `supplier_code → product`. |
| 16 | `loyalty_transactions` | `loyalty_transaction` | Movimientos puntos. |
| 17 | `faltas` | `falta` | Productos faltantes a reponer. |
| 18 | `product_batches` | `product_batch` | Lotes con vencimiento. **Core**: trazabilidad ISP/DEIS. |
| 19 | `supplier_price_lists` | `supplier_price_list` | Histórico precios por proveedor. |
| 20 | `caja_cierres` | `cash_close` | Arqueo + cierre día. |
| 21 | `devoluciones` | `return_doc` | Devoluciones. |
| 22 | `devolucion_items` | `return_item` | Líneas devolución. |
| 23 | `audit_log` | `audit_log` | Log inmutable. **Append-only enforce a nivel app**. |
| 24 | `prescription_records` | `prescription` | Recetas retenidas. Ley 20.000. |
| 25 | `pharmacist_shifts` | `pharmacist_shift` | Turno químico farmacéutico. |
| 26 | `purchase_payments` | `purchase_payment` | Pagos a OC (AP). |
| 27 | `gasto_categories` | `expense_category` | Categorías gastos operativos. |
| 28 | `gastos` | `expense` | Gastos. |
| 29 | `recurring_expenses` | `recurring_expense` | Gastos recurrentes (alquiler, luz). |
| 30 | `internal_tasks` | `internal_task` | Tareas equipo. |
| 31 | `announcements` | `announcement` | Avisos internos. |

**Reglas de naming SurrealDB**: singular snake_case (`product`, no `products`). `id` autogenerado por Surreal. Referencias como `record<X>`. Timestamps `created_at` / `updated_at` con `DEFAULT time::now()` y `VALUE time::now()` para updated.

**Reglas de tenancy**: cada tabla excepto `tenant`, `user`, `session`, `barcode_catalog` (catálogo global Chile) lleva `tenant: record<tenant>`. Crear índice compuesto `(tenant, <campo natural>)`.

### 1.2 Endpoints API a portar (87 rutas Next.js → axum)

Origen real: `pharmacy-ecommerce/apps/web/src/app/api/admin/**/route.ts`.

Lista completa de rutas admin (verificadas con `find ... -name route.ts`):

```
actividad
arqueo
audit
avisos                    avisos/[id]
batches                   batches/[id]
catalogo                  catalogo/[id]
catalog-quality
categories                categories/[id]
cierre-dia                cierre-dia/email
clientes                  clientes/[id]
costos
dashboard-extras
descuentos
devoluciones              devoluciones/[id]
ejecutivo
equipo
etiquetas/search
faltas                    faltas/[id]
farmacia                  farmacia/liquidacion
finanzas/ap               finanzas/ap/[id]/pay
finanzas/cash-flow
finanzas/dashboard
finanzas/gastos           finanzas/gastos/[id]
finanzas/gastos/recurring finanzas/gastos/recurring/[id]
finanzas/pyl
insights
inventory                 inventory/abc
inventory/reorder-suggestions
loyalty                   loyalty/stats
operaciones
orders                    orders/[id]                orders/export
pos/customer-history      pos/pickup                  pos/sale
prescriptions
products                  products/[id]               products/[id]/stock
products/bulk-price       products/export             products/import
products/stats            products/update-prices
purchase-orders           purchase-orders/[id]
purchase-orders/[id]/items
purchase-orders/[id]/map-product
purchase-orders/[id]/receive
purchase-orders/[id]/suggest-matches
purchase-orders/kpis
purchase-orders/monthly-margin
purchase-orders/monthly-summary
purchase-orders/scan
reportes                  reportes/compras
reportes/fidelidad        reportes/libro-ventas
reposicion                reposicion/express
scan-invoice              settings
stock-movements           stock-movements/adjust    stock-movements/import
supplier-prices           supplier-prices/compare    supplier-prices/import
suppliers                 suppliers/[id]
tareas                    tareas/[id]
turnos                    turnos-farmaceutico
users                     users/[uid]                 users/invite
vendedor
```

**Mapeo a axum**: prefijo `/api/v1/`. `[id]` → `:id` axum path param. Métodos HTTP: leer cada `route.ts` de Tu Farmacia para extraer `GET/POST/PATCH/DELETE` exportados.

**Endpoints NO portar** (web/cliente final): `auth/*`, `categories/*`, `loyalty/*` (cliente), `orders/*` (cliente checkout), `products/*` (cliente), `profile/*`, `push/*`, `search/*`, `store-pickup/*`, `tracking/*`, `webpay/*`. Algunos sí (search, products público) van detrás de tenant también pero scope vendor-public.

### 1.3 UI admin (referencia, NO portar)

Lista de páginas `src/app/admin/*` (50+ módulos) sirve como índice de **features** que deben quedar funcionalmente cubiertas por API:

```
actividad arqueo avisos catalogo catalogo-calidad categorias cierre-dia clientes
compras configuracion costos dashboard descuentos devoluciones ejecutivo equipo
etiquetas faltas farmacia fidelidad finanzas insights inventario libro-recetas
operaciones ordenes pos productos proveedores push reportes reposicion sistema
stock tareas turnos turnos-farmaceutico usuarios vencimientos vendedor
```

Cada página = 1+ endpoints. Si un endpoint no existe pero la página lo necesita → crearlo.

---

## 2. Arquitectura propuesta (decidir y commitear en `brain/pharma-server-decisions.md`)

### 2.1 Nuevo crate `domain`

Crear `crates/domain` con submodules por bounded context:

```
crates/domain/src/
  lib.rs
  catalog/     (product, category, barcode, batch)
  inventory/   (stock_movement, falta, abc, reorder)
  sales/       (order, order_item, pos, return)
  purchasing/  (supplier, purchase_order, purchase_item, payment, price_list)
  finance/     (expense, recurring_expense, expense_category, cash_close, ap, pyl)
  customers/   (customer, loyalty_transaction)
  prescriptions/  (prescription, pharmacist_shift)
  operations/  (internal_task, announcement, audit_log)
  settings/    (setting, therapeutic_category_mapping)
  reports/     (insights, monthly_margin, libro_ventas, fidelidad)
```

Cada submodule:
- `model.rs` — tipos Rust `#[derive(Serialize, Deserialize, ToSchema)]` para utoipa.
- `repo.rs` — funciones `async fn` puras sobre `&Db` (sin axum).
- `service.rs` — lógica de negocio (cálculo margen, asignación lote FEFO, etc.).
- `errors.rs` — variantes de `DomainError` con `thiserror`.

`api` crate solo orquesta: extrae claims, llama service, mapea error → status.

### 2.2 Convenciones API

- Prefijo `/api/v1`. Versionar por path desde día 1. Migración `v2` cuando rompamos contrato.
- Auth: middleware `RequireAuth` (ya existe vía `AuthUser` extractor) + middleware nuevo `RequireRole(&["admin"|"cashier"|"pharmacist"|"owner"])`.
- Errores: envelope estándar
  ```json
  { "error": { "code": "INSUFFICIENT_STOCK", "message": "Stock insuficiente para SKU X", "details": {...} } }
  ```
  HTTP 4xx/5xx. Códigos en SCREAMING_SNAKE, mensaje en español.
- Paginación: cursor-based. Query `?limit=N&cursor=<opaque>`. Respuesta `{ items, next_cursor }`.
- Filtros: query strings tipadas con `serde` + `axum::extract::Query<T>`.
- Mutaciones idempotentes: header `Idempotency-Key` (especialmente POS sale, payment). Tabla `idempotency_key` con TTL 24h.
- Auditoría: middleware `AuditLayer` que inserta en `audit_log` para todo `POST/PATCH/DELETE`. Campos: `tenant, user, method, path, payload_hash, ip, user_agent, ts`.

### 2.3 Roles y permisos

Reusar `user.roles: array<string>`. Set canónico: `owner`, `admin`, `pharmacist`, `cashier`, `viewer`. Mapeo:

| Módulo | owner | admin | pharmacist | cashier | viewer |
|---|---|---|---|---|---|
| catalog read | ✅ | ✅ | ✅ | ✅ | ✅ |
| catalog write | ✅ | ✅ | – | – | – |
| pos sale | ✅ | ✅ | ✅ | ✅ | – |
| prescriptions | ✅ | ✅ | ✅ | – | – |
| finance | ✅ | ✅ | – | – | – |
| reports | ✅ | ✅ | ✅(propios) | – | ✅ |
| users mgmt | ✅ | ✅ | – | – | – |
| settings | ✅ | – | – | – | – |

Implementar como `fn role_required(path: &str, method: Method) -> &'static [&'static str]` o (mejor) atributo por handler. Considerar `axum::middleware::from_fn_with_state`.

### 2.4 Cron jobs (crate `jobs`)

Reactivar `tokio-cron-scheduler` con jobs:
- **Vencimientos**: diario 06:00 → marca lotes `expires_in < 90d`.
- **Stock mínimo**: cada hora → crea `falta` si stock < reorder_point.
- **Cierre día auto**: 23:55 → snapshot ventas día, deja `cash_close` draft.
- **Backup SurrealKv**: diario 03:00 → snapshot a `data/backups/YYYY-MM-DD.snap` con retención 30 días.
- **Recurring expenses**: cuando `next_run <= today` → instancia `expense` y avanza `next_run`.

Cada job: `tracing::info_span!`, métricas Prometheus, lock por tenant (no correr 2× simultáneo mismo tenant).

### 2.5 OpenAPI / Swagger

Cada handler `#[utoipa::path(...)]`. Cada modelo `#[derive(ToSchema)]`. Mantener `/openapi.json` y `/swagger-ui` siempre verdes en CI (test: deserializar el spec).

---

## 3. Plan de ejecución por fases

> Cada fase = 1 PR. Cada PR pasa `cargo fmt + clippy -D warnings + test --workspace`. Cada PR actualiza bitácora dual.

### Fase 1 — Foundation (estimado: 1 día)
1. Crear branch `feature/erp-parity`.
2. Crear crate `domain` con scaffolding de submodules (lib.rs `pub mod`).
3. Agregar `crates/api/src/v1/mod.rs` y mover `/api/me`+`/api/login` actuales a `/api/v1/me`+`/api/v1/login` (mantener alias en `/api/*` por compat durante 1 release).
4. Middleware `RequireRole`.
5. Error envelope estándar en `core::Error` + `IntoResponse`.
6. Layer `AuditLayer` (insert async, no-block respuesta).
7. Migración `0002_audit_log.surql` con tabla `audit_log` + índice `(tenant, created_at)`.
8. Tests: golden test del error envelope, RequireRole acepta/rechaza.

### Fase 2 — Catalog (productos, categorías, códigos de barra) (1.5 días)
Migración `0003_catalog.surql`: `product`, `category`, `product_barcode`, `barcode_catalog`, `therapeutic_category_mapping`.

Endpoints:
- `GET/POST /api/v1/products` (filtros: search, category, active, low_stock)
- `GET/PATCH/DELETE /api/v1/products/:id`
- `POST /api/v1/products/:id/stock` (ajuste manual)
- `POST /api/v1/products/import` (CSV multipart)
- `GET /api/v1/products/export` (CSV stream)
- `POST /api/v1/products/bulk-price` (cambio masivo % o $)
- `POST /api/v1/products/update-prices` (desde supplier_price_list)
- `GET /api/v1/products/stats` (count, low_stock, expired, value)
- `GET/POST /api/v1/categories`, `GET/PATCH/DELETE /api/v1/categories/:id`
- `GET /api/v1/etiquetas/search` (autocomplete laboratorio/principio activo)

Lógica:
- Slug auto-generado tenant-scoped UNIQUE.
- `barcode_catalog`: tabla global (catálogo Chile compartido). Lookup `barcode → external_id` no requiere tenant.

### Fase 3 — Inventory + Stock + Vencimientos (1.5 días)
Migración `0004_inventory.surql`: `stock_movement`, `product_batch`, `falta`.

- `GET/POST /api/v1/stock-movements`, `POST /api/v1/stock-movements/adjust`, `POST /api/v1/stock-movements/import`
- `GET/POST /api/v1/batches`, `GET/PATCH/DELETE /api/v1/batches/:id`
- `GET/POST /api/v1/faltas`, `PATCH /api/v1/faltas/:id`
- `GET /api/v1/inventory` (resumen)
- `GET /api/v1/inventory/abc` (clasificación A/B/C 80/15/5 ventas 90d)
- `GET /api/v1/inventory/reorder-suggestions` (algoritmo: avg_daily_sales * lead_time + safety_stock − stock_actual)

Lógica clave: **stock = SUM(stock_movement.delta)** materializado en `product.stock`. Mantener consistencia vía transacción al insertar movimiento. Lote FEFO (First Expires First Out) al consumir stock en POS.

### Fase 4 — Sales + POS + Devoluciones (2 días)
Migración `0005_sales.surql`: `order`, `order_item`, `return_doc`, `return_item`, `pharmacist_shift` (parcial).

- `POST /api/v1/pos/sale` — endpoint crítico, budget <50ms p99. Idempotencia obligatoria. Transacción Surreal: insertar `order` + `order_item[]` + `stock_movement[]` (delta negativo, FEFO desde `product_batch`) + `loyalty_transaction` opcional.
- `GET /api/v1/pos/customer-history?rut=X` (últimas 20 compras)
- `POST /api/v1/pos/pickup` (entrega orden web pre-pagada)
- `GET /api/v1/orders`, `GET /api/v1/orders/:id`, `GET /api/v1/orders/export`
- `POST /api/v1/devoluciones`, `GET/PATCH /api/v1/devoluciones/:id`
- `GET/POST /api/v1/descuentos` (descuentos configurables)

Lógica:
- Cálculo total: `sum(items.qty * items.price) − descuentos`. IVA 19% Chile (configurable por setting).
- Pago split cash/card: `cash_amount + card_amount = total`.
- Stock decrement bloqueado si `stock < qty` → 422 `INSUFFICIENT_STOCK`.

### Fase 5 — Purchasing (proveedores, OC, recepción) (1.5 días)
Migración `0006_purchasing.surql`: `supplier`, `purchase_order`, `purchase_order_item`, `supplier_product_mapping`, `supplier_price_list`, `purchase_payment`.

- `GET/POST /api/v1/suppliers`, `GET/PATCH/DELETE /api/v1/suppliers/:id`
- `GET/POST /api/v1/purchase-orders`, `GET/PATCH/DELETE /api/v1/purchase-orders/:id`
- `GET/POST /api/v1/purchase-orders/:id/items`
- `POST /api/v1/purchase-orders/:id/receive` (genera stock_movement + product_batch desde line items con expiry_date)
- `POST /api/v1/purchase-orders/:id/map-product` (asocia supplier_code → product)
- `POST /api/v1/purchase-orders/:id/suggest-matches` (fuzzy match nombre invoice → product)
- `GET /api/v1/purchase-orders/kpis`
- `GET /api/v1/purchase-orders/monthly-margin`
- `GET /api/v1/purchase-orders/monthly-summary`
- `POST /api/v1/purchase-orders/scan` (sin OCR todavía — devolver `not_implemented` o usar stub `scan-invoice` endpoint)
- `GET/POST /api/v1/supplier-prices`, `POST /api/v1/supplier-prices/compare`, `POST /api/v1/supplier-prices/import`
- `GET /api/v1/finanzas/ap`, `POST /api/v1/finanzas/ap/:id/pay`

Lógica:
- Costo promedio ponderado al recibir: `new_cost = (stock*cost + qty*unit_cost) / (stock + qty)`.
- Estados OC: `draft → confirmed → received → paid → closed`. Cancel posible antes de received.

### Fase 6 — Finance + Reports (1.5 días)
Migración `0007_finance.surql`: `expense_category`, `expense`, `recurring_expense`, `cash_close`.

- `GET/POST /api/v1/finanzas/gastos`, `GET/PATCH/DELETE /api/v1/finanzas/gastos/:id`
- `GET/POST /api/v1/finanzas/gastos/recurring`, `GET/PATCH/DELETE /api/v1/finanzas/gastos/recurring/:id`
- `GET /api/v1/finanzas/dashboard`
- `GET /api/v1/finanzas/cash-flow`
- `GET /api/v1/finanzas/pyl` (P&L mes/año)
- `GET/POST /api/v1/arqueo`, `POST /api/v1/cierre-dia`, `POST /api/v1/cierre-dia/email`
- `GET /api/v1/reportes`, `GET /api/v1/reportes/compras`, `GET /api/v1/reportes/libro-ventas`, `GET /api/v1/reportes/fidelidad`
- `GET /api/v1/insights` (dashboard ejecutivo)
- `GET /api/v1/ejecutivo`, `GET /api/v1/dashboard-extras`
- `GET /api/v1/costos`
- `GET /api/v1/farmacia`, `POST /api/v1/farmacia/liquidacion`

Lógica:
- P&L: ingresos (orders confirmed) − COGS (sum order_items.qty * product.cost_price snapshot) − expenses periodo.
- Libro ventas: agrupado por día, exportable CSV (formato SII si aplica).

### Fase 7 — Customers + Loyalty + Prescriptions + Ops (1.5 días)
Migración `0008_ops.surql`: `customer`, `loyalty_transaction`, `prescription`, `pharmacist_shift`, `internal_task`, `announcement`, `setting`, `idempotency_key`.

- `GET/POST /api/v1/clientes`, `GET/PATCH/DELETE /api/v1/clientes/:id`
- `GET /api/v1/loyalty`, `GET /api/v1/loyalty/stats`
- `GET/POST /api/v1/prescriptions` (controlados Ley 20.000: log inmutable nombre+RUT+receta médico+fecha)
- `GET/POST /api/v1/turnos`, `GET/POST /api/v1/turnos-farmaceutico`
- `GET/POST /api/v1/tareas`, `GET/PATCH/DELETE /api/v1/tareas/:id`
- `GET/POST /api/v1/avisos`, `GET/PATCH/DELETE /api/v1/avisos/:id`
- `GET/POST /api/v1/equipo` (gestión empleados)
- `GET/POST /api/v1/vendedor` (atribución vendedor por venta)
- `GET/POST /api/v1/operaciones`
- `GET/POST /api/v1/actividad` (timeline auditoría legible)
- `GET/POST /api/v1/settings` (key-value tenant-scoped)
- `GET /api/v1/audit` (consulta audit_log paginado)
- `GET/POST /api/v1/users`, `GET/PATCH/DELETE /api/v1/users/:uid`, `POST /api/v1/users/invite`
- `POST /api/v1/reposicion`, `POST /api/v1/reposicion/express`
- `GET /api/v1/catalogo`, `GET /api/v1/catalogo/:id` (vista catálogo público pre-publicar)
- `GET /api/v1/catalog-quality` (productos con campos faltantes)
- `GET /api/v1/libro-recetas`

### Fase 8 — Cron jobs + Backup + Mini admin embebido (1 día)
- Implementar 5 jobs §2.4.
- `pharma backup create/list/restore` CLI.
- `/app` embebido: HTML+HTMX simple servido desde `crates/api/src/embedded_ui.rs` con `include_str!`. Solo para validación in-situ del técnico instalador. NO sustituye frontend real.
- Swagger UI ya existe — verificar 100% endpoints documentados.

### Fase 9 — Hardening + Performance + MSI (1 día)
- Benchmarks `criterion`: `pos_sale` <50ms, `products_list` <100ms (10k SKUs).
- Smoke tests instalación: `cargo wix --no-build && msiexec /i pharma-server-X.Y.Z.msi /quiet` → service running → `curl http://localhost:8080/health/ready` → `msiexec /x`.
- Doc usuario final: `installer/README-CLIENTE.md` (instalación, primer login, troubleshooting).
- Tag `v0.2.0` + release GH.

**Total estimado**: ~12 días de trabajo focalizado.

---

## 4. Decisiones que requieren tu juicio (resolver al inicio, documentar)

Cada una → 1 entry en `brain/pharma-server-decisions.md` con fecha, decisión, alternativas, razón.

1. **¿`barcode_catalog` global o por tenant?** Recomendación: global (catálogo Chile compartido entre instalaciones). Implica feature flag para sync futuro desde server central.
2. **¿Decimal numbers?** SurrealDB tiene `decimal`. Tu Farmacia usa `Decimal(10,2)`. Usar `surrealdb::sql::Number` o crate `rust_decimal` en Rust + serializar como string en JSON. Decidir y mantener consistencia.
3. **¿Multi-currency?** Tu Farmacia = CLP solo. Hardcode CLP, dejar campo `currency: string` en `order` para forward-compat. NO implementar conversión.
4. **¿IVA y boleta electrónica SII?** Fase 9 saca v0.2.0 SIN integración SII real. Solo cálculo IVA local. Stub endpoint `POST /api/v1/sii/emit` que devuelva 501 `NOT_IMPLEMENTED`. Bloqueante producto pero no MVP.
5. **¿OCR factura (scan-invoice, purchase-orders/scan)?** Tu Farmacia probablemente usa Tesseract/Gemini Vision. Opciones on-prem: `ocrs-cli` Rust nativo, Tesseract via subprocess. Recomendación: stub 501 + plugin futuro.
6. **¿Idempotency-Key store?** Tabla `idempotency_key (tenant, key, response_hash, body, status, expires_at)` con TTL 24h. Job de limpieza horario.
7. **¿FEFO vs FIFO?** Lotes consumidos por `expiry_date ASC` (FEFO) — estándar farmacia. Si dos lotes mismo expiry, FIFO por `created_at`.
8. **¿Snapshot/backup format?** SurrealKv soporta `EXPORT`. Backup = export `.surql` comprimido gzip. Restore = `IMPORT`. Probar round-trip.
9. **¿Realtime push a frontend?** SurrealDB `LIVE SELECT` disponible. Defer: HTTP polling suficiente Fase 1-8. Live queries en Fase 10+ cuando exista frontend.
10. **¿Tests integración cómo?** `cargo test` con `surrealdb` arrancado en memoria (`Surreal::new::<surrealdb::engine::local::Mem>(())`). Cada test crea tenant aislado. Apoyar con `testcontainers` solo si Mem no alcanza.

---

## 5. Reglas operacionales (NO violar durante la ejecución)

1. **Pre-commit en cada commit**: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`. Falla → arreglar antes de commitear. NO `--no-verify`.
2. **Migraciones append-only**. Cada fase = 1 migración numerada `NNNN_<scope>.surql`. NUNCA editar aplicada.
3. **Multi-tenant en cada query**. Toda `SELECT/UPDATE/DELETE` filtra por `tenant = $tenant`. JWT claim → `State<TenantId>`. Test que falla si query no filtra.
4. **Errores en español user-facing**. Códigos SCREAMING_SNAKE en inglés.
5. **No bloquear hot path**: AuditLayer, métricas, logs JSON → todo `tokio::spawn` o channel async.
6. **Performance budget**: POS sale <50ms p99 → bench obligatorio Fase 4.
7. **Bitácora dual por hito**: append `bitacora.md` repo + vault, actualizar `decisions-log-index.md`.
8. **Plan mode** antes de cada fase. Subagent `Explore` para releer endpoints Tu Farmacia.
9. **Vault hints**: leer `reference/pharma-server-db.md`, `reference/pharma-server-api.md`, `brain/pharma-server-patterns.md`, `brain/pharma-server-gotchas.md` antes de tocar zonas relevantes. NO duplicar lectura si el hook ya las sugirió.
10. **CLI-first**. Si necesitas herramienta nueva → instalar (`cargo install`, `choco install`) y seguir.
11. **CommitConventional + SemVer**. Bump `workspace.package.version` patch por fase, minor al cerrar v0.2.0.

---

## 6. Cómo verificar paridad al terminar

Checklist final (correr en última sesión):

1. `find pharmacy-ecommerce/apps/web/src/app/api/admin -name route.ts | wc -l` (Tu Farmacia) vs `grep -r "axum::routing" crates/api/src/v1 | wc -l` (pharma-server). Debe haber ≥87 rutas.
2. Tabla SurrealDB count ≥30 (`INFO FOR DB` en surreal CLI).
3. Cada endpoint Tu Farmacia documentado en `/openapi.json` pharma-server con request+response schema.
4. `cargo bench` POS sale <50ms p99.
5. Suite `cargo test --workspace` verde, coverage ≥70% en crate `domain`.
6. Smoke MSI: install limpia → tenant-create → user-create → login curl → POST product → POST pos/sale → uninstall.
7. Backup → restore round-trip preserva 100% datos.
8. `cargo clippy --workspace --all-targets -- -D warnings` clean.

---

## 7. Primer paso concreto

Después de leer este prompt entero:

1. `git checkout -b feature/erp-parity`.
2. `Plan mode` con scope **Fase 1 — Foundation**.
3. Spawn subagent Explore con prompt:
   > Lee `C:\Users\Administrator\Documents\GitHub\build-and-deploy-webdev-asap\pharmacy-ecommerce\apps\web\prisma\schema.prisma` entero. Para cada uno de los 31 modelos, devuelve: nombre, lista de campos con tipos Postgres, índices, relaciones. Formato Markdown tabla. Reporte completo, sin omitir campos.
4. Con ese reporte, crear `docs/parity-schema-mapping.md` mapeando los 31 modelos → tablas Surreal con tipos equivalentes (Decimal→decimal, Uuid→record id, Timestamptz→datetime, etc.).
5. Iniciar Fase 1.

---

## 8. Recordatorios finales

- NO portar frontend Next.js a este repo. Frontend va en repo aparte (futuro `pharma-admin-ui` o cliente Tauri).
- NO copiar dependencias cloud de Tu Farmacia (Firebase, Vercel, Cloud SQL, Resend, Transbank PROD, Mercado Pago, Stripe). Stubs si hace falta.
- NO mezclar lógica entre repos. Cross-imports prohibidos. `pharma-server` es genérico, vendor-agnostic, vendible a cualquier farmacia.
- Si dudas sobre alcance → priorizar features Ley 20.000 (controlados, recetas, libro recetas) y boleta SII stub. Eso vende.

Listo. Comenzar.
