# SaaS Web Cloud — RutBusiness en browser (diseño)

- **Fecha**: 2026-07-21
- **Estado**: Aprobado en brainstorming con founder (esta fecha)
- **Objetivo**: "La app descargable, pero en web": cualquier persona crea cuenta
  en rutbusiness y usa el ERP completo en el browser, sin descargar nada.

## Decisiones tomadas (founder, 2026-07-21)

| Decisión | Valor |
|---|---|
| Modelo | **Cloud completo (SaaS)** — no solo PWA-contra-server-propio, no solo demo |
| Freemium | **Free también en cloud** — cuenta gratis usa ERP core en browser (extiende espíritu ADR-0005 a web) |
| Infra | **GCE VM Linux** (gcloud ya instalado). e2-micro free tier (us-central1/west1/east1, 1GB RAM, 30GB) primero; plan B e2-small ~US$13/mes. **Proyecto GCP NUEVO** (ej. `rutbusiness-cloud`) — NUNCA `tu-farmacia-prod` (regla de scope: farmacia real separada) |
| Alcance v1 | **Signup + ERP core** (las vistas completas). SIN pago cloud, SIN migración MSI↔cloud, SIN offline web — v2+ |
| Arquitectura | **Reuso total** (opción A): mismo `pharma-api` multi-tenant, mismo `client/src` frontend. Descartadas: Cloud Run (filesystem efímero vs SurrealKV embebido), 1 contenedor/tenant (innecesario, server ya multi-tenant por JWT) |

## Topología

```
rutbusiness.cl            landing (Vercel, license-server)         — PR10, existe
app.rutbusiness.cl        cliente web PWA (estático)               — NUEVO
api.rutbusiness.cl        pharma-server cloud (GCE VM)             — NUEVO
cuenta / checkout         license-server (Vercel)                  — existe; suma signup
```

- **Un** pharma-server multi-tenant en la VM. SurrealKV en disco persistente de la VM.
- Caddy en la VM: TLS automático (Let's Encrypt) + reverse proxy a `pharma-api` (systemd).
- Backup: snapshot SurrealKV programado + copia a GCS bucket (ciclo diario mínimo).
- Dominio: verificar tenencia de `rutbusiness.cl` antes de SP1; si no existe aún,
  MVP opera con IP pública + subdominio provisional y el dominio se conecta después.

## Flujo signup

1. Landing CTA nuevo: **"Usar gratis en web"** (junto al CTA descarga MSI).
2. Form en license-server: email, password, nombre negocio, RUT, rubro (catálogo
   `docs/strategy/rubro-catalog.md`).
3. license-server valida y llama `POST /admin/v1/tenants` en api.rutbusiness.cl —
   endpoint **nuevo** en `crates/api`, protegido por secret compartido
   (header `X-Provisioning-Key`, env `PHARMA__PROVISIONING__KEY`; solo activo si
   la env existe — instalaciones on-prem no lo exponen).
4. Server crea tenant + usuario admin/owner (reusa lógica de `tenant-create` +
   `user-create` del CLI, movida/compartida a lib para llamarla desde el handler).
5. Redirect a `app.rutbusiness.cl` con server URL pre-seteada y login listo.

Anti-abuso v1 (mínimo viable): rate limit por IP en signup (license-server) +
verificación de email (link) antes de provisionar. Sin CAPTCHA en v1.

## Cliente web

Hecho verificado (2026-07-21, branch `erp-parity`): la capa API del cliente son
**73 comandos Tauri** (`client/src-tauri/src/commands/`), cada uno **proxy HTTP
1:1** a `/api/v1/...` (método+path documentados en el doc-comment de cada
comando; token Bearer vive en `SessionState` Rust). El frontend TS llama
`invoke()` — **no** `fetch` — así que `vite build` directo NO funciona en browser.

**Solución: shim de transporte web.** Módulo TS que implementa
`invoke(cmd, args)` con `fetch(serverUrl + path)`:

- Alias de build (`resolve.alias` en Vite, solo target web) que sustituye
  `@tauri-apps/api/core` por el shim. Cero cambios en las 18 vistas ni en
  `client/src/api/*` (barrel intacto).
- Tabla comando→(método, path, query/body) portada de los doc-comments Rust.
  Mecánico: 73 entradas.
- Token: en memoria + `sessionStorage` (no localStorage — expira con la pestaña).
  Mismo envelope de error `{error:{code,message}}` → mismos mensajes español.
- Comandos desktop-only (impresora ESC/POS, updater, export a archivo local)
  degradan con mensaje "disponible en la app de escritorio" — la vista no muere.
- PWA: manifest + service worker shell-only (datos siempre por red, v1).
- `server-url.ts` ya existe (ADR-0015 P0 hecho): build web fija default
  `https://api.rutbusiness.cl` en vez de loopback.

Deploy: build estático a Vercel (o al mismo Caddy). Sin app store, US$0.

## Server cloud

- **Build Linux**: compilar solo `-p api` (+ deps core/db/auth/license/domain/dte).
  El crate `service` es Windows-only y queda fuera. `rust-toolchain.toml` pin
  1.95.0 sirve; target `x86_64-unknown-linux-gnu`. **GATE 0 del proyecto: si el
  workspace no compila Linux, arreglar cfg-gates antes de todo lo demás.**
- systemd unit `pharma-api` + Caddy site. Deploy: binario por `gcloud compute scp`
  (script `scripts/deploy-cloud.sh`); CI/CD después.
- Config producción vía env `PHARMA__*`: JWT secret real, provisioning key,
  path de datos en disco persistente.
- Firewall GCP: solo 80/443 (Caddy). `pharma-api` escucha loopback.

## Seguridad / riesgos

| Riesgo | Mitigación |
|---|---|
| Primer despliegue público multiusuario real | Auditoría previa: TODOS los endpoints `/api/v1` filtran por `tenant` del JWT (patrón ya obligatorio, regla CLAUDE.md #4); test de aislamiento cross-tenant nuevo en CI |
| e2-micro 1GB RAM | Suficiente para arrancar; monitorear; upgrade vertical a e2-small es 1 comando |
| Abuso signup gratis | Email verify + rate limit (arriba); límites de recursos por tenant se definen recién si duele |
| Endpoint provisioning expuesto | Secret fuerte + solo-si-env + rate limit + log de auditoría de cada tenant creado |
| VM única = SPOF | Aceptado en v1 (Free). Snapshot diario a GCS; disco persistente sobrevive a la VM |
| SmartScreen/licencia no aplican en web | Free cloud no necesita license file; tiers pagos cloud quedan para v2 |

## Fuera de alcance v1

- Checkout/upgrade de tiers dentro del cloud (Webpay ya existe para MSI; unificar en v2).
- Migración de datos MSI↔cloud (export/import guiado) — v2, honra no-lock-in.
- Offline/cola local en el cliente web (ADR-0015 P4/Fase 15).
- Android/iOS builds (ADR-0015 P1/P3 — lanes aparte).
- Multi-VM / autoscaling / K8s.

## Descomposición (orden de ejecución)

| # | Sub-proyecto | Entrega | Depende |
|---|---|---|---|
| SP1 | Server Linux + VM | `pharma-api` corriendo en GCE con Caddy TLS + systemd + backup GCS + script deploy | — |
| SP2 | Provisioning API | `POST /admin/v1/tenants` con secret + tests + auditoría cross-tenant | SP1 (deployable), código en paralelo |
| SP3 | Cliente web | Shim invoke→fetch (73 cmds) + build PWA + deploy app.rutbusiness.cl | SP1 para probar en vivo; código en paralelo |
| SP4 | Signup + landing | Form cuenta en license-server + email verify + CTA landing "Usar gratis en web" | SP2 |

Cada SP = su propio plan de implementación + PR(s) al lane `feature/saas-web`
(worktree `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\saas-web`,
base `origin/feature/erp-parity`). GATE estándar del repo antes de cada PR.

## Criterio de éxito v1

Persona sin nada instalado: entra a la landing → "Usar gratis en web" → crea
cuenta (email verificado) → vende un producto en el POS, ve inventario y cierra
caja desde el browser. Todo en < 5 minutos, US$0 de costo marginal de infra.
