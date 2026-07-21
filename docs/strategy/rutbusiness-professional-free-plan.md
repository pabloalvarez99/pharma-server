# RutBusiness — plan "profesional + gratis para cada chileno con su RUT"

> Norte (founder): **un ERP gratis para CADA negocio chileno, identificado por su
> RUT, muy profesional.** El pack SaaS web (SP1-SP4) dejó el motor andando en la
> nube + browser. Este plan lleva el producto de "funciona" a "se ve y se siente
> como algo serio que cualquiera confía y usa gratis".

Estado base (2026-07-21): ERP web vivo (https://rutbusiness-web.vercel.app),
signup web (license-server PR #4), free tier en la nube (VM `pharma-prod`).

## Capas para llegar al norte

### 1. Puerta de entrada (confianza + claridad) — ✅ EN CURSO (autónomo)
Lo primero que ve un chileno. **Hecho** (license-server PR #4, commit `a74b848`):
- Landing `/` profesional con marca RutBusiness (firma: el RUT como llave de
  encendido). Reemplaza el redirect a un checkout sandbox con tarjetas de prueba.
- Signup rebrandeado + `?rut=` prellenado desde el landing.
- `/privacidad` + `/terminos` (Ley 19.628).

Pendiente de esta capa:
- OG/meta images + favicon RutBusiness (share en WhatsApp = canal real CL).
- Copys A/B ("gratis para siempre" vs "gratis, sin tarjeta").

### 2. Onboarding sin fricción — parcial
- Signup construido (SP4). **Falta activarlo** (envs Vercel — founder).
- Post-signup: hoy redirige al login web prellenado (server/tenant/email). Mejora:
  auto-login real (token de un solo uso del provisioning → sesión directa) para
  cero fricción. Requiere que SP2 devuelva un token de sesión en el 201.

### 3. App profesional (el operador) — siguiente autónomo
- PWA: prompt "agregar a inicio", ícono, splash.
- Estados de conexión: banner "sin conexión al servidor" claro (hoy el shim tira
  el string español pero la vista podría mostrarlo mejor).
- Touch/tablet: el dueño en tablet en el mesón (POS modo táctil ya existe en
  desktop; verificar en web).
- Onboarding primer-uso guiado en web (el desktop ya lo tiene).

### 4. El agente como héroe — diferenciador (autónomo parcial)
"1 RUT = 1 agente IA" es EL norte (CLAUDE.md). `crates/assist` ya existe
(pregúntale a tu negocio). Surfacearlo como protagonista en web + landing =
profesional + on-vision. Hoy el landing ya lo menciona ("un agente por RUT").

### 5. Confianza infra — YO documento, FOUNDER ejecuta
- **Dominio `rutbusiness.cl`** + `app.` / `api.` (hoy nip.io + vercel.app — NO
  profesional). Founder compra + apunta DNS. Desbloquea TLS real (no self-cert),
  correos `@rutbusiness.cl`, y que el producto "se vea serio".
- Status page (uptime) — autónomo cuando haya dominio.

### 6. Escala "cada chileno" — YO documento, FOUNDER ejecuta
- Hoy: 1 VM e2-micro (1GB RAM), SurrealKV embebido, multi-tenant. Aguanta pilotos,
  NO miles de tenants concurrentes.
- Umbral: monitorear RAM/CPU de la VM; a ~N tenants activos, subir a e2-small o
  separar DB. Aislamiento cross-tenant ya verificado (SP2 tests).
- No optimizar antes de tiempo (disciplina anti-framework): primero validar demanda.

## Qué necesita al FOUNDER (plata / merge / acceso)
1. **Comprar `rutbusiness.cl`** + DNS `app.` → Vercel web client, `api.` → VM.
2. **Mergear** PR #330 (lane `feature/saas-web`) + PR #4 (license-server signup+landing).
3. **Setear envs Vercel** del license-server: `PROVISIONING_URL`, `PROVISIONING_KEY`
   (==`PHARMA__PROVISIONING__KEY` de la VM), `SIGNUP_ENC_KEY` (`openssl rand -hex 32`),
   `RESEND_API_KEY` + dominio verificado en Resend, `SIGNUP_FROM_EMAIL`,
   `NEXT_PUBLIC_APP_URL`.
4. (Escala, cuando toque) subir la VM de e2-micro.

## Orden autónomo sugerido (sin founder)
1. ✅ Puerta de entrada profesional (landing + signup marca + legal).
2. PWA polish del web client (install prompt, estados conexión, touch).
3. Agente como héroe en web.
4. Docs de escala + status page (cuando haya dominio).
