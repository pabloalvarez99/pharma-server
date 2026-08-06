# PR10 — Landing RutBusiness + rediseño checkout (pharma-license-server)

Engineer on **RutBusiness**. Sesión UI-only sobre `pharma-license-server`
(Next.js). Decisiones de diseño YA tomadas con el founder (2026-07-21) — NO
re-brainstormear: marca **RutBusiness**, audiencia **comercio chileno general**,
la página inicial es **landing de producto completa** (este dominio es la cara
pública por ahora).

## Contexto / leyes

- Repo: `D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-license-server`,
  branch `main` @ `48a52a0` (pushed, deploy automático a Vercel production
  `pharma-license-server.vercel.app`).
- Stack: Next.js **14.2.35** app router + React 18 + **Tailwind 3.4** (NO v4) +
  fuentes Geist locales (`src/app/fonts/`) + Prisma + vitest + eslint.
- **CERO cambios de lógica** de pago/licencias/API. Solo UI, copy, routing,
  metadata. No agregar dependencias.
- Usar skill `frontend-design` si está disponible — el founder calificó el
  checkout actual de "horrible"; la vara es alta.

## Hechos verificados (2026-07-21)

- `src/app/page.tsx` = `redirect("/checkout")` — reemplazar por landing.
- `src/app/layout.tsx`: `lang="en"` (cambiar a `"es"`), metadata title
  `pharma-license-server` (cambiar a RutBusiness).
- `src/app/checkout/page.tsx`: server component `force-dynamic`, lee
  `searchParams.tenant_id`, mapea `SUBSCRIPTIONS`/`MICROTX` de
  `src/lib/pricing.ts`, formatter `clp()` con `Intl es-CL`.
- `src/app/checkout/CheckoutForm.tsx` (client): input `tenant_id` POR CARD
  (repetido 10 veces — uno de los problemas), `POST /api/checkout/start`
  `{sku, tenant_id}`, respuesta `{redirect_url, token}` → form POST `token_ws`
  a Webpay. **Este contrato de flujo NO se toca** — solo se re-estiliza y se
  centraliza el tenant_id.
- `src/lib/pricing.ts` (source of truth precios): pro_monthly 19.990 /
  pro_yearly 199.900 / business_monthly 49.990 / business_yearly 499.900 CLP +
  6 microtx (branding 9.990, SII 29.990, Telegram 14.990, reportes 19.990,
  caja extra 9.990, soporte x10 49.990).
- `src/lib/feature-catalog.ts`: tier free existe; keys web pagas =
  `web.custom_domain`, `web.branding_advanced`, `web.payments_online`,
  `web.marketing_automation`. **Todo lo demás web es GRATIS sin license.**
- Scripts: `lint` = next lint · `test` = vitest run · `build` incluye
  `prisma migrate deploy` (necesita `DATABASE_URL` — ver gate).

## Verdades de producto para el copy (no inventar)

- **RutBusiness** = ERP **gratis para siempre**, offline-first, Windows (MSI),
  para cualquier negocio chileno (1 RUT = 1 negocio). POS, inventario, caja,
  reportes básicos, backup, export CSV/JSON completo. Multi-rubro (farmacia,
  almacén, restaurant, tienda…) — cero branding pharma.
- **Tienda web gratis incluida** (lane Free Web mergeado 2026-07-20, PR #329
  pharma-server): catálogo público online, pedidos con retiro en tienda,
  sync ERP↔web, SEO. Estilo Shopify pero gratis y tus datos en tu local.
- Invariantes ADR-0005 (usables como trust-badges): core gratis nunca se
  recorta, sin lock-in de datos, opera sin internet, sin kill-switch remoto.
- **Pagado** (lo que cobra ESTE portal): Pro ($19.990/mes) y Business
  ($49.990/mes) + microtx one-time + web premium (dominio propio, branding
  avanzado, pagos online, marketing automation).
- Copy en español de Chile. Tono directo, sin humo corporativo.

## Tareas

### 1. Landing en `/`

Nueva `src/app/page.tsx` (borrar redirect). Secciones:

1. **Hero**: RutBusiness — "ERP + tienda web gratis para tu negocio". Sub:
   offline-first, tus datos en tu local, 1 RUT = 1 negocio. CTA primario
   "Empieza gratis" + CTA secundario "Ver planes" → `/checkout`.
2. **Gratis para siempre**: grid de lo incluido en Free (POS, inventario,
   caja, tienda web con retiro en tienda, backup, export completo).
3. **Tu tienda web**: pitch Shopify-parity (catálogo online, pedidos retiro,
   SEO) — gratis; premium web.* como upsell listado aparte.
4. **Precios resumen**: card Free ($0, destacada) + Pro + Business, bullet
   features por tier, CTA → `/checkout`. Microtx mencionadas con link.
5. **Confianza**: invariantes ADR-0005 en lenguaje humano.
6. Footer sobrio.

**CTA "Empieza gratis" — resolver en sesión**: verificar release MSI público
con `gh release list -R pabloalvarez99/pharma-server-releases`. Si hay asset
MSI → botón de descarga directo. Si no hay → sección honesta "beta — escríbenos"
con mailto (NO inventar link de descarga). El repo source es privado — NUNCA
linkear `pabloalvarez99/pharma-server`.

### 2. Rediseño `/checkout`

- Header con logo/link ← RutBusiness (a `/`).
- **Card Free $0 primero**: "ya lo tienes — el core no requiere license",
  link a landing sección gratis. Sin form.
- `tenant_id` se ingresa **UNA sola vez** (campo compartido arriba, client
  state o URL param que las cards leen) — no 10 inputs repetidos.
- Cards de plan con jerarquía real (destacar Business anual o Pro mensual,
  elegir una), features por tier en bullets cortos.
- Microtransacciones como grid compacto secundario.
- Mantener footer sandbox (tarjeta VISA de prueba) tal cual.
- Flujo Webpay intacto (mismo POST, mismos endpoints).

### 3. Metadata

`layout.tsx`: `lang="es"`, title "RutBusiness — ERP y tienda web gratis para
tu negocio", description acorde. Favicon si es trivial, skip si no.

## Gate

```powershell
npm run lint
npm test
npx prisma generate; npx next build   # NO usar npm run build (migrate deploy pide DATABASE_URL)
```

Si `next build` falla por env de DB, documentar en el PR y validar con el
preview deploy de Vercel antes de mergear. NUNCA debilitar tests.

## Ship

- Branch `feat/landing-rutbusiness` off `main` → commits → push →
  `gh pr create` base `main` → gate/preview verde → `gh pr merge --merge`
  (repo usa merge commits) → borrar branch.
- Verificar production: `pharma-license-server.vercel.app/` muestra landing
  (sin redirect) y `/checkout` rediseñado.
- `bitacora.md` del repo: append línea con fecha + resumen + PR #.

Done → `✅ LANDING RUTBUSINESS DEPLOYADA — checkout rediseñado`.
