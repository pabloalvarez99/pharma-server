---
title: Web ↔ ERP — Auditoría de paridad de features + roadmap
status: Activo (auditoría, no lockeado)
owners: pabloalvarez99
last_review: 2026-05-24
scope: pharma-server (server Rust on-prem + cliente Tauri) vs admin web tu-farmacia.cl
---

# Web ↔ ERP — Auditoría de paridad de features

**Objetivo**: llevar pharma-server (API + cliente Tauri) a nivel producción **igual o mejor**
que el admin web existente de `tu-farmacia.cl/admin` (Next.js, repo
`build-and-deploy-webdev-asap`, ~1520 productos, consola pulida).

Esto es un **punch-list honesto**, no una vuelta de la victoria. Se citan rutas reales y
archivos reales. No se reclama paridad que no exista.

## Fuentes auditadas (read-only)

| Lado | Qué se leyó | Estado a fecha |
|---|---|---|
| ERP backend | `crates/api/src/v1/*.rs` + `routes.rs` + `health.rs` + `middleware/audit.rs` | branch `feature/erp-parity` |
| ERP cliente | `client/src/views/{shell,pos,inventory,reports,login}.ts` + `client/src/api.ts` + `client/src-tauri/src/lib.rs` | branch `feat/client-pos-reports` (último cliente, 2026-05-24) |
| Web admin | Lista de módulos del sidebar (provista) | producción tu-farmacia.cl |

> Nota sobre el cliente: la rama `feat/client-dashboard-caja-clientes` mencionada en el brief
> **no existe** en el remoto. El cliente más completo y reciente es `feat/client-pos-reports`.
> Surfacea exactamente **3 vistas** (POS, Inventario, Reportes) + badge de licencia + health.
> Comandos Tauri reales (`lib.rs`): `login`, `license_status`, `server_health`, `list_products`,
> `inventory_summary`, `sales_daily`, `top_products`, `pos_sale`, `logout`. Nada más.

## Leyenda

| Símbolo | Significado |
|---|---|
| ✅ | Paridad: backend lo soporta **y** el cliente lo surfacea |
| 🟡 | Backend-only: la API existe, **falta vista en el cliente** (la mayoría de los gaps) |
| 🔴 | Falta de raíz: ni backend ni cliente (gap real de producto) |
| ➕ | Ventaja ERP-only: el ERP lo tiene y el **web no** (historia on-prem) |

---

## Matriz de paridad — por módulo del sidebar web

### OPERACIÓN

| Módulo / Feature web | Web | ERP backend (ruta real) | ERP cliente (vista) | Estado |
|---|:---:|---|:---:|:---:|
| **Dashboard** (KPIs operación) | sí | parcial — `GET /api/v1/products/stats`, `/api/v1/reports/sales-daily`. **No** hay endpoint dashboard agregado | no | 🟡 |
| **Ejecutivo** (dashboard ejecutivo) | sí | **no** — `feat/api-exec-dashboard` NO está mergeado en `feature/erp-parity` | no | 🔴 |
| **Insights** (analítica/tendencias) | sí | parcial — datos crudos en `/reports/*`, sin capa "insights" | no | 🔴 (capa) |
| **Actividad** (feed de actividad) | sí | parcial — `middleware/audit.rs` graba `audit_log` inmutable, **pero NO hay endpoint de query** | no | 🔴 (query) |
| **Equipo** (gestión usuarios/roles) | sí | parcial — roles existen (`middleware/role.rs`, JWT claims) y `pharma user-create` (CLI). **No** hay CRUD de usuarios vía API | no | 🔴 (API) |
| **POS** (punto de venta) | sí | `POST /api/v1/pos/sale`, `/api/v1/pos/returns`, `/api/v1/interactions/check` | **sí** (`pos.ts`) | ✅ |
| **Arqueo** (conteo de caja) | sí | `GET /api/v1/cash-sessions/{id}/arqueo` | no | 🟡 |
| **Cierre del día** (cierre caja) | sí | `POST /api/v1/cash-sessions/{id}/close` (+ open, movements) | no | 🟡 |
| **Turnos** (turnos de personal) | sí | parcial — `GET/POST /api/v1/turnos-farmaceutico` es **turno farmacéutico de guardia (ISP)**, NO scheduling de staff | no | 🔴 (semántica distinta) |
| **Tareas** (gestión de tareas) | sí | **no** existe ningún endpoint de tareas | no | 🔴 |

### CATÁLOGO

| Módulo / Feature web | Web | ERP backend (ruta real) | ERP cliente (vista) | Estado |
|---|:---:|---|:---:|:---:|
| **Productos** (CRUD productos) | sí | `GET/POST/PATCH/DELETE /api/v1/products`, `/products/{id}` | parcial — cliente solo **lista** (`list_products`), sin crear/editar | 🟡 |
| **Catálogo ERP** (catálogo ext.) | sí | `GET /api/v1/products` + federación `catalog.match` (`agent.rs`) | parcial — solo lista | 🟡 |
| **Categorías** (CRUD categorías) | sí | `GET/POST/PATCH/DELETE /api/v1/categories` | no | 🟡 |
| **Calidad** (data quality) | sí | **no** existe un concepto "calidad/quality" en `crates/domain` | no | 🔴 |

### ACCIONES TRANSVERSALES

| Acción web | Web | ERP backend (ruta real) | ERP cliente (vista) | Estado |
|---|:---:|---|:---:|:---:|
| **Importar Excel** (carga masiva) | sí | `POST /api/v1/products/import` (+ `/stock-movements/import`, `/supplier-prices/import`) | no | 🟡 |
| **Escanear** (barcode → carrito) | sí | parcial — hay `barcode_catalog` GLOBAL para federación (`agent.rs`), **pero el producto por-tenant NO tiene campo `barcode`** (`catalog/model.rs`) | no | 🟡/🔴 |
| **Precios bulk** (cambio masivo precio) | sí | `POST /api/v1/products/bulk-price`, `/products/update-prices` | no | 🟡 |
| **Ajuste** (ajuste de stock) | sí | `POST /api/v1/products/{id}/stock`, `/stock-movements/adjust` | no | 🟡 |
| **CSV export** (exportar datos) | sí | `GET /api/v1/products/export`, `/libro-recetas/export` | no | 🟡 |

---

## Ventajas ERP-only (➕ el web NO las tiene)

Estas son la **historia de venta on-prem** — capacidades del ERP ausentes en la consola web SaaS:

| Capacidad ERP | Ruta / evidencia | Por qué el web no lo iguala |
|---|---|---|
| ➕ **Auditoría inmutable** | `middleware/audit.rs` → `audit_log` (hash SHA-256 del body, append-only) | Web Postgres no tiene log inmutable de cada mutación de stock/precio/venta |
| ➕ **Recetas controladas Ley 20.000** | `/api/v1/prescriptions` (inmutable), `/libro-recetas` + export | Cumplimiento ISP/DEIS nativo, libro exportable |
| ➕ **Lotes + vencimientos** | `/api/v1/batches`, `/reports/near-expiry` | Trazabilidad por lote y alertas de vencimiento |
| ➕ **Compras / costo promedio** | `/api/v1/purchase-orders` (lifecycle completo), `/supplier-prices/compare` | OC, recepción, comparación de precios proveedor |
| ➕ **Federación de agentes B2B** | `crates/agent`, `/agent/did`, `/agent/inbox`, `catalog.match`, `/api/v1/agent-orders/*` | Ed25519 envelopes entre farmacias/droguerías — el web es un nodo aislado |
| ➕ **Interacciones medicamentosas** | `POST /api/v1/interactions/check` | Check de interacciones en el POS |
| ➕ **Reportes ABC / rotación / margen** | `/reports/{margins-daily,top-products,stock-rotation,near-expiry}`, `/inventory/abc`, `/inventory/reorder-suggestions` | Analítica de inventario más profunda que el web |
| ➕ **Backup local programado** | `POST /api/v1/admin/backup` + cron (`lib.rs`) | Snapshot SurrealKv on-prem, sin cloud |
| ➕ **Offline-first / LAN-only** | SurrealKv embedded, sin red en hot path | El web requiere internet + Cloud SQL; el ERP opera sin internet |
| ➕ **Multi-tenant 1 binario** | JWT `tenant_id` en cada tabla (`migrations/0001_init`) | N sucursales en una instalación, sin SaaS |
| ➕ **License/entitlement offline** | `crates/license` Ed25519 + 402 + `/admin/license/status` | Modelo freemium tiered, validación 100% local |

---

## Resumen cuantitativo

Total de ítems en la matriz: **19** (10 OPERACIÓN + 4 CATÁLOGO + 5 acciones).

| Estado | Conteo | Ítems |
|---|:---:|---|
| ✅ Paridad | **1** | POS |
| 🟡 Backend-only (falta vista cliente) | **10** | Dashboard, Arqueo, Cierre del día, Productos, Catálogo ERP, Categorías, Importar Excel, Precios bulk, Ajuste, CSV export |
| 🔴 Falta de raíz (gap real) | **7** | Ejecutivo, Insights, Actividad (query), Equipo (API CRUD), Turnos (staff), Tareas, Calidad |
| 🟡/🔴 Mixto | **1** | Escanear (infra federación sí, campo `barcode` por-tenant no) |
| ➕ Ventaja ERP-only | **11** | ver tabla arriba |

**Lectura clave**: el ERP **no está lejos** de la paridad. El backend ya cubre ~13 de 19 módulos
web (68%). El cuello de botella es el **cliente Tauri**, que solo surfacea 3 vistas. La mayoría
de los gaps (10/19) son 🟡 "el backend existe, falta la pantalla". Solo **7 son gaps reales de
backend**, y de esos varios son de baja prioridad para una farmacia (Tareas, Insights como capa).

---

## Roadmap a paridad

### Fase A — Surfacear lo que ya existe (cliente Tauri) — alto ROI, bajo riesgo

Todo backend ya implementado; solo se agregan vistas + comandos Tauri + wrappers en `api.ts`.

**Top-5 vistas a agregar (orden de prioridad):**

1. **Caja** (Arqueo + Cierre del día + apertura) → consume `/api/v1/cash-sessions/*`.
   Es el flujo diario crítico de una farmacia. Una sola vista cubre 2 módulos web. **~2-3 días.**
2. **Productos (CRUD completo)** → upgrade de la vista Inventario actual para crear/editar/borrar
   vía `POST/PATCH/DELETE /api/v1/products` + Categorías (`/api/v1/categories`). **~3 días.**
3. **Dashboard operativo** → componer `/products/stats` + `/reports/sales-daily` en una landing
   con KPIs (ventas hoy, stock bajo, por vencer). Sin backend nuevo. **~2 días.**
4. **Acciones de catálogo** (Importar Excel, Precios bulk, Ajuste, CSV export) → botones que
   llaman `/products/import`, `/bulk-price`, `/products/{id}/stock`, `/products/export`. **~3 días.**
5. **Recetas + Libro controlados** → vista para `/api/v1/prescriptions` + `/libro-recetas`
   (ventaja regulatoria, no existe en el web como tal). **~2 días.**

Estas 5 vistas cierran **los 10 ítems 🟡** y dejan al cliente en paridad funcional con el grueso
del web. Esfuerzo total estimado **~12-13 días** de un dev front.

### Fase B — Gaps reales de backend (🔴) — priorizados por valor de farmacia

| Gap | Decisión recomendada | Esfuerzo |
|---|---|---|
| **Actividad** (feed) | Agregar `GET /api/v1/audit-log` (query del `audit_log` ya grabado). Backend casi gratis. | ~1-2 días |
| **Equipo** (usuarios) | Exponer CRUD usuarios vía API (hoy solo CLI `user-create`). | ~3 días |
| **Ejecutivo / Insights** | Endpoint agregador `GET /api/v1/dashboard/exec` (rescatar `feat/api-exec-dashboard` no-mergeado) + capa insights derivada de `/reports/*`. | ~3-4 días |
| **Tareas** | Nuevo módulo dominio + tabla multi-tenant. **Baja prioridad** para una farmacia. | ~4 días |
| **Calidad** (data quality) | Definir qué mide (¿completitud de catálogo?). Posiblemente derivable de `etiquetas/search` + stats. Necesita spec. | spec primero |
| **Escanear (barcode por-tenant)** | Agregar campo `barcode` a `product` (migración `NNNN_*`) + endpoint lookup-por-barcode para el POS. La infra de federación NO sirve para scan-to-cart local. | ~2 días |
| **Turnos (staff)** | Distinto de `turnos-farmaceutico` (guardia ISP). Si se quiere scheduling de personal → módulo nuevo. **Baja prioridad.** | ~3 días |

### Fase C — Apalancar las ventajas ERP-only

No requiere paridad — es donde el ERP **ya gana**. Surfacearlas en el cliente y en marketing:
recetas controladas, lotes/vencimientos, compras, auditoría inmutable, federación B2B, offline-first.
Ver `docs/strategy/freemium-master-plan.md` para el encaje en tiers (margins/top-products = Pro;
federación = Business; SII/ISP auto = Business).

---

## Veredicto

**Para igualar al web → implementar (en orden):**
1. **Vista Caja** (arqueo + cierre) — backend listo, ~2-3 días. *(el gap más doloroso)*
2. **CRUD Productos + Categorías + acciones bulk/import/export** en el cliente — backend listo, ~6 días.
3. **Endpoint `GET /api/v1/audit-log`** para el feed "Actividad" + **CRUD usuarios API** para "Equipo" — ~4-5 días.

(Ejecutivo/Insights, Tareas, Calidad y Turnos-staff quedan como fase 2 — bajo valor inmediato para
una farmacia independiente.)

**El ERP ya le gana al web en:**
- **A. Cumplimiento CL nativo** — recetas controladas Ley 20.000 + libro ISP/DEIS exportable.
- **B. Trazabilidad + auditoría** — lotes/vencimientos + `audit_log` inmutable hash-encadenado.
- **C. Offline-first + on-prem + federación B2B** — opera sin internet, datos en la farmacia,
  multi-tenant en un binario, y transa con otros nodos vía Ed25519. El web SaaS no puede ofrecer esto.

> Honestidad: el web **gana hoy en UX** (consola pulida, 10 módulos navegables) porque el cliente
> Tauri solo tiene 3 vistas. El backend del ERP ya es **más capaz** que el web; la brecha es de
> **front-end de cliente**, no de plataforma. Cerrar Fase A (~2 semanas) invierte la comparación.
