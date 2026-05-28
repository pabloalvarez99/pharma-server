# Pharma Server — SmartScreen warning al instalar el MSI

## Qué pasa

Al doble-clickear el MSI descargado, Windows muestra:

> **Windows protegió tu PC**
> Microsoft Defender SmartScreen impidió la apertura de una aplicación no reconocida que podría poner tu PC en riesgo.

Esto es esperado mientras el MSI no esté firmado con un certificado Authenticode de una CA reconocida.

## No es un bug

- El MSI fue compilado con `cargo wix` desde el repositorio público.
- El binario embebido (`pharma-service.exe`) fue compilado con Rust 1.95.0 desde el commit que aparece en la página de release.
- Puedes verificar el SHA256 del MSI contra el publicado en la release de GitHub: `gh release view v0.1.24 -R pabloalvarez99/pharma-server`.

## Cómo continuar la instalación

1. En el diálogo de SmartScreen, hacé click en **"Más información"** (link en gris pequeño).
2. Aparece un botón nuevo: **"Ejecutar de todas formas"**.
3. Hacé click. El asistente del MSI abre normal.
4. Seguí: *Next → Install → Finish*. Tras Finish, el dashboard se abre solo en `http://localhost:8080/app`.

## Alternativa: línea de comandos (sin SmartScreen)

```powershell
msiexec /i pharma-server-0.1.24-x86_64.msi /passive
```

- `/passive` — UI mínima, sin clicks del usuario; el auto-launch del dashboard SÍ corre.
- `/quiet` — sin UI; el dashboard NO se abre (apropiado para admins TI que despliegan masivamente vía Intune/SCCM).

## Plan compra Authenticode cert

Diferido a Fase 9.1 ([ADR-0010](../adr/0010-roadmap-fase-9-parity.md)). Opciones evaluadas:

- **DigiCert / Sectigo OV**: ~$200-400/año. 1-3 días validación.
- **EV cert con USB token**: ~$400+ inicial. SmartScreen elimina warning desde la primera instalación (vs OV que requiere acumular reputación).

Cuando esté firmado, este documento será actualizado y el warning desaparece.

## Si tu organización bloquea SmartScreen totalmente

Pedile al admin TI que distribuya el MSI vía Intune/SCCM/GPO. El motor MSI respeta políticas corporativas y no consulta SmartScreen en deployments managed.

## Reportar problema

GitHub issue: https://github.com/pabloalvarez99/pharma-server/issues — etiqueta `smartscreen`.
