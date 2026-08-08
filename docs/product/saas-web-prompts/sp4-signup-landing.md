# SP4 — Signup web + CTA landing (pharma-license-server)

Ingeniero en **pharma-license-server** (repo hermano SEPARADO:
`D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-license-server`,
Next.js + Prisma, deployado Vercel production). Objetivo: "Usar gratis en web" —
form de cuenta que provisiona tenant en el cloud (SP2) y manda al usuario a
`app.rutbusiness.cl` logueado. Depende de SP2 deployado (SP1+SP2).

## Setup / Leyes

- Branch nueva en ese repo (ej. `feat/web-signup`), PR a `main`. Gate = scripts
  reales de su `package.json` (lint/test/build). **`npm run build` pide
  `DATABASE_URL` → usar `npx next build` para el gate local** (gotcha conocido).
- NO tocar el contrato Webpay de `CheckoutForm` (flujo de pago existente, intacto).
- Marca: **RutBusiness**, audiencia comercio chileno general, copy es-CL.
- Repo source pharma-server es PRIVADO — no linkearlo en UI.
- Secrets por env Vercel: `PROVISIONING_URL` (ej. `https://api.rutbusiness.cl`),
  `PROVISIONING_KEY` (mismo valor que `PHARMA__PROVISIONING__KEY` de la VM SP1).

## Contrato con SP2 (exacto, no re-derivar)

`POST {PROVISIONING_URL}/admin/v1/tenants` con header `X-Provisioning-Key`:
```json
{ "slug": "...", "business_name": "...", "rut": "...", "vertical": "...",
  "admin_email": "...", "admin_password": "..." }
```
`201 {tenant_id, slug}` · `401` key · `409 TENANT_EXISTS` · `422` validación.
Verticales válidos: `farmacia minimarket restaurant cafe tienda belleza servicios otro`.

## Tareas

1. **Página signup** (`/signup` o `/cuenta` — seguir routing existente del repo):
   email, password, nombre negocio, RUT (validar módulo 11), rubro (select con los 8).
2. **Verificación de email ANTES de provisionar**: leer el repo primero — si ya hay
   infra de email, reusarla; si no hay, integrar Resend (free tier, env
   `RESEND_API_KEY`, founder la setea) con token de un solo uso guardado vía Prisma
   (migración nueva si hace falta). Flujo: form → mail con link → click → recién ahí
   POST provisioning → redirect.
3. **Rate limit** por IP en el POST de signup (simple, en memoria o Prisma; Vercel
   serverless → preferir persistente).
4. **Errores en español**: `TENANT_EXISTS` → "Ese negocio ya tiene cuenta", etc.
5. **Redirect final**: `https://app.rutbusiness.cl` con server URL correcta
   (el web client lee localStorage `pharma:last-server`; pasar por query param que
   el cliente ya entienda o documentar el pendiente para SP3).
6. **CTA landing**: verificar estado de `src/app/page.tsx` — si el landing PR10 ya
   está mergeado, agregar CTA secundario "Usar gratis en web" junto al de descarga;
   si PR10 no existe aún, dejar el CTA en la página que exista y anotar el pendiente.
7. **Doc corto** en el repo: flujo signup + envs requeridas.

## Verificación

Local: signup completo contra SP2 (VM real o server local con
`PHARMA__PROVISIONING__KEY` de prueba) → mail llega → tenant creado → login en el
web client funciona. Anotar evidencia (no declarar verde sin correr el flujo).

## Ship

Gate verde → commit → push → PR a `main` → founder mergea → verificar Vercel
production.

Fin → `✅ SP4 LISTO — signup web vivo, PR #<n> en pharma-license-server · listo para /clear`.
