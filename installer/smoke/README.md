# installer/smoke — clean-VM MSI smoke test (zero-cost)

Verify the pharma-server MSI installs, runs the service, serves `/health/ready` 200, and
uninstalls cleanly — on a **clean** Windows VM. This is the regla #9 prerequisite #2
("smoke install VM verde") before automatic deploy to the public mirror.

**Plan**: [zero-cost-launch-plan.md §3](../../docs/strategy/zero-cost-launch-plan.md).

## Why this matters

A dev machine is dirty (Rust toolchain, leftover services, `./data/surreal`). It can't
prove the MSI works for a real pharmacy on fresh Windows. A reverted VM snapshot gives a
clean room every run.

## Cost: $0

| Component | Source | Cost |
|---|---|---|
| Hypervisor | Hyper-V (ships with Win11 Pro) | $0 |
| Guest OS | Windows 11 Dev VM ISO, https://aka.ms/windev (90-day, re-image free) | $0 |
| Orchestration | the scripts here | $0 |

## Files

| File | Runs on | Purpose |
|---|---|---|
| `setup-vm.ps1` | HOST | One-time: create VM from ISO, snapshot clean `baseline` |
| `run-smoke.ps1` | HOST | Per release: revert to baseline, copy MSI in, run smoke, report |
| `smoke-install.ps1` | GUEST (VM) | The actual test: install → service Running → health 200 → uninstall → gone |

## One-time setup

```powershell
# 0. Enable Hyper-V (reboot after). Verify CPU virtualization is on in BIOS.
Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All

# 1. Download the free Win11 Dev ISO from https://aka.ms/windev

# 2. Create the VM (elevated).
pwsh installer/smoke/setup-vm.ps1 -VmName PharmaSmoke -IsoPath C:\iso\Win11Dev.iso

# 3. Boot + connect, complete OOBE (offline account fine).
Start-VM -Name PharmaSmoke
vmconnect.exe localhost PharmaSmoke

# 4. INSIDE the VM: enable remoting so the host can drive it.
Enable-PSRemoting -Force

# 5. Run Windows Update + reboot inside the VM (so baseline is patched), shut down.

# 6. Snapshot the clean baseline.
pwsh installer/smoke/setup-vm.ps1 -VmName PharmaSmoke -SnapshotOnly
```

## Per-release smoke

```powershell
# Build + sign the MSI first (installer/sign/sign-msi.ps1), then:
pwsh installer/smoke/run-smoke.ps1 -VmName PharmaSmoke -MsiPath target\wix\pharma-server-0.1.25-x86_64.msi
# Prompts for VM admin credentials. Exit 0 = green.
```

`run-smoke.ps1` reverts to baseline before and after, so every run is clean and the VM is
left pristine.

## What the smoke verifies

1. `msiexec /i ... /qn` exits 0 (verbose log at `%TEMP%\pharma-smoke-install.log` in VM).
2. Windows service `PharmaServer` reaches **Running** within 60s.
3. `GET http://localhost:8080/health/ready` returns **200** (means migrations ran + DB
   reachable — see `crates/api/src/health.rs`).
4. `msiexec /x ... /qn` exits 0.
5. Service `PharmaServer` no longer exists.

Any failure → non-zero exit → fail the release (do not deploy).

## Gotchas

- The MSI build itself needs `-ext WixFirewallExtension` (CLAUDE.md regla #6 / memory
  gotcha). This smoke tests an already-built MSI — it does not build it.
- VM needs WinRM enabled inside (`Enable-PSRemoting -Force`) for `run-smoke.ps1` to drive
  it via `New-PSSession -VMName`.
- If using an internal-only VM switch, host→guest PowerShell Direct (`-VMName`) still works
  (it doesn't need network); the health check runs *inside* the VM against localhost, so
  no host→guest networking is required.
- Win11 Dev VM expires after 90 days — re-download + re-baseline. Cheap, just time.

## CI (later, when MRR justifies)

A self-hosted Windows runner with nested virtualization can run `run-smoke.ps1` on every
release PR. Until then, run it manually before each `release-publisher.yml` dispatch. See
[zero-cost-launch-plan.md §3.3](../../docs/strategy/zero-cost-launch-plan.md).
