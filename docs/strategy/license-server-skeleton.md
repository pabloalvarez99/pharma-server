---
title: License server skeleton — repo pharma-license-server (separado)
status: Blueprint v1 (no implementado)
date: 2026-05-27
owners: pabloalvarez99 (fundador)
related_adrs:
  - ADR-0002 (license Ed25519 offline)
  - ADR-0004 (license-server separado)
  - ADR-0006 (CRL signed)
  - ADR-0007 (key rotation)
  - ADR-0009 (MP + Stripe pilot)
last_review: 2026-05-27
---

# License server skeleton — repo `pharma-license-server`

> **Blueprint NO implementado.** Este documento es el contrato para bootstrap del repo
> separado `pharma-license-server`. Cualquier agente o el fundador puede ejecutar este
> blueprint paso por paso para tener el license-server corriendo en Vercel free tier
> en ~1-2 días. Si encuentras divergencias entre este doc y el repo real, este doc es
> la verdad **antes** del bootstrap; después del bootstrap, el repo es la verdad y
> este doc debe actualizarse.

---

## 1. Por qué repo separado

[ADR-0004](../adr/0004-license-server-separado.md) lockeó:

- El **license-server** vive en `pharma-license-server` (repo distinto a `pharma-server`).
- Razones: priv key Ed25519 sólo existe en producción del license-server (NUNCA toca
  el repo del binario), distinto stack (Node vs Rust), distinto deploy (Vercel vs MSI),
  open-source futuro distinto (license-server podría ser público sin riesgo; binario
  sigue privado por regla #10).
- Pharma-server consume al license-server **sólo en activación online opcional**; el
  modo offline-first sigue funcionando con `License::free_default()` (per
  [ADR-0005](../adr/0005-core-gratis-no-locked-in.md)).

---

## 2. Stack $0

| Capa | Servicio | Tier | Costo |
|---|---|---|---|
| Hosting | **Vercel** Hobby | Hobby | $0 (100GB bandwidth/mes) |
| DB | **Neon** Postgres | Free | $0 (0.5GB, ~50k licenses) |
| Email | **Resend** | Free | $0 (100 emails/día, sandbox sender) |
| Pagos CL | **Mercado Pago Chile** Checkout Pro | Free | $0 setup, 2.99%+IVA per tx |
| Pagos intl | **Stripe** Checkout | Free | $0 setup, 3.6%+$0.30 USD per tx |
| Cifrado | **Ed25519** (Node `tweetnacl` o `@noble/ed25519`) | Open source | $0 |
| Dominio | Subdominio Vercel `*.vercel.app` | Free | $0 |
| CDN | Vercel Edge built-in | Hobby | $0 |
| Logs / observability | Vercel Logs built-in (24h retention) | Hobby | $0 |

Upgrades paid cuando justifiquen:

| Hito | Servicio paid | Costo |
|---|---|---|
| Dominio custom (`license.pharma.cl`) | Cloudflare Registrar | $12/año at-cost |
| >50k licenses | Neon Pro | $19/mes |
| >100 emails/día | Mailgun / Resend Pro | $20/mes |
| Logs >24h | Better Stack / Axiom | $0-25/mes |

---

## 3. Stack técnico

- **Framework**: Next.js 14 (App Router) — Vercel-native.
- **Lenguaje**: TypeScript estricto (`strict: true`, `noUncheckedIndexedAccess: true`).
- **DB client**: `drizzle-orm` + `@neondatabase/serverless` (HTTP fetch en edge runtime,
  no requiere TCP).
- **Validation**: `zod` para payloads webhook + form `/buy`.
- **Crypto**: `@noble/ed25519` (pure JS, audit-friendly, edge-runtime compatible).
- **Email**: `resend` SDK.
- **Pagos**: `mercadopago` SDK + `stripe` SDK (latest).
- **Tests**: `vitest` + `@vercel/edge-config` mocks.
- **CI**: Vercel auto-deploy on push to `main`; GitHub Actions corre `vitest run` + `tsc --noEmit`.

---

## 4. Repo structure

```
pharma-license-server/
├── README.md
├── .gitignore                       # .env*, node_modules, .next
├── package.json
├── tsconfig.json
├── next.config.mjs
├── drizzle.config.ts
├── .env.example                     # template — todos los secrets listados
├── public/
│   └── pilot.cer                    # NO — pilot.cer vive en mirror, no acá
├── src/
│   ├── app/
│   │   ├── layout.tsx
│   │   ├── page.tsx                 # landing /
│   │   ├── buy/
│   │   │   └── page.tsx             # /buy — 4 botones tier + microtx
│   │   ├── thanks/
│   │   │   └── page.tsx             # /thanks?session=... — post-pago, "revisa email"
│   │   ├── admin/
│   │   │   ├── page.tsx             # /admin — list licenses, search by email/tenant
│   │   │   └── revoke/
│   │   │       └── page.tsx         # /admin/revoke — manual revoke (ADR-0006 CRL)
│   │   └── api/
│   │       ├── webhook/
│   │       │   ├── stripe/route.ts  # POST — Stripe webhook
│   │       │   └── mercadopago/route.ts  # POST — MP webhook
│   │       ├── license/
│   │       │   ├── issue/route.ts   # POST — internal, llamado por webhook
│   │       │   ├── status/route.ts  # GET — pharma-server consulta estado opcional
│   │       │   └── crl/route.ts     # GET — CRL firmado (ADR-0006)
│   │       └── checkout/
│   │           ├── stripe/route.ts  # POST — crea session Stripe
│   │           └── mercadopago/route.ts  # POST — crea preference MP
│   ├── lib/
│   │   ├── db/
│   │   │   ├── schema.ts            # drizzle schema (license, customer, payment_event, crl_entry)
│   │   │   ├── client.ts            # drizzle + neon
│   │   │   └── migrations/          # drizzle-kit output
│   │   ├── crypto/
│   │   │   ├── signer.ts            # firma license JSON con Ed25519
│   │   │   ├── crl.ts               # firma CRL JSON
│   │   │   └── canonical.ts         # JSON canonical (mismo algoritmo que crates/agent/envelope.rs)
│   │   ├── payment/
│   │   │   ├── provider.ts          # interface PaymentProvider
│   │   │   ├── stripe.ts            # StripeProvider impl
│   │   │   ├── mercadopago.ts       # MercadoPagoProvider impl
│   │   │   └── tiers.ts             # tier matrix (mismas keys que crates/license)
│   │   ├── email/
│   │   │   ├── send.ts              # resend wrapper
│   │   │   └── templates/
│   │   │       └── license-delivery.tsx  # React Email
│   │   └── auth/
│   │       └── admin.ts             # admin route guard (env BASIC_AUTH)
│   └── types/
│       ├── license.ts               # mirror de crates/license/types.rs (zod schema)
│       └── tier.ts                  # mirror tier enum
├── tests/
│   ├── crypto.test.ts               # round-trip firma/verify
│   ├── canonical.test.ts            # canonical JSON byte-equal vs Rust
│   ├── stripe-webhook.test.ts       # mock webhook + DB state
│   └── mp-webhook.test.ts
└── .github/
    └── workflows/
        └── ci.yml                   # tsc + vitest + drizzle-kit check
```

---

## 5. DB schema

Postgres en Neon (Drizzle schema en `src/lib/db/schema.ts`):

```typescript
export const customer = pgTable('customer', {
  id: uuid('id').primaryKey().defaultRandom(),
  email: text('email').notNull().unique(),
  rut: text('rut'),  // opcional, CL
  name: text('name'),
  createdAt: timestamp('created_at').defaultNow().notNull(),
});

export const license = pgTable('license', {
  id: uuid('id').primaryKey().defaultRandom(),
  customerId: uuid('customer_id').references(() => customer.id).notNull(),
  tenantId: text('tenant_id').notNull(),  // ID que aparecerá en license.tenant_id
  tier: text('tier').notNull(),  // 'free' | 'pro' | 'business' | 'enterprise'
  microtxKeys: text('microtx_keys').array().notNull().default([]),  // ['sii_unlock', 'branding_pack', ...]
  issuedAt: timestamp('issued_at').defaultNow().notNull(),
  expiresAt: timestamp('expires_at'),  // null = perpetual (microtx) | timestamp = subscription
  keyId: text('key_id').notNull(),  // ADR-0007 multi-key rotation
  signedJson: text('signed_json').notNull(),  // canonical JSON + base64 sig
  revoked: boolean('revoked').default(false).notNull(),
});

export const paymentEvent = pgTable('payment_event', {
  id: uuid('id').primaryKey().defaultRandom(),
  provider: text('provider').notNull(),  // 'stripe' | 'mercadopago'
  externalId: text('external_id').notNull(),  // payment_intent.id / payment.id
  customerEmail: text('customer_email').notNull(),
  amount: integer('amount').notNull(),  // cents / centavos
  currency: text('currency').notNull(),  // 'CLP' | 'USD'
  tier: text('tier'),
  microtxKey: text('microtx_key'),
  status: text('status').notNull(),  // 'pending' | 'paid' | 'failed' | 'refunded'
  licenseId: uuid('license_id').references(() => license.id),
  rawWebhook: jsonb('raw_webhook').notNull(),
  receivedAt: timestamp('received_at').defaultNow().notNull(),
}, (t) => ({
  externalUq: uniqueIndex('payment_event_provider_external_uq').on(t.provider, t.externalId),
}));

export const crlEntry = pgTable('crl_entry', {
  licenseId: uuid('license_id').references(() => license.id).primaryKey(),
  revokedAt: timestamp('revoked_at').defaultNow().notNull(),
  reason: text('reason'),  // 'refund' | 'chargeback' | 'manual_admin' | 'key_rotation'
});
```

Migraciones via `drizzle-kit generate` + `drizzle-kit push` contra Neon.

---

## 6. License JSON canonical (Ed25519)

**MUST match byte-for-byte el formato que `crates/license/src/types.rs` parsea.** Cualquier
divergencia rompe la verificación en el binario.

```typescript
type LicenseJson = {
  v: 1;                              // version
  tenant_id: string;                 // UUID
  tier: 'free' | 'pro' | 'business' | 'enterprise';
  microtx: string[];                 // ej ['sii_unlock', 'branding_pack']
  issued_at: string;                 // RFC3339 UTC
  expires_at: string | null;         // RFC3339 UTC | null = perpetual
  key_id: string;                    // ADR-0007 multi-key
};

// Canonical: keys sorted alphabetically + no whitespace + UTF-8.
// Misma function que crates/agent/envelope.rs::canonical_json_bytes.
function canonicalize(license: LicenseJson): Uint8Array {
  const sorted = Object.fromEntries(Object.entries(license).sort());
  return new TextEncoder().encode(JSON.stringify(sorted));
}

// Sign:
//   sig = base64(ed25519.sign(canonicalize(license), privKey))
// Payload final entregado al cliente:
//   { "license": { ...LicenseJson }, "sig": "...base64...", "key_id": "k1" }
```

**Test obligatorio**: `tests/canonical.test.ts` debe usar fixtures idénticas a las de
`crates/license/tests/fixtures/` (golden files) y verificar **byte-equal**. Si diverge,
romper CI.

---

## 7. Key management

[ADR-0002](../adr/0002-license-ed25519-offline.md) + [ADR-0007](../adr/0007-key-rotation-licenser.md):

1. **Generar keypair localmente** (NUNCA en CI, nunca en repo):
   ```bash
   openssl genpkey -algorithm ed25519 -out license-signer-k1.pem
   openssl pkey -in license-signer-k1.pem -pubout -out license-signer-k1.pub
   ```

2. **Privkey** vive sólo en:
   - Vercel env var `LICENSE_SIGNER_PRIVKEY_K1` (encrypted at rest, no logs, no client
     exposure).
   - Backup encriptado offline (USB cifrado + passphrase) en custodia del fundador.

3. **Pubkey** embebida en `crates/license/src/keys.rs`:
   ```rust
   pub const PUBKEYS: &[(&str, &[u8; 32])] = &[
       ("k1", &[0x...]),  // primary
       // ("k2", &[...]),  // futuro rotation
   ];
   ```
   Múltiples claves activas a la vez (ADR-0007).

4. **CRL endpoint** `/api/license/crl`:
   - Devuelve JSON firmado: `{ "revoked": ["lic-uuid-1", ...], "issued_at": "...", "sig": "...", "key_id": "k1" }`.
   - Pharma-server lo consume opcionalmente (depende de `[license] crl_url` en
     `config/local.toml`).
   - Refresh cadence: 7 días, fail-closed sólo si el license expira (per
     [ADR-0005](../adr/0005-core-gratis-no-locked-in.md) invariante #6: sin kill-switch
     remoto, core gratis NUNCA se rompe).

---

## 8. Payment flow

```
1. Cliente abre /buy → click "Pro mensual CLP $XX.XXX"
2. Frontend hace POST /api/checkout/mercadopago { tier: 'pro', email: 'farma@x.cl' }
3. Handler crea customer (si no existe) + payment_event(status='pending') + MP preference
4. Frontend redirige a init_point MP
5. Cliente paga en MP
6. MP envía webhook → POST /api/webhook/mercadopago
7. Handler verifica signature MP + idempotency (external_id unique)
8. Si paid → POST /api/license/issue { customerId, tier, microtxKey?, expiresAt? }
9. /api/license/issue:
   - genera tenant_id (UUID v4)
   - construye LicenseJson + canonicalize + ed25519.sign(privkey)
   - INSERT license row con signed_json
   - sendEmail(customer.email, { signed_json_attachment, instrucciones_import })
10. Cliente recibe email con `license.json` adjunto
11. Cliente importa con `pharma license import license.json` → server local activa tier
```

Stripe flow es **idéntico**, sólo cambian webhook signature verification + provider
sdk + endpoint.

**Idempotency**: `payment_event.uniqueIndex(provider, external_id)` previene
double-emit si MP/Stripe re-envían webhook.

---

## 9. Env vars (template `.env.example`)

```bash
# DB
DATABASE_URL=postgresql://...@neon.tech/...?sslmode=require

# Ed25519
LICENSE_SIGNER_PRIVKEY_K1=base64-32-bytes
LICENSE_KEY_ID_PRIMARY=k1

# Stripe
STRIPE_SECRET_KEY=sk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...

# Mercado Pago
MERCADOPAGO_ACCESS_TOKEN=APP_USR-...
MERCADOPAGO_WEBHOOK_SECRET=...  # opcional, MP no firma webhooks por default — usar query_param secret

# Email
RESEND_API_KEY=re_...
RESEND_FROM=licenses@pharma-server-pilot.dev  # subdominio Resend free OK al inicio

# Admin
ADMIN_BASIC_AUTH_USER=admin
ADMIN_BASIC_AUTH_PASS=<random32>

# CRL
CRL_REFRESH_INTERVAL_HOURS=168  # 7 días
```

NUNCA committear `.env` real. `.env.example` SI committeado (sin valores).

---

## 10. Bootstrap checklist (1-2 días, $0)

- [ ] Crear repo `pharma-license-server` privado en `pabloalvarez99`.
- [ ] `npx create-next-app@latest pharma-license-server --typescript --app --src-dir --tailwind --eslint`.
- [ ] Agregar `drizzle-orm @neondatabase/serverless drizzle-kit zod @noble/ed25519 resend stripe mercadopago vitest`.
- [ ] Configurar `tsconfig.json` strict.
- [ ] Crear Neon project (free) → copiar `DATABASE_URL` a `.env.local`.
- [ ] Escribir `src/lib/db/schema.ts` (sección §5).
- [ ] `npx drizzle-kit generate` + `npx drizzle-kit push`.
- [ ] Generar keypair Ed25519 local → `LICENSE_SIGNER_PRIVKEY_K1` en `.env.local`.
- [ ] Copiar pubkey 32 bytes a `crates/license/src/keys.rs` en pharma-server (PR aparte).
- [ ] Implementar `src/lib/crypto/signer.ts` + `canonical.ts` con fixtures cross-repo
      (byte-equal vs `crates/license/tests/fixtures/`).
- [ ] Implementar `src/app/api/license/issue/route.ts`.
- [ ] Implementar `PaymentProvider` interface + `StripeProvider` + `MercadoPagoProvider`.
- [ ] Implementar webhooks (`/api/webhook/stripe`, `/api/webhook/mercadopago`).
- [ ] Implementar landing `/buy` con 4 botones (Free zero-cost path, Pro, Business, Microtx).
- [ ] Configurar Resend → DNS verification subdominio sandbox OK al inicio.
- [ ] Deploy a Vercel → conectar repo GitHub → autopush `main`.
- [ ] Env vars en Vercel dashboard.
- [ ] Smoke test: pagar Stripe test card 4242 4242 4242 4242 → recibir email con license
      JSON adjunto → `pharma license import` → `pharma license status` muestra tier
      correcto.

---

## 11. Cuándo NO usar este blueprint

- **Si vas a vender Enterprise con SLA de 99.9%** — Vercel Hobby + Neon free no garantizan
  SLA. Upgradear stack ANTES de cerrar Enterprise.
- **Si necesitas multi-región DB** — Neon free es región única. Para latencia LATAM cross-país,
  considerar Neon Pro multi-region o Supabase Free.
- **Si el fundador prefiere Cloudflare Workers vs Vercel** — todo el stack es portable
  (drizzle, @noble/ed25519, etc.). Reescribir `app/api/*` como Workers routes es ~1 día.
  Decision lockeable en ADR-0010 si surge la pregunta.

---

## 12. Estado actual

**Hoy 2026-05-27**: este doc es **blueprint**. El repo `pharma-license-server` NO existe.
PRs antiguos #51 (`license-server-scaffold-fase-11a`) y #52
(`cli-license-activate-fase-11b`) en `pharma-server` están **pendientes triage**
(ver `bitacora.md` § ESTADO ACTUAL). Si #51 incluye scaffold del license-server **dentro**
del repo pharma-server, **descartarlo** — violenta ADR-0004 (debe ser repo separado).

Ningún commit consume este blueprint todavía. El primer agente que arranque el bootstrap
debe actualizar este doc § "Estado actual" con el commit SHA inicial del repo nuevo.

## More information

- [ADR-0002](../adr/0002-license-ed25519-offline.md), [ADR-0004](../adr/0004-license-server-separado.md), [ADR-0006](../adr/0006-revocation-strategy-signed-crl.md), [ADR-0007](../adr/0007-key-rotation-licenser.md).
- [ADR-0009](../adr/0009-pilot-payment-provider.md) — orden providers pilot.
- [`zero-cost-launch-plan.md`](./zero-cost-launch-plan.md) §4 — cómo encaja en plan global.
- [`license-architecture.md`](./license-architecture.md) — arquitectura de la capa license
  desde el lado de pharma-server.
- Neon docs: https://neon.tech/docs/introduction
- Vercel free tier: https://vercel.com/pricing
- Resend free tier: https://resend.com/pricing
- Mercado Pago developers CL: https://www.mercadopago.cl/developers
- Stripe Checkout: https://stripe.com/docs/payments/checkout
