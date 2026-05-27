#Requires -Version 5.1
<#
.SYNOPSIS
    (RUNS ON HOST) Create a Hyper-V VM for MSI smoke testing and snapshot a clean baseline.

.DESCRIPTION
    One-time setup. Creates a Gen-2 Hyper-V VM from a Windows 11 Dev ISO (free, 90-day,
    https://aka.ms/windev), boots it, then — after you finish OOBE manually — you call
    `-SnapshotOnly` to capture the "baseline" checkpoint that run-smoke.ps1 reverts to.

    Decision context: zero-cost-launch-plan §3. Hyper-V ships with Windows 11 Pro (free).

.PARAMETER VmName
    VM name. Default "PharmaSmoke".

.PARAMETER IsoPath
    Path to the Windows 11 Dev ISO. Required unless -SnapshotOnly.

.PARAMETER VhdPath
    Where to create the VHDX. Default C:\HyperV\<VmName>.vhdx.

.PARAMETER MemoryGB
    Startup RAM. Default 4.

.PARAMETER SnapshotOnly
    Skip creation; just snapshot the existing VM as "baseline" (run after OOBE + WinRM enabled).

.NOTES
    Requires elevation + Hyper-V enabled:
      Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All
    After first boot, INSIDE the VM enable WinRM so run-smoke.ps1 can Invoke-Command:
      Enable-PSRemoting -Force
      Set-Item WSMan:\localhost\Client\TrustedHosts -Value '*' -Force   # host->guest
#>
[CmdletBinding()]
param(
    [string]$VmName = "PharmaSmoke",
    [string]$IsoPath,
    [string]$VhdPath = "C:\HyperV\PharmaSmoke.vhdx",
    [int]$MemoryGB = 4,
    [switch]$SnapshotOnly
)

$ErrorActionPreference = "Stop"

$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { Write-Error "Run elevated (Hyper-V cmdlets need admin)."; exit 1 }

if (-not (Get-Command Get-VM -ErrorAction SilentlyContinue)) {
    Write-Error "Hyper-V module not available. Enable Hyper-V: Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All (then reboot)."
    exit 1
}

if ($SnapshotOnly) {
    Write-Host "Snapshotting '$VmName' as 'baseline'..."
    Checkpoint-VM -Name $VmName -SnapshotName "baseline"
    Write-Host "Snapshot 'baseline' created. run-smoke.ps1 will revert to it before each run."
    exit 0
}

if (-not $IsoPath -or -not (Test-Path $IsoPath)) {
    Write-Error "IsoPath required and must exist. Download the free Win11 Dev ISO from https://aka.ms/windev"
    exit 1
}

if (Get-VM -Name $VmName -ErrorAction SilentlyContinue) {
    Write-Error "VM '$VmName' already exists. Remove it first or use -SnapshotOnly."
    exit 1
}

$vhdDir = Split-Path $VhdPath -Parent
if (-not (Test-Path $vhdDir)) { New-Item -ItemType Directory -Path $vhdDir -Force | Out-Null }

Write-Host "Creating Gen-2 VM '$VmName' (${MemoryGB}GB RAM, 60GB dynamic VHDX)..."
$switch = (Get-VMSwitch -SwitchType External -ErrorAction SilentlyContinue | Select-Object -First 1)
if (-not $switch) {
    $switch = (Get-VMSwitch -ErrorAction SilentlyContinue | Select-Object -First 1)
}
if (-not $switch) {
    Write-Warning "No VM switch found. Creating internal switch 'PharmaSmokeSwitch' (host<->guest only)."
    $switch = New-VMSwitch -Name "PharmaSmokeSwitch" -SwitchType Internal
}

New-VM -Name $VmName -Generation 2 -MemoryStartupBytes (${MemoryGB} * 1GB) `
    -NewVHDPath $VhdPath -NewVHDSizeBytes 60GB -SwitchName $switch.Name | Out-Null

Add-VMDvdDrive -VMName $VmName -Path $IsoPath
$dvd = Get-VMDvdDrive -VMName $VmName
Set-VMFirmware -VMName $VmName -FirstBootDevice $dvd
Set-VM -Name $VmName -CheckpointType Standard -AutomaticCheckpointsEnabled $false

Write-Host ""
Write-Host "VM '$VmName' created. Next steps (manual, one time):"
Write-Host "  1. Start-VM -Name $VmName ; then connect: vmconnect.exe localhost $VmName"
Write-Host "  2. Complete Windows 11 OOBE (offline account is fine)."
Write-Host "  3. INSIDE the VM, enable remoting:  Enable-PSRemoting -Force"
Write-Host "  4. Run Windows Update + reboot (so 'baseline' is patched)."
Write-Host "  5. Shut down cleanly, then snapshot the baseline:"
Write-Host "       pwsh installer/smoke/setup-vm.ps1 -VmName $VmName -SnapshotOnly"
