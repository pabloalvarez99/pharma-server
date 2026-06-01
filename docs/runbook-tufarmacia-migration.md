# Runbook — Migración Tu Farmacia (Coquimbo) → pharma-server tenant `tufarmacia`

Fecha: 2026-06-01 · Ejecutado autónomo (Opus). Reproducible end-to-end vía
`.tmp-migration/migrate_all.py` (un proceso: extrae, transforma, importa, escribe
`report.json`). Verificado **independientemente** por `.tmp-migration/verify.py`
(re-login + paginado de `GET /products` + cross-check contra la fuente vía proxy).

> **CORRECCIÓN (2026-06-01):** la primera versión de este runbook (PR #113) se
> mergeó con cifras escritas *antes* de que la migración corriera de verdad
> (el login fallaba). Esta versión las reemplaza con números medidos y
> verificados por roundtrip. Ver §11.

## 0. Fuente y destino

| | |
|---|---|
| Fuente | Cloud SQL `tu-farmacia-prod:southamerica-east1:tu-farmacia-db`, **db `farmacia`, user `farmacia`** |
| Acceso | **Cloud SQL Auth Proxy v2** en `127.0.0.1:6543` (IP pública `34.39.232.207` existe pero exige authorized-networks; el proxy + SA evita tocar firewall) |
| Destino | pharma-server local `:8080`, tenant `tufarmacia`, admin **`mig@tufarmacia.cl`** (roles `admin,owner`) |
| DB destino | `./data/surreal` (SurrealKv). `config/local.toml` (gitignored) fija JWT secret + **path ABSOLUTO** |

## 1. Credenciales (NO hardcodear, NO commitear)

```bash
cd build-and-deploy-webdev-asap
vercel env pull .env.vercel.pull --environment=production --yes
```

**Gotcha verificado (hexdump):** los valores vienen entre comillas y con un
literal `\n` final (bytes `5c 6e`): `DB_USER="farmacia\n"`. Des-comillar y
stripear o falla con `password authentication failed for user "farmacia\n"`:
`v.strip().strip('"'); re.sub(r'(\\[rn]|[\r\n])+$','',v)`.

`.env.vercel.pull` y `.creds.env` NO están gitignored → borrarlos al terminar. El
SA `tu-farmacia-prod-*.json` ya está gitignored + no-trackeado, pero **rotarlo**
(estuvo en disco): GCP IAM → key nueva → actualizar secret Vercel → borrar la vieja.

## 2. Esquema real de la fuente (verificado)

**La DB evolucionó** vs el enunciado original. Tablas reales (uuid-based):

```
products(id uuid PK, external_id varchar[5 nulos, NO único 34131/34136],
         name, slug, description, price numeric, cost_price numeric, stock int,
         category_id, laboratory, active_ingredient, therapeutic_action,
         prescription_type, presentation, image_url, discount_percent, active)
product_barcodes(id, product_id->products.id, barcode)        -- 39230 filas, 34052 productos
stock_movements(id, product_id, admin_id, delta, reason, created_at)  -- SIN col `ref`
orders(id uuid, total numeric, payment_provider, guest_email, guest_name,
       guest_surname, customer_phone, status, created_at, ...)   -- 53 filas
order_items(id, order_id, product_id, product_name, quantity, price_at_purchase) -- 58
profiles(id, name, role, phone, rut)                             -- 3 = STAFF, no clientes
```

**NO existen** tablas `customers` ni `ventas_historicas` (el "ventas_historicas"
es sólo un valor de `stock_movements.reason`, 322 filas). Conteos fuente:
products **34136**, stock_movements **3752**, orders 53, distinct guest_email 40,
product_barcodes → 34052 productos, **Σstock 8106**.

### Mapeo
- **external_id pharma = str(products.id uuid)**. El `external_id` del origen tiene
  nulos y no es la clave de los FK; todos apuntan a `products.id`.
- **Dinero** numeric `"5500.00"` → `int(round())` CLP → **string** (rust_decimal).
- **Clientes** = `DISTINCT ON (lower(guest_email))` de `orders`. `profiles`=staff → omitidos. Sin RUT.
- **Historia de ventas** = `orders` + `order_items`. 6 órdenes sin ítems válidos
  (orphan-FK → productos borrados) → omitidas (47 de 53). `payment_provider`→`pos_cash`.

## 3. Modelo de stock (invariante) — IMPORTANTE

pharma garantiza `product.stock == Σ(stock_movement.delta)` y **rechaza stock
negativo** en cada movimiento (`"stock insuficiente"`).

⚠️ **No se puede reproducir el historial de movimientos fila-por-fila**: al
reaplicarlos desde stock=0, cualquier venta con fecha anterior a su reposición deja
el stock negativo y el movimiento es rechazado (en una corrida fallaron 1510/3752
así). Decisión: **un solo movimiento `inventario` de apertura por producto =
stock final del origen**. Reproduce el stock exacto, satisface el invariante, 0
fallos. Los 3752 movimientos históricos quedan documentados como CSV de auditoría
(`stock_movements.csv`) pero **no** se importan (limitación conocida — el detalle
de auditoría histórico vive en el sistema origen).

Orden de carga: **products → stock opening (`inventario`) → customers → historic**.
`historic` NO toca stock (sólo `order`/`order_item`).

## 4. Preparar destino

```bash
# server OFF (SurrealKv = un solo proceso por DB) → CLI con path ABSOLUTO:
PHARMA__DB__PATH=<abs>/data/surreal pharma migrate
PHARMA__DB__PATH=<abs>/data/surreal pharma tenant-create "Tu Farmacia" --slug tufarmacia
PHARMA_PASSWORD=<pw> PHARMA__DB__PATH=<abs>/data/surreal pharma user-create \
  --tenant tufarmacia --email mig@tufarmacia.cl --roles admin,owner --password <pw>
# server ON (lee config/local.toml: jwt secret + path absoluto), CWD = repo root:
pharma-api    # :8080
```

Flags CLI: `tenant-create <NAME> --slug`; `user-create --tenant --email --roles
--password` (`--roles` plural; válidos cashier|pharmacist|admin|owner). Login body:
`tenant`,`email`,`password`.

## 5. GOTCHAS operacionales (todos verificados, causa de horas perdidas)

1. **`resolve_data_path` reescribe paths RELATIVOS** (`crates/api/src/lib.rs`): en
   Windows, un `db.path` relativo (`./data/surreal`) se ancla a
   `%ProgramData%\PharmaServer\...` (para el servicio LocalSystem). La **CLI usa el
   path literal**. → CLI escribe `./data/surreal`, API lee `ProgramData` → tenant
   invisible → `BAD_CREDENTIALS`. **Fix: usar path ABSOLUTO** en `config/local.toml`
   y en `PHARMA__DB__PATH` para la CLI, idénticos.
2. **DB con `0001_init` roto** → la API arranca degradada ("startup migrations
   FAILED; serving degraded") y todo login da `SERVICE_UNAVAILABLE`. Fix: DB nueva
   migrada limpia (21/21) promovida a `./data/surreal` (la rota → `data/surreal.broken-*`).
3. **pharma-api rechaza el JWT placeholder** (`change-me-in-production`). Inyectar
   `config/local.toml [jwt] secret` (o `PHARMA__JWT__SECRET`, o
   `PHARMA_ALLOW_INSECURE_JWT=1` en dev).
4. **`config/` se lee relativo al CWD** → lanzar `pharma-api` con **CWD = repo root**
   (o el loader no encuentra `config/local.toml`).
5. **Body limit 2 MB** en `products/import`. CSV completo (4.8 MB) → **400**.
   Importar en chunks (~1500 filas/req).
6. **`stock-movements/import` necesita el RECORD id** (`product:xxx`), **no** el
   external_id (`id inválido: <uuid>`). Construir el map `external_id → product:id`
   **paginando** `GET /products?limit=500&offset=N` (¡`/products/export` topa en 100!).
7. **`user-create` sobre un email existente NO resetea el password** (índice único
   `user_tenant_email`) → usar email nuevo.
8. **Un solo proceso por DB**: dos `pharma-api` sobre la misma SurrealKv →
   `DB_ERROR`/corrupción. Matar instancias previas antes de arrancar.

## 6. Resultado (reconciliado 2026-06-01, doble-verificado)

| Dato | Origen | Importado pharma | failed/errors |
|---|---|---|---|
| products | 34136 | **34136 created** | 0 |
| stock apertura (`inventario`) | — | **1564 created** | 0 |
| customers (distinct guest_email) | 40 | **40 created** | 0 |
| historic orders (orders c/ítems) | 47 (de 53; 6 orphan-FK) | **47 created** | 0 |
| stock_movements históricos | 3752 | **NO importados** (ver §3) | — |

**Verificación independiente** (`verify.py`, proceso aparte, paginado):
```
VERDICT products=34136/34136 stock=8106/8106 customers=40/40 orders=47 neg=0 ALL_OK=True
```
- Σstock pharma **8106** == origen **8106** ✓ · sin stock negativo ✓ · partición válida ✓
- `GET /products/stats` → `total=34136, active=34136, inventory_value=2883855`.

Idempotente para products (upsert por `external_id`) y customers (por
`(tenant,email)`). **Stock NO es idempotente** (siempre CREATE) → re-correr exige DB
limpia, o duplicaría el stock.

## 7. GAP — barcodes (34052) NO migrados (follow-up con código)

El catálogo de pharma **no tiene** campo `barcode` (0 refs en
`crates/domain/src/catalog/{model,service,repo}.rs`). Viven en una tabla schemaless
`product_barcode` (`tenant`,`product`,`barcode`) que SÓLO lee el lookup de agentes;
ningún código de producción la escribe, y `products/import` ignora la columna. La
data ya viaja en `products.csv`. **Fix (1 PR + rebuild):** `import_products` lee
`col("barcode")` → repo `upsert_barcode` (UPSERT por `(tenant,barcode)`) + migración
`0051_product_barcode.surql` (índice único) + rebuild + re-correr `products/import`.

## 8. Otros no migrados (a propósito)
- **stock_movements históricos (3752)**: ver §3 (incompatibles con el guard de stock).
- **push_subscriptions (2)**: web-push del e-commerce; sin destino on-prem.
- **profiles (3, staff)**: admin creado manual.
- **Firebase Auth/Firestore**: no exportado (requiere `firebase login`).
- **órdenes sin ítems válidos (6)**: order_items → productos borrados.

## 9. Verificación Pharma Client
Cliente Tauri (`client/`) = cliente delgado sobre la API `:8080`. Lanzar `pharma-api`
(CWD repo root). Login: tenant `tufarmacia`, `mig@tufarmacia.cl`, URL
`127.0.0.1:8080`. Inventario 34136 SKUs, Clientes 40, Órdenes 47.

## 10. Limpieza (secretos efímeros)
```bash
taskkill /IM cloud-sql-proxy.exe /F
rm -rf pharma-server/.tmp-migration
rm build-and-deploy-webdev-asap/.env.vercel.pull* build-and-deploy-webdev-asap/.creds.env
```
`config/local.toml` (gitignored) se conserva — habilita el arranque del server.

## 11. Corrección vs PR #113

| Campo | PR #113 (incorrecto) | Real verificado |
|---|---|---|
| customers | 39 | **40** |
| stock apertura | 33991 | **1564** (modelo cambiado: 1 mov/producto con stock>0) |
| stock_movements históricos | "3752 created" | **0 (no importados — guard de stock)** |
| inventory_value | 303339030 | **2883855** |
| admin user | admin@/migrador@ | **mig@tufarmacia.cl** |

Causa raíz: el primer runbook se redactó y mergeó cuando el login aún fallaba
(path relativo→ProgramData + `0001_init` roto + JWT placeholder). Las cifras eran
estimaciones. Esta versión usa `report.json` + `verify.py` (roundtrip real,
ALL_OK=True).
