# Pharma Server — Smoke install procedure (Windows Sandbox)

## Por qué Windows Sandbox

- Built-in en Windows Pro / Enterprise.
- VM efímera: cada `.wsb` arranca limpio, se destruye al cerrar.
- Cero estado residual: simula primer-install de farmacia real sin contaminar el host.
- Sin necesidad de Hyper-V manual, ISO descargas, etc.

## Prerrequisitos one-time (requiere reboot)

```powershell
# Habilitar feature (NoRestart para hacerlo sin reboot inmediato):
Enable-WindowsOptionalFeature -Online -FeatureName 'Containers-DisposableClientVM' -NoRestart -All

# Después reboot:
Restart-Computer
```

Tras el reboot, "Windows Sandbox" aparece en Start Menu.

## Cómo correr el smoke

1. Asegurate de tener el MSI buildeado:
   ```powershell
   cd C:\Users\Administrator\Documents\GitHub\pharma-server
   cargo wix --package service --no-build --nocapture `
     -C -ext -C WixFirewallExtension -L -ext -L WixFirewallExtension
   ls target\wix\*.msi
   ```

2. Doble-click `installer\sandbox\smoke.wsb`. Se abre la VM Sandbox.

3. Dentro del Sandbox, el `LogonCommand` ejecuta automáticamente `smoke-inside.ps1` que:
   - Encuentra el MSI más reciente en `C:\msi` (mounted del host).
   - Instala con `msiexec /passive /l*v C:\Users\WDAGUtilityAccount\install.log`.
   - `sc.exe query PharmaServer` para verificar service instalado.
   - Polling `GET /` con timeout 15s.
   - `curl` final `GET /` mostrando `{"name":"pharma-server","version":"X.Y.Z"}`.
   - Check shortcut Start Menu.

4. Verificación manual extra:
   - Click en Start → "Pharma Server" → "Pharma Server Dashboard". Browser default abre en `http://localhost:8080/app` con dashboard funcional.
   - Browser default debería haber abierto solo (modo `passive`).

5. Test uninstall:
   ```powershell
   msiexec /x C:\msi\pharma-server-0.1.24-x86_64.msi /passive
   sc.exe query PharmaServer  # debe responder "service does not exist"
   ```

6. Cerrar la ventana Sandbox para destruir la VM (todo lo instalado se borra).

## Test matrix esperado

| Modo | Browser auto-abre? | Service installed? | Shortcut? |
|---|---|---|---|
| `msiexec /quiet` | NO | SÍ | SÍ |
| `msiexec /passive` | SÍ (tras healthcheck OK) | SÍ | SÍ |
| Doble-click (full UI) | SÍ | SÍ | SÍ |

## Si el smoke falla

- **API no responde en 15s**: aumentar timeout en `launch-wait.ps1` o investigar logs del service en `C:\ProgramData\PharmaServer\`.
- **Shortcut missing**: verificar `ShortcutComponents` en `installer/wix/main.wxs`.
- **Service no instala**: ver `install.log` para errores WiX.
- **Firewall warning popup**: esperado solo si Sandbox cambia política default; en farmacia real `WixFirewallExtension` aplica regla silenciosamente.

## Limitaciones del Sandbox para smoke

- No persiste estado entre runs → cada smoke arranca con licencia FREE.
- Networking default = NAT del host, no LAN real. Para test multi-tenant en LAN real, necesitas VM real Hyper-V o second physical machine.
- No simula Windows Server (target valid para deployments enterprise multi-sucursal).
- No simula bajos recursos (target: i3 + 8GB SSD del CLAUDE.md performance budget).

Para esos casos, ver `docs/install/full-vm-smoke.md` (TODO Fase 9.2).
