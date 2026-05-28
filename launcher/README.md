# launcher — Pharma Server desktop client

League-of-Legends-style launcher: one icon → splash → the dashboard opens as a
chromeless desktop window (no browser tabs / address bar). Backend is started
automatically.

## Files

| File | Role |
|---|---|
| `pharma-launcher.ps1` | Brains: splash window, ensure backend up, poll `/health/ready`, open chromeless client. |
| `pharma-launcher.vbs` | Hidden host — runs the `.ps1` with no console flash. The desktop shortcut targets this. |
| `generate-icon.ps1` | Regenerates `pharma.ico` (green tile + medical cross, sizes 256/48/32/16). |
| `pharma.ico` | Generated icon used by the desktop shortcut. |

## Boot flow

```
double-click "Pharma Server" (desktop)
  → wscript runs pharma-launcher.vbs (hidden)
    → pharma-launcher.ps1 shows splash
      1. port 8080 already listening? → reuse it
         else service "PharmaServer" installed? → start it (elevates if needed)   [MSI/customer]
         else dev box? → spawn target\release|debug\pharma-api.exe (hidden, CWD=repo)  [dev]
      2. poll http://127.0.0.1:8080/health/ready until 200 (timeout 40s)
      3. open chromeless window:  msedge/chrome --app=http://127.0.0.1:8080/app
      4. close splash
```

The dev-spawned server is **left running** on purpose — the next double-click finds
the port already listening and opens instantly.

## Recreate the desktop shortcut

```powershell
$desktop = [Environment]::GetFolderPath('Desktop')
$sh = New-Object -ComObject WScript.Shell
$s  = $sh.CreateShortcut((Join-Path $desktop 'Pharma Server.lnk'))
$s.TargetPath       = "$env:SystemRoot\System32\wscript.exe"
$s.Arguments        = '"C:\Users\Administrator\Documents\GitHub\pharma-server\launcher\pharma-launcher.vbs"'
$s.WorkingDirectory = 'C:\Users\Administrator\Documents\GitHub\pharma-server'
$s.IconLocation     = 'C:\Users\Administrator\Documents\GitHub\pharma-server\launcher\pharma.ico,0'
$s.Save()
```

## Notes

- **Dev requires a built binary**: `cargo build -p api --release` (or debug). No binary
  and no service → the launcher shows a "compila el servidor / instala el MSI" dialog.
- **Dev server logs**: `%LOCALAPPDATA%\PharmaServer\dev-server.log` (+ `.err`).
- **Stop the dev server**: `Get-Process pharma-api | Stop-Process` (or Task Manager).
- **Customer path** (MSI installed): the service `PharmaServer` auto-starts at boot, so
  the launcher usually just opens the window.
- **Chromeless** needs Edge or Chrome (Chromium `--app`). Without either, it falls back
  to the default browser (a normal tab).
- The client window uses an isolated profile at `%LOCALAPPDATA%\PharmaServer\client-profile`.
- Override host/port: `pharma-launcher.ps1 -BackendHost 127.0.0.1 -Port 8080`.
