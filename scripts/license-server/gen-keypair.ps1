<#
.SYNOPSIS
  Generate an Ed25519 keypair for the licenser (dev) and derive its DID.

.DESCRIPTION
  Produces three files in -OutDir:
    licenser-sk.pem    Ed25519 private key (PKCS#8 PEM). NEVER commit. Ops backup.
    licenser-pk.pem    Ed25519 public key (SubjectPublicKeyInfo PEM).
    licenser-pk.raw    Raw 32-byte public key (binary).
    licenser-pk.b58    Base58 of raw pubkey (matches DID body).
    licenser-did.txt   "did:pharma:<bs58>" (single line, no trailing newline).

  Uses openssl 3.x (`openssl genpkey -algorithm ed25519`). If openssl is missing
  OR raw-pubkey extraction fails (older openssl), prints fallback instructions
  pointing at a future `cargo run -p license-tools` once that binary exists.

  Match this output with pharma-server/crates/license/src/keys.rs::LICENSER_KEYS
  by appending `(key_id, did)` and tagging the relevant version constant.

.PARAMETER OutDir
  Directory to write the keypair into. Created if missing.

.PARAMETER KeyId
  Identifier embedded later in the license JSON. Default: lk-YYYY-01
  (auto-derived from the current year).

.EXAMPLE
  pwsh ./scripts/license-server/gen-keypair.ps1 -OutDir ../pharma-license-server/.secrets

.EXAMPLE
  pwsh ./scripts/license-server/gen-keypair.ps1 -OutDir .secrets -KeyId lk-2026-02

.NOTES
  This script intentionally does NOT install anything. If openssl is missing,
  it documents the manual cargo path and exits non-zero.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$OutDir,

    [string]$KeyId = ''
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($KeyId)) {
    $KeyId = "lk-$(Get-Date -Format yyyy)-01"
}

# 1) Verify openssl availability + version.
$opensslCmd = Get-Command openssl -ErrorAction SilentlyContinue
if ($null -eq $opensslCmd) {
    Write-Host 'openssl not found on PATH.' -ForegroundColor Yellow
    Write-Host ''
    Write-Host 'Fallback (NOT YET AVAILABLE — placeholder for Fase 11a.x):'
    Write-Host '  cargo run -p license-tools -- gen-keypair --out-dir <dir> --key-id <id>'
    Write-Host ''
    Write-Host 'Until license-tools exists, install openssl: choco install openssl  OR  scoop install openssl'
    exit 70
}
$opensslVersion = & openssl version
Write-Host "openssl: $opensslVersion"

# 2) Prepare output dir.
if (-not (Test-Path $OutDir)) {
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
}
$resolved = (Resolve-Path $OutDir).Path
Write-Host "out: $resolved"

$sk = Join-Path $resolved 'licenser-sk.pem'
$pkPem = Join-Path $resolved 'licenser-pk.pem'
$pkRaw = Join-Path $resolved 'licenser-pk.raw'
$pkB58 = Join-Path $resolved 'licenser-pk.b58'
$didTxt = Join-Path $resolved 'licenser-did.txt'

if (Test-Path $sk) {
    Write-Error "refusing to overwrite existing key at $sk. Move it aside or pick a different -OutDir."
    exit 71
}

# 3) Generate private + extract public PEM via openssl 3.x.
Write-Host '[1/4] generating Ed25519 keypair'
& openssl genpkey -algorithm ed25519 -out $sk
& openssl pkey -in $sk -pubout -out $pkPem

# 4) Extract raw 32-byte public key.
#
# openssl 3.x writes pubkey as a 44-byte SubjectPublicKeyInfo DER:
#   30 2a 30 05 06 03 2b 65 70 03 21 00 <32-byte key>
# So the 32 raw bytes start at offset 12 of the DER.
Write-Host '[2/4] extracting raw pubkey'
$pkDer = Join-Path $resolved 'licenser-pk.der'
& openssl pkey -in $sk -pubout -outform DER -out $pkDer
$derBytes = [System.IO.File]::ReadAllBytes($pkDer)
if ($derBytes.Length -lt 44) {
    Write-Error "unexpected DER length $($derBytes.Length); abort."
    exit 72
}
$raw = New-Object byte[] 32
[Array]::Copy($derBytes, 12, $raw, 0, 32)
[System.IO.File]::WriteAllBytes($pkRaw, $raw)
Remove-Item $pkDer -Force

# 5) Base58 encode (Bitcoin alphabet) using .NET BigInteger — no extra dep.
Write-Host '[3/4] base58-encoding pubkey'
$ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

function ConvertTo-Base58 {
    param([byte[]]$bytes)
    # Count leading zeros (each = '1' in base58).
    $leadingZeros = 0
    foreach ($b in $bytes) {
        if ($b -ne 0) { break }
        $leadingZeros++
    }
    # Big-endian -> BigInteger needs little-endian + an optional zero sign byte.
    $rev = [byte[]]::new($bytes.Length + 1)
    for ($i = 0; $i -lt $bytes.Length; $i++) { $rev[$i] = $bytes[$bytes.Length - 1 - $i] }
    $rev[$bytes.Length] = 0
    $bi = [System.Numerics.BigInteger]::new($rev)
    $sb = New-Object System.Text.StringBuilder
    while ($bi -gt 0) {
        $rem = [int]($bi % 58)
        $bi  = [System.Numerics.BigInteger]::Divide($bi, 58)
        [void]$sb.Insert(0, $ALPHABET[$rem])
    }
    for ($i = 0; $i -lt $leadingZeros; $i++) { [void]$sb.Insert(0, '1') }
    return $sb.ToString()
}

$b58 = ConvertTo-Base58 -bytes $raw
$b58 | Set-Content -Encoding ascii -NoNewline -Path $pkB58
$did = "did:pharma:$b58"
$did | Set-Content -Encoding ascii -NoNewline -Path $didTxt

# 6) Print summary.
Write-Host '[4/4] done'
Write-Host ''
Write-Host '================================================================================'
Write-Host ' Ed25519 keypair generated'
Write-Host '================================================================================'
Write-Host "  sk PEM:  $sk"
Write-Host "  pk PEM:  $pkPem"
Write-Host "  pk raw:  $pkRaw    (32 bytes)"
Write-Host "  pk b58:  $pkB58    ($b58)"
Write-Host "  DID:     $didTxt"
Write-Host "  key_id:  $KeyId"
Write-Host ''
Write-Host '  DID string: '$did
Write-Host ''
Write-Host 'Next: paste into pharma-server crates/license/src/keys.rs::LICENSER_KEYS as:'
Write-Host "  (`"$KeyId`", `"$did`"),"
Write-Host ''
Write-Host 'And insert into license-server DB:'
Write-Host "  INSERT INTO key_pairs (key_id, did, pubkey_base64, status, activated_at)"
Write-Host "  VALUES ('$KeyId', '$did', <base64 of pk.raw>, 'active', NOW());"
Write-Host ''
Write-Host 'WARNING: licenser-sk.pem is the private signing key. Treat as crown jewel.'
Write-Host '  - Encrypted offline backup (e.g. age, gpg).'
Write-Host '  - Production: KMS-held, never copied to dev laptops.'
Write-Host '================================================================================'
