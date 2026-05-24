---
title: pharma-license-server — Implementation spec (Fase 11a)
status: Draft v1 — awaiting separate-repo init
date: 2026-05-24
owners: pabloalvarez99 (fundador)
related_adrs:
  - ADR-0002 (Ed25519 offline)
  - ADR-0003 (Webpay first)
  - ADR-0004 (license-server separado)
  - ADR-0006 (revocation CRL signed)
  - ADR-0007 (key rotation)
target_repo: pabloalvarez99/pharma-license-server
implements_phase: Fase 11a
companion_of:
  - pabloalvarez99/pharma-server (this repo) — verifier (`crates/license`)
last_review: 2026-05-24
---

# pharma-license-server — Implementation spec

> **Documento de implementación.** Define la skeleton técnica del servicio que vive en
> el repo separado `pabloalvarez99/pharma-license-server` ([ADR-0004](../adr/0004-license-server-separado.md)).
> **No se implementa en este repo.** Las shapes JSON deben matchear bit-a-bit
> al verifier offline `crates/license` (ver [`license-architecture.md`](./license-architecture.md) §2).

---

## 1. Scope

`pharma-license-server` es el único componente cloud del ecosistema pharma-server. Sus
responsabilidades:

1. **Mintear licenses** Ed25519-firmadas a tenants que pagaron (tier sub + microtx).
2. **Publicar CRLs** firmados ([ADR-0006](../adr/0006-revocation-strategy-signed-crl.md))
   cuando hay refunds/chargebacks/fraude/compromiso.
3. **Recibir webhooks de pago** (Webpay primary, Stripe/Khipu/MP follow-ups) y disparar
   issue idempotente.
4. **Servir un web admin** mínimo: checkout, listar licenses propias, descargar `.lic`, status.
5. **Custodiar la signing key** del licenser (KMS, nunca exportable, rotación per
   [ADR-0007](../adr/0007-key-rotation-licenser.md)).

**Fuera de scope** (delegado o futuro):
- Procesamiento de pago (lo hace Webpay/Stripe; este server sólo recibe webhooks).
- Emisión de boleta SII (delegado a provider DTE — SimpleAPI/Bsale; ADR-0008 pendiente).
- Telemetría de uso del nodo (opt-in, pipeline separado).
- Federation marketplace (Fase 13, otro servicio).

---

## 2. Stack y deployment

| Capa | Tecnología | Razón |
|---|---|---|
| Runtime | Node.js 22 LTS | Webpay SDK oficial, Stripe SDK oficial, ecosystem maduro. |
| Framework | **Next.js 15 App Router** | Server Actions para admin UI + API Routes para webhooks/issue. Vercel-native. |
| DB | **Postgres** (Neon serverless o Vercel Postgres) | Transacciones ACID para issue idempotente. Multi-region read replicas si crece. |
| ORM | Prisma 6 o Drizzle ORM | Migrations versionadas + tipos generados. |
| Auth admin | Clerk (Vercel Marketplace) **o** admin token simple (HMAC bearer) | Clerk si multi-operador; token simple para MVP solo-founder. |
| Crypto | `@noble/ed25519` + canonical-JSON helper port | Matchea `crates/agent/canonical.rs` byte-a-byte. |
| KMS | AWS KMS o GCP Cloud KMS (key non-exportable) | Privada nunca sale del HSM. Para MVP: env var sealed, **`.secrets/` gitignored, ops backup encrypted**. |
| CDN | Vercel Edge | Licenses `.lic` + CRLs servidos con `Cache-Control: immutable` o `s-maxage=60`. |
| Deploy | Vercel (preview por PR + producción en `main`) | CI/CD nativo. Distinto pipeline del MSI build de pharma-server. |
| Observability | Vercel Analytics + structured logs JSON | Webhook idempotency dashboard. |

**No hay paridad de stack con pharma-server** (Rust + SurrealDB). Repos separados por
diseño ([ADR-0004](../adr/0004-license-server-separado.md)).

---

## 3. Endpoints (contrato externo)

Todas las rutas viven bajo `app/api/v1/...`. Shapes JSON **deben** matchear al verifier en
`crates/license/src/schema.rs:54-74`.

### 3.1 `POST /api/v1/issue` — emite license firmada (admin only)

**Auth**: `Authorization: Bearer <ADMIN_TOKEN>` (Clerk JWT o HMAC token según deployment).

**Request body**:
```json
{
  "tenant_id": "uuid-v4",
  "tier": "free | pro | business | enterprise",
  "features": ["reports.margins_daily", "integrations.sii_dte_auto"],
  "bought_addons": [
    {
      "addon_id": "branding_pack_v1",
      "feature_keys": ["branding.custom_logo", "branding.themes"],
      "purchased_at": "2026-05-24T14:00:00Z",
      "order_id": "ord_..."
    }
  ],
  "seat_count": 3,
  "expires_at": "2027-05-24T14:00:00Z",
  "metadata": {
    "billing_cycle": "yearly",
    "support_sla_hours": 24,
    "white_label": false
  },
  "idempotency_key": "iss_<orderId>_<timestamp>"
}
```

**Behaviour**:
1. Resolve current active `key_id` from `key_pairs` table (status=`active`).
2. Build the canonical `License` JSON (matching `crates/license::schema::License`).
3. Compute canonical bytes (RFC8785-lite, identical to `agent::canonical::canonical_bytes`)
   over the document **without** the `signature` field.
4. Sign with KMS-held private key → base64-stdpad.
5. Insert the signed document into `licenses` (history append-only).
6. Insert a row in `webhook_events`/`issue_events` for audit + idempotency.
7. Persist the `.lic` file to Vercel Blob / CDN under `licenses/{license_id}.lic`.
8. Return `{ license_id, lic_url, signed_at }`.

**Idempotency**: same `idempotency_key` returns the previously-minted license unchanged.

**Response 201**:
```json
{
  "license_id": "lic_01HX5...",
  "lic_url": "https://cdn.pharma-server.cl/licenses/lic_01HX5....lic",
  "signed_at": "2026-05-24T14:00:01Z",
  "key_id": "lk-2026-01"
}
```

**Errors**: 401 (auth), 422 (schema), 409 (idempotency conflict), 500 (KMS down).

### 3.2 `POST /api/v1/revoke` — revoca una license (admin only)

**Auth**: idem.

**Request body**:
```json
{
  "license_id": "lic_01HX5...",
  "reason": "refund | chargeback | fraud | key_compromise | other",
  "notes": "ticket #1234"
}
```

**Behaviour**:
1. Mark `licenses` row as `revoked_at=now, revoke_reason=<reason>`.
2. Append to `revocations` table.
3. Bump CRL version: `crl_version = MAX(crl_version) + 1`.
4. Build the new `crl-v{N}.json` per [ADR-0006](../adr/0006-revocation-strategy-signed-crl.md)
   §Diseño, sign Ed25519, publish to CDN at `/crl/crl-v{N}.json` (immutable) and update
   `/crl/crl-latest.json` redirect.
5. Optionally email the tenant.

**Response 200**: `{ revoked_at, crl_version_published }`.

### 3.3 `GET /api/v1/crl/v1` — CRL público firmado

**Auth**: none (público).

**Behaviour**: respond with the latest signed CRL JSON
([ADR-0006](../adr/0006-revocation-strategy-signed-crl.md) schema). `Cache-Control:
public, s-maxage=60, stale-while-revalidate=300`.

Also expose:
- `GET /api/v1/crl/v{N}` (immutable, cached forever) — version-pinned consumption.
- `GET /api/v1/crl/snapshot/v{N}` — monthly full snapshot for cold-start nodes.

### 3.4 `POST /api/v1/webhooks/webpay` — confirmación de pago Webpay

**Auth**: Transbank signature header (HMAC over body).

**Behaviour**:
1. Verify Transbank signature. Reject `401` if invalid.
2. Idempotency check: ignore if `webhook_events.transbank_tx_id` already processed.
3. Resolve tenant + order from `orders` table.
4. **Internally call** `/api/v1/issue` with the derived license payload.
5. Persist event row.
6. Return `200` quickly (Webpay retries on timeout).

**Future siblings** (Fase 11b/c/d):
- `POST /api/v1/webhooks/stripe` (signed via `stripe-signature` header).
- `POST /api/v1/webhooks/khipu`.
- `POST /api/v1/webhooks/mercadopago`.

### 3.5 Admin UI (web)

Pages bajo `app/admin/...`:
- `app/admin/licenses` — list, filter by tenant, view raw `.lic`.
- `app/admin/licenses/[id]` — detail + revoke button.
- `app/admin/keys` — list `key_pairs`, current active, rotation history (manual rotate
  requires KMS console access, this is read-only audit).
- `app/admin/webhooks` — recent webhook events, replay button for failed/missed.

**No customer-facing checkout in 11a**. Webpay/Stripe checkout pages embedded via SDK
arrive in Fase 11b/c.

---

## 4. Database schema

Postgres. Migrations append-only (same rule as `pharma-server`).

### 4.1 `tenants`
```sql
CREATE TABLE tenants (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  pharma_tenant_id UUID NOT NULL,                 -- matches pharma-server's tenant
  email TEXT NOT NULL,
  display_name TEXT,
  country TEXT DEFAULT 'CL',
  created_at TIMESTAMPTZ DEFAULT NOW(),
  UNIQUE(pharma_tenant_id)
);
```

### 4.2 `licenses` (history append-only)
```sql
CREATE TABLE licenses (
  license_id TEXT PRIMARY KEY,                    -- ULID, matches the .lic JSON
  tenant_id UUID REFERENCES tenants(id) NOT NULL,
  tier TEXT NOT NULL,                             -- free|pro|business|enterprise
  schema_version INT NOT NULL DEFAULT 1,
  signed_json JSONB NOT NULL,                     -- the FULL .lic JSON (signature included)
  key_id TEXT NOT NULL REFERENCES key_pairs(key_id),
  issued_at TIMESTAMPTZ NOT NULL,
  expires_at TIMESTAMPTZ,                         -- NULL allowed only for free
  revoked_at TIMESTAMPTZ,
  revoke_reason TEXT,
  idempotency_key TEXT UNIQUE,
  order_id TEXT,
  created_at TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX idx_licenses_tenant ON licenses(tenant_id, issued_at DESC);
CREATE INDEX idx_licenses_revoked ON licenses(revoked_at) WHERE revoked_at IS NOT NULL;
```

### 4.3 `revocations`
```sql
CREATE TABLE revocations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  license_id TEXT REFERENCES licenses(license_id) NOT NULL,
  revoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  reason TEXT NOT NULL,                           -- refund|chargeback|fraud|key_compromise|other
  notes TEXT,
  crl_version_published INT NOT NULL              -- the crl-vN.json that first listed this
);
```

### 4.4 `key_pairs`
```sql
CREATE TABLE key_pairs (
  key_id TEXT PRIMARY KEY,                        -- e.g. lk-2026-01
  did TEXT NOT NULL,                              -- did:pharma:<bs58>
  pubkey_base64 TEXT NOT NULL,                    -- raw 32-byte Ed25519 pubkey base64-encoded
  kms_arn TEXT,                                   -- HSM/KMS reference, NULL if envsealed dev
  status TEXT NOT NULL,                           -- active|retired|compromised
  activated_at TIMESTAMPTZ NOT NULL,
  retired_at TIMESTAMPTZ
);
```

### 4.5 `webhook_events` (idempotency + audit)
```sql
CREATE TABLE webhook_events (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  provider TEXT NOT NULL,                         -- webpay|stripe|khipu|mercadopago
  external_id TEXT NOT NULL,                      -- provider's transaction id
  payload JSONB NOT NULL,                         -- raw body (redacted of PII)
  status TEXT NOT NULL,                           -- received|processed|failed
  license_id TEXT REFERENCES licenses(license_id),
  received_at TIMESTAMPTZ DEFAULT NOW(),
  processed_at TIMESTAMPTZ,
  error TEXT,
  UNIQUE(provider, external_id)
);
```

### 4.6 `crl_versions`
```sql
CREATE TABLE crl_versions (
  crl_version INT PRIMARY KEY,
  previous_version INT REFERENCES crl_versions(crl_version),
  signed_json JSONB NOT NULL,                     -- full signed CRL doc per ADR-0006
  key_id TEXT NOT NULL REFERENCES key_pairs(key_id),
  published_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## 5. Key management

### 5.1 MVP (local dev, pre-KMS)

- Generate Ed25519 keypair via `scripts/license-server/gen-keypair.ps1` (this repo) or
  the bootstrap script (companion repo).
- Private key lives in `.secrets/licenser-sk.pem` (gitignored). **Backup ops, never commit.**
- Public key + `did:pharma:<bs58>` published to:
  - `key_pairs` row with `status=active`.
  - `pharma-server` repo: hardcoded into `crates/license/src/keys.rs::LICENSER_KEYS`.
  - Public DID document at `did:web:pharma-server.cl/.well-known/did.json`.

### 5.2 Production (Fase 11b+)

- KMS (AWS / GCP) holds the private key non-exportable.
- All sign operations go through KMS API (`kms:Sign` IAM scope).
- Audit log of every sign retained ≥ 7 years (compliance + post-mortem).

### 5.3 Rotation

Procedure ratified in [ADR-0007](../adr/0007-key-rotation-licenser.md):
1. Generate new keypair in KMS. Insert new row in `key_pairs` (`status=active`,
   prior row demoted to `status=retired`).
2. Publish new release of `pharma-server` MSI containing both keys in `LICENSER_KEYS`.
3. Deploy MSI to 100% of fleet (target: 60 days).
4. Switch license-server signer to new `key_id`.
5. Keep old key in `LICENSER_KEYS` for 24 months (validates pre-rotation licenses).

### 5.4 Compromise (emergency)

Procedure in [ADR-0007](../adr/0007-key-rotation-licenser.md) §"Procedimiento de
rotación de emergencia":
- KMS revoke `kms:Sign` permission on compromised key.
- Publish global CRL revoking all licenses signed with that `key_id` (`reason=
  "key_compromise"`).
- Emergency MSI release with new key + cherry-picked CRL pre-baked.
- Free re-issue to legit tenants (cron job iterating `licenses` joined to `tenants`).

---

## 6. Webhook security

| Provider | Signature header | Verification |
|---|---|---|
| Webpay | `tbk-signature` (HMAC SHA-256 with shared secret) | Compare against env `WEBPAY_WEBHOOK_SECRET`. |
| Stripe | `stripe-signature` | Use `stripe.webhooks.constructEvent`. |
| Khipu | `x-khipu-signature` (RSA SHA-256, pubkey published by Khipu) | Verify with khipu pubkey. |
| Mercado Pago | `x-signature` (HMAC) | Standard MP verify. |

All webhook handlers must:
1. Verify signature first; reject `401` otherwise.
2. Replay-protect via `webhook_events.UNIQUE(provider, external_id)`.
3. Return `2xx` within 5s. Heavy work goes to a `setImmediate` / queue worker.

---

## 7. Cross-repo contract testing

Critical: the verifier in this repo (`crates/license`) must accept what
`pharma-license-server` mints, bit-for-bit. To prevent drift:

1. Publish a versioned JSON schema in the companion repo:
   `pharma-license-server/contracts/license-v1.schema.json`.
2. Consume it from `crates/license/tests/contract_v1.rs` (future) via `schemars`/`jsonschema`.
3. Each PR that touches the schema in either repo **must** update both sides + bump
   `schema_version` if breaking.
4. Canonical-JSON helper port (`@noble/canonical-json` or hand-written) covered by a
   suite that compares output byte-by-byte to `agent::canonical::canonical_bytes` for a
   fixed corpus.

---

## 8. Phased rollout (in companion repo)

| Step | Deliverable | Phase |
|---|---|---|
| **11a.1** | Scaffold `pharma-license-server` repo. Next.js 15 App Router + Postgres + Prisma. CI to Vercel preview. | This spec |
| **11a.2** | DB migrations (§4). | This spec |
| **11a.3** | `POST /issue` + admin-token auth + KMS-or-dev signing. | This spec |
| **11a.4** | `POST /revoke` + CRL publish. | This spec |
| **11a.5** | `GET /crl/v{N}` + immutable CDN headers. | This spec |
| **11b** | Webpay Oneclick sub + `POST /webhooks/webpay` + admin checkout page. | ADR-0003 |
| **11c** | Stripe Checkout (microtx international cards). | Fase 11c |
| **11d** | CRL distribution + key rotation rehearsal in staging. | ADR-0006 + ADR-0007 |
| **11e** | DTE provider integration (SII boleta electrónica). | ADR-0008 (pending) |

---

## 9. What this repo (`pharma-server`) provides today

Already merged on `feature/erp-parity`:
- `crates/license` verifier (Ed25519 offline, canonical JSON) — Fase 10a.
- `ApiError::payment_required` 402 + `From<GateError>` — Fase 10b.
- `pharma license` CLI (`import|status|features|verify|export|clear|reload`) — Fase 10c.
- 1 endpoint POC gated (`GET /api/v1/reports/margins-daily`) — Fase 10d.
- Hot-reload endpoint `POST /api/v1/admin/license/reload` — Fase 10 cola.

The companion repo only needs to mint JSON that this verifier already accepts.

---

## 10. References

- [`license-architecture.md`](./license-architecture.md) — schema, gating, refresh, CRL,
  rotation, feature keys catalog.
- [ADR-0002](../adr/0002-license-ed25519-offline.md) — primitive choice.
- [ADR-0003](../adr/0003-payments-webpay-first.md) — Webpay primary.
- [ADR-0004](../adr/0004-license-server-separado.md) — separate repo rationale.
- [ADR-0006](../adr/0006-revocation-strategy-signed-crl.md) — CRL design.
- [ADR-0007](../adr/0007-key-rotation-licenser.md) — multi-key rotation.
- [`payments-cl.md`](./payments-cl.md) — rails comparativa CL.
- [`scaling-architecture.md`](./scaling-architecture.md) — CDN, multi-region, telemetry pipeline.
- `crates/license/src/schema.rs:54-74` — verifier-side schema (this repo).
- `crates/license/src/verify.rs:33-89` — sign-verify flow (mirror in JS for the server).
- `crates/agent/src/canonical.rs` — canonical-JSON algorithm port target.

---

## 11. Open questions (for the companion repo)

- Auth admin para 11a.1 dev/MVP: Clerk vs. simple HMAC bearer? Recommend HMAC bearer
  for solo-founder MVP; migrate to Clerk in 11b when first operator hire happens.
- Postgres host: Neon (better cold-start + branching) vs. Vercel Postgres (less moving
  parts). Recommend Neon for branching per PR.
- DTE provider (boleta SII): SimpleAPI vs. Bsale vs. self-host. Open ADR-0008.
- `.lic` distribution: Vercel Blob (cheap) vs. dedicated CDN (R2 + Cloudflare). MVP: Blob.
