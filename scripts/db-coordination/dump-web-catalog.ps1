<#
.SYNOPSIS
  Dump the tu-farmacia.cl web product catalog (Cloud SQL Postgres) to
  demo-data/web_catalog.csv for web<->ERP reconciliation.

.DESCRIPTION
  psql is NOT required. Uses `gcloud sql export csv`, which runs the SELECT
  server-side and writes the result to a GCS bucket; the file is then
  downloaded. A throwaway bucket is created, granted to the instance service
  account, used, and (unless -KeepBucket) deleted. NO DB password is ever
  needed — export uses the Cloud SQL Admin API with your gcloud IAM creds.

.PARAMETER KeepBucket
  Leave the temp GCS bucket in place instead of deleting it.

.EXAMPLE
  ./dump-web-catalog.ps1
.EXAMPLE
  ./dump-web-catalog.ps1 -Project tu-farmacia-prod -Instance tu-farmacia-db -Db farmacia

.NOTES
  Requires gcloud authenticated with roles to manage SQL exports + create a
  GCS bucket (e.g. roles/cloudsql.admin + roles/storage.admin).
#>
[CmdletBinding()]
param(
  [string]$Project  = "tu-farmacia-prod",
  [string]$Instance = "",            # auto-discovered if empty
  [string]$Db       = "farmacia",
  [string]$Region   = "southamerica-east1",
  [string]$Out      = "demo-data/web_catalog.csv",
  [switch]$KeepBucket
)
$ErrorActionPreference = "Stop"

# Columns dumped (header added by reconcile.py — gcloud export emits no header):
#   external_id,name,price,stock,active,updated_at
$Query = "SELECT external_id, name, price, stock, active, updated_at FROM products ORDER BY external_id"

# Move to repo root so relative $Out resolves consistently.
$repoRoot = (git -C $PSScriptRoot rev-parse --show-toplevel) 2>$null
if (-not $repoRoot) { $repoRoot = (Get-Location).Path }
Set-Location $repoRoot

Write-Host "==> Project: $Project  DB: $Db  Region: $Region"

if (-not $Instance) {
  Write-Host "==> Discovering Cloud SQL instance..."
  $Instance = (gcloud sql instances list --project $Project `
    --filter='DATABASE_VERSION:POSTGRES*' --format='value(NAME)' | Select-Object -First 1)
  if (-not $Instance) { throw "No Postgres instance found in $Project" }
}
Write-Host "==> Instance: $Instance"

$Sa = (gcloud sql instances describe $Instance --project $Project `
  --format='value(serviceAccountEmailAddress)')
if (-not $Sa) { throw "Could not read instance service account" }
Write-Host "==> Instance service account: $Sa"

$Bucket = "gs://$Project-recon-$([int][double]::Parse((Get-Date -UFormat %s)))"
Write-Host "==> Creating temp bucket: $Bucket"
gcloud storage buckets create $Bucket --project $Project --location $Region | Out-Null

try {
  Write-Host "==> Granting objectAdmin on $Bucket to $Sa"
  gcloud storage buckets add-iam-policy-binding $Bucket `
    --member="serviceAccount:$Sa" --role="roles/storage.objectAdmin" `
    --project $Project | Out-Null

  $Uri = "$Bucket/web_catalog.csv"
  Write-Host "==> Exporting products to $Uri"
  gcloud sql export csv $Instance $Uri --database=$Db --project $Project --query=$Query

  $outDir = Split-Path $Out -Parent
  if ($outDir -and -not (Test-Path $outDir)) { New-Item -ItemType Directory -Force $outDir | Out-Null }
  Write-Host "==> Downloading to $Out"
  gcloud storage cp $Uri $Out --project $Project

  $rows = (Get-Content $Out | Measure-Object -Line).Lines
  Write-Host "==> Done. $Out has $rows rows (no header)."
  Write-Host "    Columns: external_id,name,price,stock,active,updated_at"
  Write-Host "    Next: python scripts/db-coordination/reconcile.py"
}
finally {
  if ($KeepBucket) {
    Write-Host "==> -KeepBucket set, leaving $Bucket (delete later: gcloud storage rm -r $Bucket)"
  } else {
    Write-Host "==> Cleaning up temp bucket $Bucket"
    gcloud storage rm -r $Bucket --project $Project 2>$null | Out-Null
  }
}
