# Import customers into a running pharma-server via /api/v1/admin/import-customers.
#
# Reads the JSON produced by `scripts/extract_customers_supabase.py`, logs in
# as the admin/owner user, and POSTs the customers in batches of 100 (the API
# caps batches at 200 — we stay well under). Reruns are idempotent: rows are
# matched by `(tenant, email)` and updated in place.
#
# Usage:
#   pwsh scripts/import_customers.ps1 `
#       -BaseUrl   http://localhost:8080 `
#       -Tenant    tufarmacia `
#       -Email     admin@tufarmacia.cl `
#       -Password  $env:PHARMA_ADMIN_PW `
#       -JsonPath  demo-data/tufarmacia_customers.json
#
# Exit codes:
#   0 = all rows accounted for (created + updated >= total non-erroring)
#   1 = HTTP / IO failure
#   2 = bad CLI args / missing file

[CmdletBinding()]
param(
    [string] $BaseUrl  = 'http://localhost:8080',
    [string] $Tenant   = 'tufarmacia',
    [string] $Email    = 'admin@tufarmacia.cl',
    [Parameter(Mandatory = $true)]
    [string] $Password,
    [string] $JsonPath = 'demo-data/tufarmacia_customers.json',
    [int]    $BatchSize = 100
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $JsonPath)) {
    Write-Error "JSON file not found: $JsonPath"
    exit 2
}
if ($BatchSize -lt 1 -or $BatchSize -gt 200) {
    Write-Error "BatchSize must be between 1 and 200 (server cap)"
    exit 2
}

# --- login --------------------------------------------------------------
$loginBody = @{
    tenant   = $Tenant
    email    = $Email
    password = $Password
} | ConvertTo-Json -Compress

try {
    $loginRes = Invoke-RestMethod `
        -Method  POST `
        -Uri     "$BaseUrl/api/v1/login" `
        -ContentType 'application/json' `
        -Body    $loginBody
} catch {
    Write-Error "login failed: $($_.Exception.Message)"
    exit 1
}

$token = $loginRes.token
if (-not $token) {
    Write-Error 'login response missing token'
    exit 1
}
Write-Host "logged in as $Email (tenant=$Tenant)"

# --- load & batch ------------------------------------------------------
$payload = Get-Content $JsonPath -Raw | ConvertFrom-Json
if (-not $payload.customers) {
    Write-Error "no 'customers' array in $JsonPath"
    exit 2
}
$total = $payload.customers.Count
Write-Host "importing $total customers in batches of $BatchSize"

$created     = 0
$updated     = 0
$errorsAll   = @()

for ($i = 0; $i -lt $total; $i += $BatchSize) {
    $end   = [Math]::Min($i + $BatchSize, $total) - 1
    $slice = $payload.customers[$i..$end]
    $body  = @{ customers = $slice } | ConvertTo-Json -Depth 6 -Compress

    try {
        $res = Invoke-RestMethod `
            -Method      POST `
            -Uri         "$BaseUrl/api/v1/admin/import-customers" `
            -ContentType 'application/json' `
            -Headers     @{ Authorization = "Bearer $token" } `
            -Body        $body
    } catch {
        Write-Error "batch $i..$end POST failed: $($_.Exception.Message)"
        exit 1
    }

    $created   += [int]$res.created
    $updated   += [int]$res.updated
    if ($res.errors) {
        foreach ($e in $res.errors) {
            # Re-base idx into the global array so the operator can find the row.
            $errorsAll += [pscustomobject]@{
                global_idx = $i + [int]$e.idx
                message    = $e.message
            }
        }
    }
    Write-Host ("batch {0}..{1}: created={2} updated={3} errors={4}" -f `
        $i, $end, $res.created, $res.updated, ($res.errors.Count))
}

Write-Host ''
Write-Host '--- import summary ---'
Write-Host "created : $created"
Write-Host "updated : $updated"
Write-Host "errors  : $($errorsAll.Count)"
if ($errorsAll.Count -gt 0) {
    $errorsAll | Format-Table -AutoSize | Out-String | Write-Host
}

exit 0
