#!/usr/bin/env bash
# pharma-license-server bootstrap (POSIX / Git Bash on Windows)
#
# Purpose: given an EMPTY target directory, scaffold the placeholder layout for
# pabloalvarez99/pharma-license-server per docs/strategy/license-server-spec.md.
# This is intentionally a PLACEHOLDER — no real Next.js init runs. It seeds:
#   - .git repo
#   - README.md (linking back to the spec in pharma-server)
#   - .gitignore (Next.js + secrets)
#   - .secrets/ (gitignored) for the dev signing key
#   - app/, contracts/, prisma/, .github/workflows/ dirs (empty + .gitkeep)
#   - next steps printed at the end
#
# Run from anywhere. Pass target dir as $1.
# Example:
#   ./scripts/license-server/bootstrap.sh ~/code/pharma-license-server

set -euo pipefail

TARGET="${1:-}"
if [[ -z "$TARGET" ]]; then
  echo "usage: $0 <target-dir>" >&2
  echo "  target-dir must NOT exist or must be empty." >&2
  exit 64
fi

if [[ -e "$TARGET" ]] && [[ -n "$(ls -A "$TARGET" 2>/dev/null || true)" ]]; then
  echo "error: $TARGET exists and is non-empty. Refusing." >&2
  exit 65
fi

mkdir -p "$TARGET"
cd "$TARGET"

echo "[1/6] git init"
git init -q -b main
git commit --allow-empty -q -m "chore: initial commit (pharma-license-server)"

echo "[2/6] .gitignore"
cat > .gitignore <<'EOF'
# Node / Next
node_modules/
.next/
out/
dist/
*.log
.env
.env.local
.env.*.local

# Vercel
.vercel/

# Prisma
prisma/migrations/dev.db*

# Secrets — NEVER commit licenser private keys
.secrets/
*.pem
*.key

# OS / IDE
.DS_Store
.idea/
.vscode/
*.swp
EOF

echo "[3/6] README.md"
cat > README.md <<'EOF'
# pharma-license-server

> Licenser / billing companion service for [`pharma-server`](https://github.com/pabloalvarez99/pharma-server).
> Mints Ed25519-signed `.lic` files, publishes CRLs, ingests payment webhooks.

**Spec (canonical):** see
[`docs/strategy/license-server-spec.md`](https://github.com/pabloalvarez99/pharma-server/blob/feature/erp-parity/docs/strategy/license-server-spec.md)
in the `pharma-server` repo.

This repo is the **separate companion repo** as per
[ADR-0004](https://github.com/pabloalvarez99/pharma-server/blob/feature/erp-parity/docs/adr/0004-license-server-separado.md).

## Stack

- Next.js 15 App Router (Node.js 22 LTS)
- Postgres (Neon serverless or Vercel Postgres)
- Prisma 6 ORM
- `@noble/ed25519` for signing
- KMS-held private key (AWS/GCP) in prod; envsealed `.secrets/licenser-sk.pem` in dev
- Deployed to Vercel

## Endpoints (target)

| Method | Path | Purpose |
|---|---|---|
| POST | `/api/v1/issue` | Admin: mint a signed `.lic` |
| POST | `/api/v1/revoke` | Admin: revoke + publish new CRL |
| GET | `/api/v1/crl/v1` | Public: latest signed CRL |
| GET | `/api/v1/crl/v{N}` | Public: specific CRL version (immutable) |
| POST | `/api/v1/webhooks/webpay` | Webhook: Transbank payment confirmation |
| POST | `/api/v1/webhooks/stripe` | Webhook: Stripe payment confirmation |

## Schema contract

License JSON shape MUST match the verifier in `pharma-server/crates/license/src/schema.rs`.
See `contracts/license-v1.schema.json` (publish here once it exists).

## Layout

```
app/
  admin/                  # admin UI pages (Next.js App Router)
  api/v1/
    issue/route.ts
    revoke/route.ts
    crl/
      v1/route.ts
      v[version]/route.ts
    webhooks/
      webpay/route.ts
      stripe/route.ts
contracts/
  license-v1.schema.json  # cross-repo contract — consumed by pharma-server tests
prisma/
  schema.prisma           # tables: tenants, licenses, revocations, key_pairs, webhook_events, crl_versions
.secrets/                 # gitignored, holds dev signing key
.github/workflows/        # CI: lint, test, vercel deploy preview
```

## Bootstrap (real, not this placeholder)

```bash
pnpm dlx create-next-app@latest . --ts --app --src-dir=false --tailwind --eslint --import-alias='@/*'
pnpm add @noble/ed25519 @prisma/client zod
pnpm add -D prisma
pnpm prisma init --datasource-provider postgresql
# then translate the §4 spec tables into prisma/schema.prisma
```

## Dev signing key

```bash
# from pharma-server checkout:
pwsh ./scripts/license-server/gen-keypair.ps1 -OutDir ../pharma-license-server/.secrets
```

Outputs:
- `.secrets/licenser-sk.pem` — keep offline backup. NEVER commit.
- `.secrets/licenser-pk.b58` — base58 pubkey (matches DID body).
- `.secrets/licenser-did.txt` — full `did:pharma:<bs58>`.

Copy the DID into `pharma-server/crates/license/src/keys.rs::LICENSER_KEYS` with a new
`key_id` (e.g. `lk-2026-01`).

## CI/CD

- `main` → production Vercel deploy.
- PR → preview Vercel deploy.
- Migrations gated behind `prisma migrate deploy` in deploy step.

## License

Private. Contains commercial logic + signing infra. Do not open-source without
removing webhook secrets, KMS arns, customer data.
EOF

echo "[4/6] directory skeleton"
mkdir -p app/api/v1/{issue,revoke,webhooks/webpay,webhooks/stripe,crl/v1}
mkdir -p app/admin/licenses
mkdir -p contracts
mkdir -p prisma
mkdir -p .secrets
mkdir -p .github/workflows

# .gitkeep so empty dirs are tracked
touch app/api/v1/issue/.gitkeep
touch app/api/v1/revoke/.gitkeep
touch app/api/v1/webhooks/webpay/.gitkeep
touch app/api/v1/webhooks/stripe/.gitkeep
touch app/api/v1/crl/v1/.gitkeep
touch app/admin/licenses/.gitkeep
touch contracts/.gitkeep
touch prisma/.gitkeep
touch .github/workflows/.gitkeep

# .secrets must NOT be tracked, but explain its purpose:
cat > .secrets/README.md <<'EOF'
# .secrets/ — DO NOT COMMIT

This directory is gitignored. It holds:
- `licenser-sk.pem` — Ed25519 private signing key (dev).
- `licenser-pk.b58` — base58-encoded pubkey.
- `licenser-did.txt` — DID string `did:pharma:<bs58>`.

Production deployments use KMS instead (AWS KMS / GCP Cloud KMS) — see
`docs/strategy/license-server-spec.md` §5 in the pharma-server repo.

Generate via:
  pwsh ../pharma-server/scripts/license-server/gen-keypair.ps1 -OutDir .

Or POSIX/openssl:
  ../pharma-server/scripts/license-server/gen-keypair.sh -o .
EOF

echo "[5/6] placeholder contract"
cat > contracts/license-v1.schema.json <<'EOF'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://pharma-server.cl/contracts/license-v1.schema.json",
  "title": "License v1",
  "description": "Placeholder. Must match pharma-server/crates/license/src/schema.rs:54-74 byte-for-byte once filled in.",
  "type": "object",
  "required": [
    "schema_version", "license_id", "tenant_id", "tier", "features",
    "seat_count", "issued_at", "issuer_did", "key_id", "signature"
  ],
  "properties": {
    "schema_version": { "const": 1 },
    "license_id": { "type": "string" },
    "tenant_id": { "type": "string", "format": "uuid" },
    "tier": { "enum": ["free", "pro", "business", "enterprise"] },
    "features": { "type": "array", "items": { "type": "string" } },
    "bought_addons": { "type": "array" },
    "seat_count": { "type": "integer", "minimum": 1 },
    "issued_at": { "type": "string", "format": "date-time" },
    "expires_at": { "type": ["string", "null"], "format": "date-time" },
    "issuer_did": { "type": "string", "pattern": "^did:pharma:" },
    "key_id": { "type": "string" },
    "signature": { "type": "string", "contentEncoding": "base64" },
    "metadata": { "type": "object" }
  }
}
EOF

echo "[6/6] initial commit"
git add -A
git commit -q -m "chore: skeleton from pharma-server bootstrap

Per docs/strategy/license-server-spec.md in pharma-server@feature/erp-parity.
Placeholder layout only. Real Next.js init pending (see README.md)."

cat <<'NEXT'

================================================================================
 bootstrap done — pharma-license-server scaffolded (placeholder)
================================================================================

Next steps (RUN INSIDE the new repo):

  1. Real Next.js init:
       pnpm dlx create-next-app@latest . --ts --app --src-dir=false --tailwind --eslint --import-alias='@/*'

  2. Install deps:
       pnpm add @noble/ed25519 @prisma/client zod
       pnpm add -D prisma

  3. Prisma:
       pnpm prisma init --datasource-provider postgresql
       # translate spec §4 tables into prisma/schema.prisma
       pnpm prisma migrate dev --name init

  4. Generate dev keypair (from pharma-server checkout):
       pwsh ../pharma-server/scripts/license-server/gen-keypair.ps1 -OutDir .secrets

  5. Update pharma-server crates/license/src/keys.rs::LICENSER_KEYS with the new
     (key_id, did) tuple from .secrets/licenser-did.txt.

  6. Create GitHub repo and push:
       gh repo create pabloalvarez99/pharma-license-server --private --source=. --push

  7. Link to Vercel:
       vercel link
       vercel env add WEBPAY_WEBHOOK_SECRET production
       vercel env add ADMIN_TOKEN production
       vercel env add DATABASE_URL production

Spec source of truth:
  https://github.com/pabloalvarez99/pharma-server/blob/feature/erp-parity/docs/strategy/license-server-spec.md
================================================================================
NEXT
