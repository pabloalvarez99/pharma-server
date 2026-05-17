# pharma-server — Roadmap ecosistema ERP + agentes

> **Visión expandida (2026-05-16)**: pharma-server deja de ser solo un ERP on-prem vendible. Pasa a ser **nodo de un ecosistema federado de agentes ERP**, donde cada instalación es un participante soberano (farmacia, proveedor, droguería, laboratorio). Humanos reales operan cada nodo. Transacciones inter-nodo (cotización, OC, despacho, pago) usan protocolo común.
>
> **Fase 0 (hoy)**: ERP single-nodo vendible. **Fase ∞**: malla de agentes humanos comerciando vía pharma-server. Mismo binario, capas opt-in.

---

## 0. Estado consolidado (post-merge 2026-05-16, v0.1.3)

Branch `feature/erp-parity` integra Fase 1 + Fase 2 + Fase 3 + Fase 5-subset + Fase 7-subset.

Dominio cubierto (16 tablas multi-tenant + 2 globales):
- foundation: `tenant`, `user`, `session`, `audit_log`
- catálogo: `category`, `product`, `product_barcode`, **global** `barcode_catalog`, **global** `therapeutic_category_mapping`
- inventario: `stock_movement`, `product_batch`, `falta`
- compras (subset): `supplier`, `supplier_product_mapping`, `supplier_price_list`
- clientes/recetas: `customer`, `loyalty_transaction`, `prescription`, `pharmacist_shift`

Dominio pendiente (10 tablas en Tu Farmacia Prisma — referencia `docs/parity-prisma-models.md`):
- ventas/POS: `order`, `order_item`, `devolucion`, `devolucion_item`
- compras-full: `purchase_order`, `purchase_order_item`, `purchase_payment`
- finanzas: `caja_cierre`, `gasto_category`, `gasto`, `recurring_expense`
- operaciones: `internal_task`, `announcement`
- settings: `admin_setting` (key-value), `push_subscription` (web, opt-in)

---

## 1. Roadmap fases ERP (extensión)

### Fase 4 — Sales/POS (siguiente, blueker)

- Migración `0007_sales.surql`: `order`, `order_item`, `devolucion`, `devolucion_item`, `admin_setting` (key-value). Multi-tenant.
- `domain::sales` dir-module: tx atómica `POST /api/v1/pos/sale` igual blueprint Tu Farmacia (`apps/web/src/app/api/admin/pos/sale/route.ts`):
  1. Validar stock (pre-check).
  2. `BEGIN; CREATE order; CREATE order_item×N; UPDATE product SET stock -= …; CREATE stock_movement(reason="sale", ref=order.id) ×N; CREATE prescription_record si controlado; CREATE loyalty_transaction si customer; COMMIT;`
  3. FEFO consumption: usa `inventory::service::plan_fefo` de Fase 3 → decrementa lotes en orden expiry ASC.
- Endpoints: `POST /pos/sale`, `GET/POST orders`, `GET orders/:id`, `POST orders/:id/refund`, `POST devoluciones`, `GET devoluciones`, `GET orders/stats`.
- **Reglas de negocio portadas de Tu Farmacia** (literal, lib/ts → domain/rs):
  - `controlled-substances.ts` → `domain::sales::controlled::is_controlled(active_ingredient)` con set Decreto 404 CL.
  - `drug-interactions.ts` (Beers + Vademécum CL) → `domain::sales::interactions::check(cart_active_ingredients[]) -> Vec<InteractionDetail>` (severidad crítica/mayor/moderada).
  - `drug-duplicates.ts` → detectar mismo principio activo en cart.
  - `loyalty.ts` → fórmula puntos: 1 punto/$1000 CLP (configurable via `admin_setting`).
- Idempotency: header `Idempotency-Key` → tabla `idempotency_key` (tenant, key, response_hash, expires_at TTL 24h) — patrón decisión Fase 0.
- Budget: POS sale <50ms p99 hardware mínimo. SurrealKv embedded, sin red en hot path.

### Fase 5-full — Purchasing/AP (post-Fase 4)

- Migración `0008_purchasing_full.surql`: `purchase_order`, `purchase_order_item`, `purchase_payment`.
- WAC (weighted-average cost): al recibir batch, `product.cost_price = (current_stock * current_cost + qty * unit_cost) / (current_stock + qty)`. Atómico con `stock_movement(reason="purchase_received")` + `product_batch` creación.
- Endpoints: `POST/GET purchase-orders`, `PATCH/:id`, `POST :id/receive` (FEFO-aware: stamp expiry + batch_code), `POST :id/payments`, `GET :id/payments`, `GET supplier-prices/compare` (ya existe Fase 5-subset).
- OCR `POST /scan-invoice`: stub 501 ahora. Plug Cloud Vision si online disponible (opt-in flag). Para offline, evaluar `tesseract-rs` (puro Rust) o WASM Tesseract — decisión: postergar a Fase 8 + capa online.

### Fase 6 — Finance/Reports/Operations

- Migración `0009_finance.surql`: `caja_cierre`, `gasto_category`, `gasto`, `recurring_expense`, `internal_task`, `announcement`.
- Endpoints:
  - Caja: `POST /caja/abrir`, `POST /caja/cerrar` (compute esperado vs contado vs diferencia), `GET /caja/cierres`.
  - Cierre-dia: `GET /reportes/cierre-dia?date=YYYY-MM-DD` — agregaciones tipo Tu Farmacia (`apps/web/src/app/api/admin/cierre-dia/route.ts`).
  - Gastos: `GET/POST gastos`, `GET/POST gasto-categories`, `GET/POST recurring-expenses` + scheduler genera `gasto` mensual.
  - Reportes: `GET reportes/ventas`, `/margenes`, `/rotacion`, `/abc` (ya en Fase 3), `/vencimientos`, `/cierre-mensual`.
  - Operaciones: `GET/POST internal-tasks`, `PATCH :id`, `GET/POST announcements`.

### Fase 8 — Cron, backup, swagger, admin UI desktop

- Crate `jobs` (hoy vacío): scheduler para `expiry_alerts` (productos próximos vencimiento), `stock_alerts` (low_stock), `cleanup_orders` (expirar reservas), `daily_summary` (email/log), `weekly_purchases`, `recurring_expense_emit`. Patrón: tokio-cron-scheduler ya en deps.
- Backup: `pharma backup --out <path>` snapshot SurrealKv via `surreal export` → `.surql.gz`. Restore: `pharma restore --in <path>` con prompt confirmación. Scheduler diario default 03:00.
- Swagger UI: utoipa-axum + utoipa-swagger-ui ya en deps, levantar `/api/v1/swagger` en `api`.
- **Admin UI desktop**: Tu Farmacia ya tiene wrapper Electron (`pharmacy-ecommerce/apps/desktop/main.js`). Pharma-server replica: nuevo crate `desktop` con Electron mínimo (o **Tauri** — más liviano, Rust nativo, mismo binary). Apunta a `http://127.0.0.1:8080/app` (admin embebido). Modo POS: `--pos` → kiosk fullscreen en `/app/pos`. Empacable como MSI separado o bundled.

### Fase 9 — Hardening + MSI shippeable

- MSI firmado (cert Authenticode evita SmartScreen).
- Migrate auto en MSI postinstall: `pharma migrate` antes de service start.
- Token rotation: `pharma metrics-token --rotate`, `pharma jwt-secret --rotate`.
- Rate limit: tower-governor en `/api/login`, `/sync/push`.
- Lockout: 5 fails → cooldown 15min por `(tenant, email)`.
- Logs rotation: tracing-appender daily.
- README install guide farmacéutico.

---

## 2. Capa online opcional (Fase 10 — nueva)

Objetivo: cada nodo on-prem puede opt-in a sincronización con cloud (relé Anthropic-hosted o self-hosted) sin perder offline-first. NO requerido para vender. ON por defecto = OFF.

Arquitectura:
- **Outbox local**: tabla `sync_outbox` (tenant, entity, op CREATE/UPDATE/DELETE, payload JSON, attempts, last_error?, sent_at?, created_at). Audit middleware existente (Fase 1) ya intercepta mutations → extender para escribir outbox cuando `[sync].enabled=true` en config.
- **Push worker**: cron en crate `jobs`, polling outbox, POST a `sync_endpoint` con bearer + payload firmado (HMAC-SHA256 con device key). Backoff exponencial. Idempotente por `(tenant, op_id)`.
- **Pull worker**: GET `/sync/pull?since=<cursor>` para recibir actualizaciones de:
  - catálogo global (`barcode_catalog`, `therapeutic_category_mapping`) — fuente única, sin conflicto.
  - precios sugeridos cross-tenant (opt-in marketplace) — read-only.
- **Receiver cloud** (deploy aparte, NO incluido en MSI): API REST `/sync/push` + `/sync/pull` + cola dispatch. Persistencia liviana (Postgres + S3 para payloads grandes).
- **Conflicto**: tenant-owned data NUNCA se pulls (source-of-truth = local). Solo datos globales/marketplace. LWW por `updated_at` + tenant tie-breaker para ambiguous.
- **Datos sensibles**: PII paciente, recetas, ventas → NUNCA salen del nodo sin opt-in explícito por tenant. Default = solo metadata anónima (KPIs agregados opcionales).
- **Auth nodo↔cloud**: device key Ed25519 generada en install (`pharma keypair --init`), persistida en `data/device.key` permisos 600. Pub key registrada en cloud al primer push.

Nuevas tablas: `sync_outbox`, `sync_cursor` (last_pulled_at por feed).
Nuevo crate: `sync` (push + pull workers + signing).
Nuevos endpoints (local): `GET /sync/status`, `POST /sync/force-push`.
Config: `[sync] enabled = false; endpoint = ""; share_kpis = false; share_catalog = true`.

---

## 3. Foundation ecosistema agentes (Fase 11 — sketch)

Objetivo: cada nodo pharma-server es un **agente** con identidad criptográfica, capaz de transar con otros agentes (humanos detrás de cada uno). No depende de cloud central — protocolo P2P opcionalmente federado por relays opt-in.

### 3.1 Identidad

- DID-style: `did:pharma:<base58-ed25519-pubkey>`.
- Generada en install (`pharma agent --init`), persistida en `data/agent.key`. Pub key + metadata pública (`name`, `kind=pharmacy|supplier|distributor|lab`, `region`) en `agent_card.json` autofirmado.
- Verifiable Credentials opcional: ISP/SII attestation de farmacia legítima (post-Fase 11+).

### 3.2 Mensajería

Envelope estándar:
```json
{
  "from": "did:pharma:<pk_a>",
  "to": "did:pharma:<pk_b>",
  "msg_id": "uuid",
  "ts": "RFC3339",
  "topic": "quote.request | quote.response | po.create | shipment.notify | payment.confirm",
  "body": { ... topic-specific JSON ... },
  "sig": "ed25519(canonical_json(envelope_sin_sig))"
}
```

Transport opciones:
- **HTTP push directo** (LAN o WAN si pubkey + endpoint conocidos). Default.
- **Relay federado opcional** (hub HTTP que cola mensajes para nodos offline). Sigue siendo opt-in, mensaje E2E-firmado.
- **NATS** (ya en deps workspace, no usado todavía) para relay self-hosted.

### 3.3 Topics MVP

- `catalog.lookup` (request) → `catalog.match` (response): "tengo este código de barras / SKU, ¿qué tienes?".
- `quote.request` → `quote.response`: lista items + qty, recibe precios + lead time.
- `po.create` → `po.ack` / `po.reject`: orden de compra inter-nodo.
- `shipment.notify` → `shipment.ack`: track despacho.
- `payment.confirm` → `payment.ack`: cierre AP/AR.

### 3.4 Esquema compartido

- `barcode_catalog` global (ya existe) = vocabulario producto canónico.
- `agent_card` schema compartido para discovery.
- Resolución cross-tenant: `(barcode, qty, currency=CLP)` → identifica producto vía catálogo global → cada nodo mapea a su `product` interno.

### 3.5 Reputación

Tabla nueva `agent_interaction` (peer_did, kind, outcome, ts, optional_rating). Acumula confianza local-only — NUNCA centralizada. Cada nodo construye su grafo de trust.

### 3.6 Discovery

- Bootstrap manual (intercambiar `agent_card.json` por mail/QR).
- Federado opcional: agente publica card en relay → otros lo buscan.

---

## 3bis. Fase 12 — capa de confianza / marketplace (estrategia)

Las Fases 1-11 entregan ERP vendible + protocolo agente. La **Fase 12** sube un nivel:
convertir el protocolo firmado existente (`crates/agent/*`, `/agent/inbox`,
`agent_interaction`, `agent_order`) en un **marketplace B2B de confianza con identidad
verificable, escrow y reputación portable** — y, a largo plazo, en un riel de
identidad/liquidación reusable para SMB LATAM.

El análisis fundador/VC/arquitecto completo (validación de mercado, modelo de negocio,
arquitectura del Trust Hub, identidad/antifraude, GTM Coquimbo/La Serena, roadmap 24
meses, financiero, riesgos existenciales, visión unicornio) vive en
**[`docs/marketplace-master-plan.md`](./marketplace-master-plan.md)**.

Resumen ejecutivo: el activo diferencial no es el marketplace sino el protocolo
federado firmado anclado a un nodo ERP que ya se vende solo; estrategia elegida = B2B
vertical (farmacia indep. ↔ distribuidor) con ERP como anzuelo; monetización en 3
capas (ERP SaaS + take rate escrowed + identity-as-a-service); Hub centralizado online
sobre protocolo federado por debajo (no malla leaderless en v1); **no custodiar
fondos** (orquestar vía PSP licenciado + Khipu/Fintoc). Documento estratégico, no
scaffolding — la implementación del Hub es un plan separado posterior.

---

## 4. Decisiones de arquitectura clave (locked-in vs abierto)

| Tema | Decisión | Estado |
|---|---|---|
| Single binary on-prem | Mantener: MSI + Windows service + SurrealKv embedded | locked |
| Multi-tenant en mismo binario | Sí, igual Fase 1+ | locked |
| Frontend desktop wrapper | Tauri (Rust nativo, más liviano que Electron) | abierto, leaning Tauri |
| Online sync ON por defecto | NO. Opt-in por tenant. | locked |
| Datos sensibles fuera del nodo | NUNCA sin opt-in. Default = solo catálogo + KPIs agregados. | locked |
| Protocolo agente | Ed25519 + HTTP push + relay opcional + JSON canónico firmado | locked |
| Reputación centralizada | NO. Local-only por nodo. | locked |
| Marketplace cross-tenant | Read-only fase 1 (precios sugeridos), bidireccional fase 2 | abierto |
| Identity verifiable (SII/ISP) | Post-Fase 11. Out of scope ahora. | abierto |
| Hub federado oficial | Self-host opcional, no requerido | abierto |

---

## 5. Orden propuesto + estimación gruesa

1. Fase 4 Sales/POS — 2-3 sesiones (es el corazón ERP, libera valor inmediato).
2. Fase 5-full Purchasing+AP — 2 sesiones.
3. Fase 6 Finance/Reports — 2 sesiones.
4. Fase 8 Cron+backup+swagger+desktop wrapper — 2 sesiones (Tauri stub).
5. Fase 9 Hardening+MSI shippeable — 1-2 sesiones. **Cortar v1.0.0 vendible aquí**.
6. Fase 10 Online sync opt-in — 2 sesiones. Cortar v1.1.0.
7. Fase 11 Agent foundation (identity + envelope + 2 topics MVP `catalog.lookup`+`quote.request`) — 3 sesiones. Cortar v1.2.0 = "agent-ready".

Cada fase paralelizable en 2-3 agentes si los slices son independientes — patrón validado por integración 2026-05-16.

---

## 6. Lecciones de Tu Farmacia (referencias literales)

| Tema | Archivo Tu Farmacia | Cómo se reusa en pharma-server |
|---|---|---|
| POS sale tx atómica | `apps/web/src/app/api/admin/pos/sale/route.ts` | Blueprint exacto Fase 4 |
| Cierre-dia agregaciones | `apps/web/src/app/api/admin/cierre-dia/route.ts` | Blueprint Fase 6 reportes |
| Drug interactions Beers/CL | `apps/web/src/lib/drug-interactions.ts` | Port literal a `domain::sales::interactions` |
| Controlled substances Decreto 404 | `apps/web/src/lib/controlled-substances.ts` | Port literal a `domain::sales::controlled` |
| OCR Cloud Vision | `apps/web/src/app/api/admin/scan-invoice/route.ts` | Opt-in cuando online; offline = stub 501 |
| Transbank Webpay | `apps/web/src/lib/transbank.ts` | Opt-in integración Fase 4+ (config flag) |
| Desktop wrapper Electron | `apps/desktop/main.js` | Patrón válido, reusable; preferimos Tauri |
| PWA offline page + sw.js | `apps/web/src/app/offline/` + `public/sw.js` | Para futuro frontend web del admin |
| Loyalty award | `apps/web/src/lib/loyalty.ts` | Port a `domain::customers::loyalty::award` |
| Cron jobs | `apps/web/src/app/api/cron/*` | Mapa 1:1 a jobs Rust en crate `jobs` |

Tu Farmacia es **single-tenant cloud-first** (Cloud SQL Postgres + Vercel + Firebase). Pharma-server es **multi-tenant offline-first** (SurrealKv embedded + MSI on-prem). Mismo dominio, arquitectura opuesta. Las reglas de negocio + UI flows se reusan; el stack y el despliegue no.
