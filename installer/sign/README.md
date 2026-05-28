# installer/sign — MSI code signing (pilot phase)

Zero-cost MSI signing for the pilot phase. Self-signed cert + RFC3161 timestamp.

**Decision**: [ADR-0008](../../docs/adr/0008-self-sign-pilot-msi.md). **Plan**:
[zero-cost-launch-plan.md §2](../../docs/strategy/zero-cost-launch-plan.md).

## Why self-signed (and why that's OK for now)

A real Authenticode cert costs $80-600/year (or Azure Trusted Signing $10/mo). Pre-revenue,
that's pure burn. Self-signing is $0 and lets us distribute a pilot MSI **today**. The
trade-off: pilot clients must import the public cert once (15-min assisted onboarding) to
skip the SmartScreen warning. This does **not** scale past ~20 clients — upgrade path below.

## Files

| File | Purpose | Secret? |
|---|---|---|
| `generate-pilot-cert.ps1` | Generate `pilot.pfx` + `pilot.cer` (run once per dev machine) | produces a secret |
| `sign-msi.ps1` | Sign an MSI with `pilot.pfx` + timestamp (run each release) | reads secret |
| `verify-signature.ps1` | Inspect signature/timestamp/trust on a signed MSI | no |
| `import-pilot-cert.ps1` | **CLIENT-SIDE** — import `pilot.cer` to Trusted Publishers (run on pilot machine) | no |
| `pilot.pfx` | PRIVATE signing key — **gitignored, NEVER commit** | YES |
| `pilot.cer` | PUBLIC cert — distributable, committable, ships as release asset | no |

## Operator workflow (dev / release machine)

```powershell
# 1. One time: generate the cert. Password from env (never as plain arg).
$env:PHARMA_CERT_PASSWORD = "<strong-password>"
pwsh installer/sign/generate-pilot-cert.ps1

# 2. Build the MSI (see installer/wix + CLAUDE.md regla #6).
#    cargo wix --package service ... -ext WixFirewallExtension

# 3. Sign it.
$env:PHARMA_CERT_PASSWORD = "<strong-password>"
pwsh installer/sign/sign-msi.ps1 -MsiPath target/wix/pharma-server-0.1.25-x86_64.msi

# 4. Verify (on this machine it'll be untrusted unless pilot.cer imported — that's fine).
pwsh installer/sign/verify-signature.ps1 -MsiPath target/wix/pharma-server-0.1.25-x86_64.msi
```

## Pilot client workflow (15-min assisted onboarding)

1. Client downloads `pilot.cer` from the release mirror (it's public).
2. Client runs (elevated):
   ```powershell
   pwsh installer/sign/import-pilot-cert.ps1 -CerPath <downloaded pilot.cer>
   ```
   Or manually: double-click `pilot.cer` → Install Certificate → Local Machine →
   Trusted Publishers.
3. Client double-clicks the MSI. No SmartScreen warning.

Without the import, the client can still install via SmartScreen "More info → Run anyway",
but the cert import is the smoother path.

## CI integration (release-publisher.yml)

When wiring signing into CI:

1. Store `pilot.pfx` as a base64 GitHub Actions secret (`PILOT_PFX_B64`) and the password
   as `PHARMA_CERT_PASSWORD`.
2. In the workflow, decode the pfx to a temp file, set `PHARMA_CERT_PASSWORD`, call
   `sign-msi.ps1`, then attach both the signed MSI and `pilot.cer` as release assets.
3. **Never** echo the password or commit the pfx.

## Upgrade path (when revenue allows — ADR-0008 §staging)

| Trigger | Action | Cost |
|---|---|---|
| First sale closed | Repackage MSI → MSIX, publish via Microsoft Store dev account | $19 one-time |
| Manual onboarding >20 clients | Migrate to **Azure Trusted Signing**, automate in CI | $9.99/mo |
| Mainstream launch | EV cert on USB HSM (or stay on Azure if CI matters more) | $400-600/yr |

When you upgrade, update [ADR-0008](../../docs/adr/0008-self-sign-pilot-msi.md) status and
[zero-cost-launch-plan.md §2.3](../../docs/strategy/zero-cost-launch-plan.md).

## Security notes

- `pilot.pfx` is a code-signing private key. Treat it like any other secret: gitignored,
  rotate if leaked, store the password in a password manager (not in the repo).
- The timestamp (`/tr`) is mandatory — without it the signature dies with the cert and
  clients can't reinstall after the 3-year validity ends.
- Self-signed certs do **not** accumulate Microsoft reputation. Upgrading to MSIX/EV later
  requires pilot clients to re-trust the new cert. Acceptable at low volume.
