# pharma-server — API reference

OpenAPI 3.1 spec generada automáticamente desde `#[utoipa::path]` en los
handlers axum. Servida por el binario `pharma-api` en:

- `http://<host>:8080/docs` — Swagger UI (HTML interactivo).
- `http://<host>:8080/docs/openapi.json` — spec cruda (JSON).

## Convenciones

- **Auth**: JWT Bearer en `Authorization`. Reclama `tenant_id`, `sub`, `roles`.
- **Roles**: cashier < pharmacist < admin < owner. Ladder = cada superior
  incluye los anteriores. Helpers: `cashier_plus()`, `pharmacist_plus()`,
  `admin_plus()`, `owner_only()` (ver `crates/api/src/middleware/role.rs`).
- **Errores**: envelope `{ "error": { code, message, details? } }`. `code`
  en SCREAMING_SNAKE; `message` en español; HTTP status estándar.
- **Idempotencia**: `POST /api/v1/pos/sale` honra `Idempotency-Key`.
- **License gate**: endpoints marcados con `tier` requieren plan superior;
  responden 402 `FEATURE_REQUIRES_UPGRADE`. Ver
  `docs/strategy/freemium-master-plan.md`.

## Endpoints (Fase 1 — núcleo POS)

Tabla cubre los 6 módulos anotados con OpenAPI en esta sesión. El resto de
endpoints (`/api/v1/agent/*`, `/api/v1/agent-orders/*`, `/api/v1/backup`,
`/api/v1/expenses`, `/api/v1/license/*`, `/api/v1/purchasing/*`) llega en
PR siguiente.

`tier` = plan mínimo del modelo freemium. `Free` = todos.

### Sales (POS)

| Endpoint | Método | Roles | Tier |
|---|---|---|---|
| `/api/v1/pos/sale` | POST | cashier+ | Free |
| `/api/v1/pos/returns` | POST | cashier+ | Free |
| `/api/v1/orders` | GET | any auth | Free |
| `/api/v1/orders/{id}` | GET | any auth | Free |
| `/api/v1/returns` | GET | any auth | Free |
| `/api/v1/interactions/check` | POST | any auth | Free |
| `/api/v1/settings/{key}` | GET | any auth | Free |
| `/api/v1/settings/{key}` | PUT | admin+ | Free |

### Inventory

| Endpoint | Método | Roles | Tier |
|---|---|---|---|
| `/api/v1/stock-movements` | GET | any auth | Free |
| `/api/v1/stock-movements` | POST | admin+ | Free |
| `/api/v1/stock-movements/adjust` | POST | admin+ | Free |
| `/api/v1/stock-movements/import` | POST | admin+ | Free |
| `/api/v1/batches` | GET | any auth | Free |
| `/api/v1/batches` | POST | admin+ | Free |
| `/api/v1/batches/{id}` | GET / PATCH / DELETE | any/admin+/admin+ | Free |
| `/api/v1/faltas` | GET / POST | any/admin+ | Free |
| `/api/v1/faltas/{id}` | PATCH | admin+ | Free |
| `/api/v1/inventory` | GET | any auth | Free |
| `/api/v1/inventory/abc` | GET | any auth | Pro |
| `/api/v1/inventory/reorder-suggestions` | GET | any auth | Free |

### Catalog

| Endpoint | Método | Roles | Tier |
|---|---|---|---|
| `/api/v1/products` | GET | any auth | Free |
| `/api/v1/products` | POST | admin+ | Free |
| `/api/v1/products/{id}` | GET / PATCH / DELETE | any/admin+/admin+ | Free |
| `/api/v1/products/{id}/stock` | POST | admin+ | Free |
| `/api/v1/products/bulk-price` | POST | admin+ | Free |
| `/api/v1/products/update-prices` | POST | admin+ | Free (501) |
| `/api/v1/products/import` | POST | admin+ | Free |
| `/api/v1/products/export` | GET | any auth | Free |
| `/api/v1/products/stats` | GET | any auth | Free |
| `/api/v1/etiquetas/search` | GET | any auth | Free |
| `/api/v1/categories` | GET / POST | any/admin+ | Free |
| `/api/v1/categories/{id}` | GET / PATCH / DELETE | any/admin+/admin+ | Free |

### CashRegister (Caja)

| Endpoint | Método | Roles | Tier |
|---|---|---|---|
| `/api/v1/cash-sessions` | GET | any auth | Free |
| `/api/v1/cash-sessions` | POST | cashier+ | Free |
| `/api/v1/cash-sessions/{id}` | GET | any auth | Free |
| `/api/v1/cash-sessions/{id}/arqueo` | GET | any auth | Free |
| `/api/v1/cash-sessions/{id}/close` | POST | cashier+ | Free |
| `/api/v1/cash-sessions/{id}/movements` | GET | any auth | Free |
| `/api/v1/cash-sessions/{id}/movements` | POST | cashier+ | Free |

### Customers

| Endpoint | Método | Roles | Tier |
|---|---|---|---|
| `/api/v1/clientes` | GET | any auth | Free |
| `/api/v1/clientes` | POST | cashier+ | Free |
| `/api/v1/clientes/{id}` | GET / PATCH / DELETE | any/cashier+/cashier+ | Free |
| `/api/v1/loyalty` | GET | any auth | Free |
| `/api/v1/loyalty/stats` | GET | any auth | Free |

### Prescriptions (Ley 20.000)

| Endpoint | Método | Roles | Tier |
|---|---|---|---|
| `/api/v1/prescriptions` | GET | any auth | Free |
| `/api/v1/prescriptions` | POST | pharmacist+ | Free |
| `/api/v1/prescriptions/{id}` | GET | any auth | Free |
| `/api/v1/libro-recetas` | GET | any auth | Free |
| `/api/v1/libro-recetas/export` | GET | any auth | Free |
| `/api/v1/turnos-farmaceutico` | GET | any auth | Free |
| `/api/v1/turnos-farmaceutico` | POST | admin+ | Free |
| `/api/v1/turnos-farmaceutico/{id}` | PATCH | admin+ | Free |

## Schemas

Los request/response DTOs se documentan como `object` opaco en esta
versión (los tipos vivien en `crates/domain` y aún no llevan `ToSchema`
porque la slice de domain-DTO docs es otro PR). Para la forma real de
cada body, revisar `crates/domain/src/<modulo>/model.rs` o invocar el
endpoint en Swagger UI con `Try it out` (responde con el JSON real).

## Cómo regenerar la spec local

```powershell
cargo run --bin pharma-api      # arranca el server en :8080
curl http://localhost:8080/docs/openapi.json | jq . > openapi.json
```

El test `crates/api/tests/openapi_spec.rs::spec_serialises_to_json`
garantiza que el spec se genera sin panic en CI.
