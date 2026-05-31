# Runbook — Migración Tu Farmacia (Coquimbo) → pharma-server tenant `tufarmacia`

Fecha: 2026-05-31 · Ejecutado autónomo (Opus, /effort max). Reproducible end-to-end
vía `.tmp-migration/migrate_all.py` (un solo proceso: extrae, transforma, importa,
verifica, reconcilia; exit 0 sólo si cuadra).

Migra la farmacia real **Tu Farmacia** (repo `build-and-deploy-webdev-asap`, GCP
`tu-farmacia-prod`, Cloud SQL Postgres 15) a un **tenant** del producto
`pharma-server` corriendo local en `http://127.0.0.1:8080`, para operar con el
Pharma Client (Tauri).

## 0. Fuente y destino

| | |
|---|---|
| Fuente | Cloud SQL `tu-farmacia-prod:southamerica-east1:tu-farmacia-db`, **db `farmacia`, user `farmacia`** |
| Acceso | **Cloud SQL Auth Proxy v2** en `127.0.0.1:6543` (la instancia tiene IP pública `34.39.232.207` pero exige authorized-networks; el proxy con SA evita tocar firewall) |
| Destino | pharma-server local `:8080`, tenant `tufarmacia` (`tenant:be5iipw5er715eocqyl7`), admin `admin@tufarmacia.cl` (roles `admin,owner`) |
| Endpoints | `POST /api/v1/products/import` (CSV), `/api/v1/stock-movements/import` (CSV), `/api/v1/admin/import-customers` (JSON), `/api/v1/admin/import-historic-orders` (JSON) |

## 1. Credenciales (NO hardcodear, NO commitear)

```bash
cd build-and-deploy-webdev-asap
vercel env pull .env.vercel.pull --environment=production --yes
# trae DB_USER, DB_NAME, DB_PASSWORD, CLOUD_SQL_INSTANCE, FIREBASE_*, etc.
```

**Gotcha verificado**: los valores del pull vienen entre comillas y con un literal
`\n` final (bytes `5c 6e`, no salto real) → `DB_USER="farmacia\n"`. Hay que
des-comillar y stripear ese token o falla con
`password authentication failed for user "farmacia\n"`. El extractor robusto:
`v.strip().strip('"'); re.sub(r'(\\[rn]|[\r\n])+$','',v)`.

`.env.vercel.pull` y `.creds.env` **NO** están gitignored → borrarlos al terminar.
El SA `tu-farmacia-prod-*.json` ya está gitignored + no-trackeado, pero **rotarlo**
(estuvo en disco): GCP IAM → key nueva → actualizar secret Vercel → borrar la vieja.

## 2. Esquema real de la fuente (verificado 2026-05-31)

**La DB evolucionó** desde el enunciado original. Tablas reales (uuid-based):

```
products(id uuid PK, external_id varchar[5 nulos, casi-único 34131/34136],
         name, slug, description, price numeric, cost_price numeric, stock int,
         category_id, laboratory, active_ingredient, therapeutic_action,
         prescription_type, presentation, image_url, discount_percent, active)
product_barcodes(id, product_id->products.id, barcode)        -- 39230 filas, 34052 productos
stock_movements(id, product_id, admin_id, delta, reason, created_at)  -- SIN col `ref`
orders(id uuid, total numeric, payment_provider, guest_email, guest_name,
       guest_surname, customer_phone, status, created_at, ...)   -- 53 filas
order_items(id, order_id, product_id, product_name, quantity, price_at_purchase) -- 58
profiles(id, name, role, phone, rut)                             -- 3 = STAFF, no clientes
loyalty_transactions(...)                                        -- 0 filas
```

**NO existen** `customers` ni `ventas_historicas` (el "ventas_historicas" es sólo un
valor de `stock_movements.reason`, 322 filas). Conteos fuente: products **34136**,
stock_movements **3752** (import_excel 3297 + ventas_historicas 322 + reposicion 133),
orders 53, distinct guest_email 40, Σstock **8106**.

### Mapeo

- **external_id pharma = str(products.id uuid)**. `products.external_id` tiene 5 nulos
  y no es la clave de los FK; todos los FK (`stock_movements/order_items/
  product_barcodes.product_id`) apuntan a `products.id` → `str(id)` resuelve todo.
- **Dinero** numeric `"5500.00"` → `int(round())` CLP → **string** (rust_decimal).
- **Clientes** = `DISTINCT ON (lower(guest_email))` de `orders` (no hay tabla de
  clientes). `profiles` son staff → se omiten. Sin RUT en origen.
- **Historia de ventas** = `orders` + `order_items`. 6 órdenes quedan sin ítems
  válidos (sus order_items referencian productos borrados, orphan-FK) → se omiten.
  `payment_provider` (mercadopago/webpay/store) → `pos_cash` (único método válido
  del importador histórico; ver POS_METHODS).
- **reason**: el server sólo valida no-vacío (sin allowlist) → los reasons del origen
  pasan tal cual.

## 3. Modelo de stock (invariante)

pharma garantiza `product.stock == Σ(stock_movement.delta)` (cada movimiento ajusta
`stock` en la misma tx). Por eso:

1. productos se importan con `stock=0`;
2. se cargan los **3752 movimientos reales** (historia/auditoría);
3. se carga **1 movimiento `inventario` de apertura por producto** =
   `stock_origen − Σ(delta del producto)`, para que `stock_final = stock_origen`
   **sin romper** el invariante (33991 productos requieren ajuste ≠ 0; los otros
   145 ya cuadran por sus movimientos).

Orden de carga: **products → stock_movements → stock_reconcile → customers →
historic**. `historic` NO toca stock (sólo puebla `order`/`order_item`), por diseño.

## 4. Preparar destino

```bash
# server OFF (SurrealKv file-lock) → CLI:
pharma migrate                                  # ya aplicado
pharma tenant-create "Tu Farmacia" --slug tufarmacia
PHARMA_PASSWORD=<pw> pharma user-create --tenant tufarmacia \
  --email admin@tufarmacia.cl --roles admin --password <pw>
# server ON — REQUIERE JWT secret fuerte (rechaza el placeholder):
PHARMA__JWT__SECRET=$(openssl rand -hex 32) pharma-api    # :8080
TOKEN=$(curl -s -XPOST :8080/api/v1/login -H 'Content-Type: application/json' \
  -d '{"tenant":"tufarmacia","email":"admin@tufarmacia.cl","password":"<pw>"}' | jq -r .token)
```

Flags CLI verificados: `tenant-create <NAME> --slug`; `user-create --tenant --email
--roles --password` (rol plural `--roles`; roles válidos cashier|pharmacist|admin|owner).
Login body requiere `tenant`,`email`,`password`. **pharma-api no arranca con el JWT
placeholder** — inyectar `PHARMA__JWT__SECRET` (o `PHARMA_ALLOW_INSECURE_JWT=1` en dev).

## 5. Resultado (reconciliado 2026-05-31, exit 0)

| Dato | Origen (Cloud SQL) | Importado | failed/errors |
|---|---|---|---|
| products | 34136 | **34136 created** | 0 |
| stock_movements (real) | 3752 | **3752 created** | 0 |
| stock apertura (`inventario`) | — | **33991 created** | 0 |
| customers (distinct guest_email) | 40 | **39 created** | 0 |
| historic orders (orders c/ítems) | 47 (de 53; 6 sin ítems válidos) | **47 created** | 0 |

**Verificación independiente** (re-query directo a la API, no del pipeline):
- `GET /products/stats` → `total=34136, active=34136, stock=8106, inventory_value=303339030`.
- **Invariante**: Σstock pharma = **8106** == Σstock origen = **8106** ✓.
- `GET /customers` → poblado con guest emails reales.

Notas: customers 39 vs 40 distinct = un email colapsó al normalizar (lower/empty).
Idempotente: re-correr no duplica (productos upsert por `external_id`; customers por
`(tenant,email)`).

## 6. GAP — barcodes (34052) NO migrados (follow-up con código)

El catálogo de pharma **no tiene** campo `barcode` (verificado: 0 refs en
`crates/domain/src/catalog/{model,service,repo}.rs`). Los barcodes viven en una tabla
schemaless **`product_barcode`** (`tenant`, `product`, `barcode`) que SÓLO lee el
lookup de agentes (`agent_inbox`/`agent_orders`); ningún código de producción la
escribe (sólo tests la siembran), y `products/import` ignora la columna `barcode`.

La data ya viaja en `products.csv` (col `barcode`, 34052 con valor). **Fix (1 PR
chico, requiere rebuild):**
1. `import_products` (`crates/api/src/v1/catalog.rs`): leer `col("barcode")`.
2. Repo fn `upsert_barcode(db, tenant, product, barcode)` →
   `UPSERT product_barcode` idempotente por `(tenant, barcode)` (matchea el shape que
   consultan los tests: `SELECT VALUE product FROM product_barcode WHERE tenant=$t AND
   barcode=$b`).
3. Migración `0051_product_barcode.surql` con índice único `(tenant, barcode)`.
4. Rebuild `pharma-api` release, reiniciar, re-correr `products/import` (upsert
   actualiza los 34136 y siembra barcodes).

**Diferido** porque editar Rust en la rama compartida `feat/dte-9-1-b2-xmldsig` (otra
sesión activa) arriesga conflictos (cf. lección #96), y el GATE de rebuild release
queda fuera del alcance de una migración de datos.

## 7. Otros no migrados (a propósito)
- **push_subscriptions (2)**: web-push del e-commerce; sin destino on-prem.
- **profiles (3, staff)**: el admin se creó manual; no hay bulk-import de staff.
- **Firebase Auth/Firestore**: no exportado (requiere `firebase login` interactivo +
  `auth:export`/`firestore:export`). Los clientes salen de `orders.guest_email`.
- **órdenes sin ítems válidos (6)**: order_items apuntaban a productos borrados.

## 8. Verificación Pharma Client
Cliente Tauri (`client/`) = cliente delgado sobre la misma API `:8080`. Login: tenant
`tufarmacia`, `admin@tufarmacia.cl`, URL `127.0.0.1:8080`. Inventario muestra 34136
SKUs, Clientes 39, Reportes/órdenes 47.

## 9. Limpieza (secretos efímeros)
```bash
taskkill /IM cloud-sql-proxy.exe /F
rm -rf pharma-server/.tmp-migration                 # sa/token/jwt/CSV/JSON con PII real
rm build-and-deploy-webdev-asap/.env.vercel.pull* build-and-deploy-webdev-asap/.creds.env
```
`.tmp-migration/` y `demo-data/` NO se commitean (data real + PII).
