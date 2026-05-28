---
title: License server — estado actual + gaps (repo pharma-license-server)
status: Estado real v2 (repo EXISTE, Fase 11b code-complete)
date: 2026-05-27
owners: pabloalvarez99 (fundador)
related_adrs:
  - ADR-0002 (license Ed25519 offline)
  - ADR-0004 (license-server separado)
  - ADR-0006 (CRL signed)
  - ADR-0007 (key rotation)
  - ADR-0009 (rail de cobro pilot)
last_review: 2026-05-27
---

# License server — estado actual + gaps

> **CORRECCIÓN 2026-05-27**: una versión previa de este doc asumía que
> `pharma-license-server` NO existía y proponía un blueprint Drizzle desde cero. **Falso.**
> El repo EXISTE (privado, creado 2026-05-21), usa **Prisma** (no Drizzle), y tiene la
> **Fase 11b code-complete con Webpay**. Este doc ahora refleja el estado real + los gaps
> que faltan. Fuente de verdad: el `bitacora.md` del propio repo
> `C:/Users/Administrator/Documents/GitHub/pharma-license-server/bitacora.md`.

---

## 1. Repo (ya existe)

- **GitHub**: `pabloalvarez99/pharma-license-server` (PRIVADO, creado 2026-05-21).
- **Local**: `C:/Users/Administrator/Documents/GitHub/pharma-license-server/`.
- **Branch activa**: `feat/webpay-checkout-fase-11b` — Fase 11b **code-complete**,
  deploy pendiente. **PR #1 OPEN**, mergeable, CI Vercel verde.
- **Por qué repo separado**: [ADR-0004](../adr/0004-license-server-separado.md) — priv key
  nunca toca el repo del binario; stack Node vs Rust; deploy Vercel vs MSI.

---

## 2. Stack real (verificado en `package.json` + `prisma/schema.prisma`)

| Capa | Tech real | Nota |
|---|---|---|
| Framework | **Next.js 14.2** App Router + TS + Tailwind | no greenfield |
| DB ORM | **Prisma 6.19** (NO Drizzle) | `prisma/schema.prisma` |
| DB | Postgres (**Neon** target) | free tier |
| Crypto | `@noble/ed25519` v3 + `@noble/hashes` v2 (sha512) | edge-friendly |
| Encoding | `bs58` v6 (DID) + `ulid` v3 (licenseId) | |
| Pagos | `transbank-sdk` 6.1.1 (**Webpay**, sandbox) | Stripe = F11c pendiente |
| Auth admin | `next-auth` v4 credentials + `bcryptjs` cost 12 | ADR-0009 *del license-server* |
| Validación | `zod` v4 | |
| Tests | `vitest` (19/19 verde) | |

Hosting target: **Vercel Hobby (free)** + **Neon free** = $0. Coherente con el plan
[zero-cost-launch-plan.md](./zero-cost-launch-plan.md).

> **OJO — colisión de numeración ADR entre repos**: `pharma-license-server` tiene SU PROPIO
> `docs/adr/0008-kms-strategy.md` y `docs/adr/0009-admin-auth.md`. Son distintos a los ADR
> 0008/0009 de **este** repo (`pharma-server`: self-sign cert + rail pago pilot). Cada repo
> tiene su namespace ADR independiente. Al citar, **siempre prefijar el repo**.

---

## 3. Qué YA funciona (Fase 11b code-complete)

- **Canonical JSON** (`src/lib/canonical.ts`): bit-exact con
  `pharma-server/crates/agent/src/canonical.rs`. **Test cross-repo verde** + fixture
  `fixtures/cross-repo-v1.lic` verificada en Rust ✓.
- **License schema v1** (`src/lib/license.ts`, zod): mirror de
  `crates/license/src/schema.rs`. Lockeada.
- **Signer** (`src/lib/signer.ts`): carga seed Ed25519 desde env, firma canonical bytes.
- **Issuance** (`src/lib/issuance.ts`): valida tenant + active-key DID, firma, persiste.
- **Pricing + feature catalog** (`src/lib/{pricing,feature-catalog}.ts`): mirror del
  catálogo cerrado v1 (subs pro/business mensual+anual + 6 microtx, CLP enteros).
- **Webpay** (`src/lib/webpay.ts`): wrapper transbank-sdk sandbox; switch a prod por env
  `WEBPAY_INTEGRATION_TYPE=PRODUCTION`.
- **Checkout flow**: `POST /api/checkout/start` (crea Order pending + Webpay) →
  `GET /api/checkout/return` (callback idempotente por `Order.webpayToken @unique` →
  emite license → Order confirmed → redirect success).
- **Admin**: NextAuth credentials gate `/admin/*` + `POST /api/admin/licenses/issue`
  (emisión manual auth-guarded).
- **Endpoint público**: `GET /api/licenses/[id]` — sirve `rawJson` firmado, CDN cache 5min.
- **Prisma schema**: `Tenant / License / Order / LicenserKey / CrlEntry`. `Order` YA tiene
  `webpayToken` **y** `stripeSessionId` (Stripe anticipado en schema).
- **Prod key generada**: `key_id=lk-prod-2026-01`,
  DID `did:pharma:HbL8Gfa3x4HEGseE8jqa85NyA1pRg58D6ZbMfV4C5Ep9`, seed en
  `.secrets/prod-key.json` (gitignored).

---

## 4. Gaps — qué falta (BACKLOG real del repo)

### 4.1 Cerrar Fase 11b (deploy — todo $0 excepto Transbank-prod)

Pendientes del `bitacora.md` del license-server (ninguno cuesta dinero salvo el último):

- [ ] `npx vercel link` + instalar Neon Postgres (Marketplace) + `vercel env pull`.
- [ ] `npm run prisma:migrate` + `npm run prodkey:seed` contra Neon.
- [ ] Set Vercel env (Production): `DATABASE_URL`, `LICENSER_PRIVATE_KEY_SEED` (base64 32B),
      `LICENSER_KEY_ID=lk-prod-2026-01`, `NEXTAUTH_SECRET`, `NEXTAUTH_URL`,
      `ADMIN_USERNAME`, `ADMIN_PASSWORD_BCRYPT`, `WEBPAY_INTEGRATION_TYPE=TEST`,
      `NEXT_PUBLIC_LICENSE_BASE_URL`.
- [ ] `npx vercel --prod`.
- [ ] Smoke E2E: tarjeta sandbox `4051 8856 0044 6623` / RUT `11.111.111-1` / clave `123`
      → emite license → descarga → `pharma license activate <id> --server <url>`.
- [ ] **Webpay PRODUCCIÓN** (cobrar dinero real): requiere RUT empresa + certificación
      Transbank (~2-4 sem). **Este es el único paso que NO es $0/inmediato** → ver
      [ADR-0009](../adr/0009-pilot-payment-provider.md) para el rail alternativo de cobro
      pilot sin esperar SpA.

### 4.2 Gap cross-repo CRÍTICO — embeber prod key en pharma-server

- [ ] `crates/license/src/keys.rs` HOY tiene placeholder `("lk-dev-2026",
      "did:pharma:11111...")`. La prod key `lk-prod-2026-01` /
      `did:pharma:HbL8Gfa3x4HEGseE8jqa85NyA1pRg58D6ZbMfV4C5Ep9` **aún NO está embebida**.
      El license-server bitacora la marca como "PR pharma-server v0.1.25" — **pendiente**.
      Sin esto, el binario no verifica licencias reales del license-server (sólo el
      placeholder dev). PR aparte a pharma-server.

### 4.3 Fases siguientes (BACKLOG license-server)

- [ ] **F11c** — Stripe Checkout (microtx tarjeta internacional). Schema ya tiene
      `Order.stripeSessionId`. Ver [ADR-0009](../adr/0009-pilot-payment-provider.md).
- [ ] **F11c'** — Mercado Pago (rail $0 persona-natural para primer cobro CL real, ADR-0009).
- [ ] **F11d** — Admin UI CRUD completo (tenants/licenses/keys/orders, shadcn/ui). Hoy placeholder.
- [ ] **F11e** — `GET /api/crl/v[N]` endpoint firmado ([ADR-0006](../adr/0006-revocation-strategy-signed-crl.md)).
- [ ] **GCP KMS migration** pre-billing (license-server `docs/adr/0008-kms-strategy.md`):
      staging = seed cifrado en env Vercel; prod = GCP KMS asymmetric Ed25519.
- [ ] **Contracts** `contracts/license-v1.schema.json` — JSON-schema compartido Rust+TS.

---

## 5. Cómo encaja en el plan $0

El license-server ya está construido para correr **gratis** (Vercel Hobby + Neon free +
Webpay sandbox). El único costo/espera real para **cobrar dinero real** es:

- **Webpay producción** → RUT empresa + Transbank cert (~2-4 sem). NO es $0 inmediato.
- **Alternativa $0/días** → Mercado Pago (persona natural CL) o Stripe (si hay banca US).
  Ver [ADR-0009](../adr/0009-pilot-payment-provider.md).

Por eso el plan pilot NO espera a Webpay-prod: deploya el license-server free + agrega un
rail de cobro de onboarding rápido. Webpay-prod se activa cuando el fundador constituya
SpA (el código ya está listo, sólo cambia `WEBPAY_INTEGRATION_TYPE=PRODUCTION` + creds).

---

## 6. Handoff a agentes nuevos

**Si vas a tocar el license-server**:
1. `cd C:/Users/Administrator/Documents/GitHub/pharma-license-server` y lee SU `bitacora.md`
   (es la fuente de verdad, NO este doc — este es resumen cross-repo).
2. Branch `feat/webpay-checkout-fase-11b` tiene el código; PR #1 abierto.
3. NO reescribir en Drizzle ni "desde cero" — es Prisma + Next 14, code-complete.
4. El gap más importante para que pharma-server use licencias reales: embeber prod key en
   `crates/license/src/keys.rs` (§4.2).

**Si el fundador dice "deploya el license-server"** → ejecutar checklist §4.1 (todo $0
hasta Webpay-prod). Crear el proyecto Vercel + Neon NO es autónomo (provisiona recursos
externos) → confirmar con fundador.

## More information

- Repo: `C:/Users/Administrator/Documents/GitHub/pharma-license-server/` + su `bitacora.md`.
- [ADR-0004](../adr/0004-license-server-separado.md), [ADR-0002](../adr/0002-license-ed25519-offline.md), [ADR-0006](../adr/0006-revocation-strategy-signed-crl.md), [ADR-0007](../adr/0007-key-rotation-licenser.md).
- [ADR-0009](../adr/0009-pilot-payment-provider.md) — rail de cobro pilot.
- [`zero-cost-launch-plan.md`](./zero-cost-launch-plan.md) §4 — encaje en plan global.
- [`license-architecture.md`](./license-architecture.md) — arquitectura desde el lado pharma-server.
