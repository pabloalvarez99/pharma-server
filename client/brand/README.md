# RutBusiness — marca + design system

Nombre de producto: **RutBusiness**. Lema: *"El sistema operativo de tu negocio.
Tu RUT es la llave."* Multi-rubro (farmacia = beachhead). Ver [ADR-0015](../../docs/adr/0015-universal-cross-platform-client.md).

## Archivos

| Archivo | Qué es |
|---|---|
| `client/brand/rutbusiness-ui.html` | **Showcase vivo** — login (RUT-héroe + validación mód-11 en vivo) + app shell + dashboard. Dark/light. Se abre directo en navegador (doble click). Referencia visual, NO se importa. |
| `client/src/brand.css` | **Design system canónico** — tokens (`--rb-*`) + componentes base (`.rb-btn/.rb-input/.rb-card/.rb-kpi/.rb-pill/.rb-nav/.rb-table`). Esto SÍ se importa en la app. |

## Cómo adoptarlo en el cliente real (lane onboarding/shell — ye)

1. **Fuentes** — en `client/index.html` `<head>`:
   ```html
   <link rel="preconnect" href="https://fonts.googleapis.com">
   <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
   <link href="https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,400..700&family=IBM+Plex+Sans:wght@400;500;600;700&family=IBM+Plex+Mono:wght@400;500;600&display=swap" rel="stylesheet">
   ```
   (Offline-first: a futuro self-host de las fuentes — no bloquea el dev.)
2. **Import** — en `client/src/main.ts`: `import "./brand.css";` y `document.body.classList.add("rb")`.
3. **Tema** — `document.documentElement.dataset.theme = "dark" | "light"` (persistir en setting).
4. **Aplicar** — reemplazar estilos ad-hoc de `login.ts` / `shell.ts` por las clases `rb-*`.
   El showcase es la referencia 1:1 de cómo se ven login + shell + dashboard.

## Paleta

- Base ink/grafito (`--rb-bg`, `--rb-surface`) · acento **esmeralda** (`--rb-brand`)
  · secundario **ámbar** (`--rb-amber`, alertas/Pro) · `--rb-danger`/`--rb-info`.
- Datos (RUT, CLP, folios) SIEMPRE en mono (`.rb-num` / `--rb-ff-mono`).
- Display/marca en Fraunces (`.rb-display`), UI en IBM Plex Sans.

## Multi-rubro

La nav y secciones específicas (ej. Recetas/controlados) llevan `.rb-tag` y se
muestran/ocultan según `business.vertical` (ver `client/src/vertical.ts`).
`farmacia` = beachhead; el sistema sirve cualquier rubro.
