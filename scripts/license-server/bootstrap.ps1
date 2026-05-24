<#
.SYNOPSIS
  Bootstrap a placeholder pharma-license-server repo (companion to pharma-server).

.DESCRIPTION
  Given an EMPTY target directory, scaffold the placeholder layout for
  pabloalvarez99/pharma-license-server per docs/strategy/license-server-spec.md.

  This is intentionally a PLACEHOLDER — no real Next.js init runs. It seeds:
    - .git repo
    - README.md (linking back to the spec in pharma-server)
    - .gitignore (Next.js + secrets)
    - .secrets/ (gitignored) for the dev signing key
    - app/, contracts/, prisma/, .github/workflows/ dirs (empty + .gitkeep)
    - next-steps printed at the end

.PARAMETER TargetDir
  Path to the new repo root. Must not exist or must be empty.

.EXAMPLE
  pwsh ./scripts/license-server/bootstrap.ps1 -TargetDir C:\Users\me\code\pharma-license-server

.NOTES
  No real project generation. Run the printed `pnpm dlx create-next-app` step yourself.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$TargetDir
)

$ErrorActionPreference = 'Stop'

if ((Test-Path $TargetDir) -and (Get-ChildItem -Path $TargetDir -Force -ErrorAction SilentlyContinue)) {
    Write-Error "error: $TargetDir exists and is non-empty. Refusing."
    exit 65
}

New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
Push-Location $TargetDir
try {

    Write-Host '[1/6] git init'
    & git init -q -b main
    & git commit --allow-empty -q -m 'chore: initial commit (pharma-license-server)'

    Write-Host '[2/6] .gitignore'
    @'
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
'@ | Set-Content -Encoding utf8 -Path .gitignore

    Write-Host '[3/6] README.md'
    @'
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

## Bootstrap (real, not this placeholder)

```bash
pnpm dlx create-next-app@latest . --ts --app --src-dir=false --tailwind --eslint --import-alias='@/*'
pnpm add @noble/ed25519 @prisma/client zod
pnpm add -D prisma
pnpm prisma init --datasource-provider postgresql
```

## Dev signing key

```powershell
# from pharma-server checkout:
pwsh ../pharma-server/scripts/license-server/gen-keypair.ps1 -OutDir .secrets
```

## CI/CD

- `main` -> production Vercel deploy.
- PR -> preview Vercel deploy.
- Migrations gated behind `prisma migrate deploy` in deploy step.

## License

Private. Contains commercial logic + signing infra.
'@ | Set-Content -Encoding utf8 -Path README.md

    Write-Host '[4/6] directory skeleton'
    $dirs = @(
        'app/api/v1/issue',
        'app/api/v1/revoke',
        'app/api/v1/webhooks/webpay',
        'app/api/v1/webhooks/stripe',
        'app/api/v1/crl/v1',
        'app/admin/licenses',
        'contracts',
        'prisma',
        '.secrets',
        '.github/workflows'
    )
    foreach ($d in $dirs) {
        New-Item -ItemType Directory -Force -Path $d | Out-Null
    }
    $gitkeeps = @(
        'app/api/v1/issue/.gitkeep',
        'app/api/v1/revoke/.gitkeep',
        'app/api/v1/webhooks/webpay/.gitkeep',
        'app/api/v1/webhooks/stripe/.gitkeep',
        'app/api/v1/crl/v1/.gitkeep',
        'app/admin/licenses/.gitkeep',
        'contracts/.gitkeep',
        'prisma/.gitkeep',
        '.github/workflows/.gitkeep'
    )
    foreach ($f in $gitkeeps) {
        if (-not (Test-Path $f)) { New-Item -ItemType File -Path $f | Out-Null }
    }

    @'
# .secrets/ — DO NOT COMMIT

This directory is gitignored. It holds:
- `licenser-sk.pem` — Ed25519 private signing key (dev).
- `licenser-pk.b58` — base58-encoded pubkey.
- `licenser-did.txt` — DID string `did:pharma:<bs58>`.

Production deployments use KMS instead (AWS KMS / GCP Cloud KMS) — see
`docs/strategy/license-server-spec.md` §5 in the pharma-server repo.

Generate via:
  pwsh ../pharma-server/scripts/license-server/gen-keypair.ps1 -OutDir .
'@ | Set-Content -Encoding utf8 -Path .secrets/README.md

    Write-Host '[5/6] placeholder contract'
    @'
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
'@ | Set-Content -Encoding utf8 -Path contracts/license-v1.schema.json

    Write-Host '[6/6] initial commit'
    & git add -A
    & git commit -q -m @'
chore: skeleton from pharma-server bootstrap

Per docs/strategy/license-server-spec.md in pharma-server@feature/erp-parity.
Placeholder layout only. Real Next.js init pending (see README.md).
'@

    Write-Host ''
    Write-Host '================================================================================'
    Write-Host ' bootstrap done — pharma-license-server scaffolded (placeholder)'
    Write-Host '================================================================================'
    Write-Host ''
    Write-Host 'Next steps (RUN INSIDE the new repo):'
    Write-Host ''
    Write-Host '  1. Real Next.js init:'
    Write-Host "       pnpm dlx create-next-app@latest . --ts --app --src-dir=false --tailwind --eslint --import-alias='@/*'"
    Write-Host ''
    Write-Host '  2. Install deps:'
    Write-Host '       pnpm add @noble/ed25519 @prisma/client zod'
    Write-Host '       pnpm add -D prisma'
    Write-Host ''
    Write-Host '  3. Prisma:'
    Write-Host '       pnpm prisma init --datasource-provider postgresql'
    Write-Host '       # translate spec §4 tables into prisma/schema.prisma'
    Write-Host '       pnpm prisma migrate dev --name init'
    Write-Host ''
    Write-Host '  4. Generate dev keypair (from pharma-server checkout):'
    Write-Host '       pwsh ../pharma-server/scripts/license-server/gen-keypair.ps1 -OutDir .secrets'
    Write-Host ''
    Write-Host '  5. Update pharma-server crates/license/src/keys.rs::LICENSER_KEYS with the new'
    Write-Host '     (key_id, did) tuple from .secrets/licenser-did.txt.'
    Write-Host ''
    Write-Host '  6. Create GitHub repo and push:'
    Write-Host '       gh repo create pabloalvarez99/pharma-license-server --private --source=. --push'
    Write-Host ''
    Write-Host '  7. Link to Vercel:'
    Write-Host '       vercel link'
    Write-Host '       vercel env add WEBPAY_WEBHOOK_SECRET production'
    Write-Host '       vercel env add ADMIN_TOKEN production'
    Write-Host '       vercel env add DATABASE_URL production'
    Write-Host ''
    Write-Host 'Spec source of truth:'
    Write-Host '  https://github.com/pabloalvarez99/pharma-server/blob/feature/erp-parity/docs/strategy/license-server-spec.md'
    Write-Host '================================================================================'

}
finally {
    Pop-Location
}
