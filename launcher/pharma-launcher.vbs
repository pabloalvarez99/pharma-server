' Pharma Server client launcher — hidden host.
' Runs pharma-launcher.ps1 with no console window flash, so the desktop icon
' opens straight into the splash → dashboard (League-style), nothing else.
'
' The desktop shortcut targets THIS file.

Option Explicit

Dim shell, fso, scriptDir, ps1Path, cmd
Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
ps1Path = scriptDir & "\pharma-launcher.ps1"

cmd = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File """ & ps1Path & """"

' 0 = hidden window, False = don't wait (return immediately).
shell.Run cmd, 0, False
