# RutBusiness Client

Frontend único (TS vanilla + Vite) con dos targets:

- **Desktop (Tauri 2)** — `npm run dev` / `npm run build` (+ `npm run tauri dev|build`).
  `invoke` va por IPC a los 73 comandos Rust de `src-tauri/src/commands/*`.
- **Web / PWA (SP3, ADR-0015 P2)** — `npm run dev:web` / `npm run build:web`.
  El MISMO frontend corriendo en browser: Vite en `--mode web` aliasea
  `@tauri-apps/api/core` → `src/web-transport/` (shim `invoke`→`fetch` que
  replica 1:1 los 73 comandos, mismos errores en español) y
  `@tauri-apps/plugin-updater` → stub no-op. Cero cambios en las 18 vistas ni
  en `src/api/*`.

## Web build (PWA)

```powershell
npm run build:web            # tsc + vite build --mode web → dist-web/
npm run dev:web              # dev server modo web (shim activo)
npx vite preview --outDir dist-web   # probar el build localmente
```

- **Server por defecto**: `https://api.rutbusiness.cl`, inyectable en build con
  `VITE_DEFAULT_SERVER_URL` (mientras el dominio no exista:
  `https://136.67.83.70.nip.io`, la VM `pharma-prod`). El shim siembra
  `localStorage["pharma:last-server"]` solo si está vacío; el operador puede
  cambiar la URL en el login igual que en desktop.
- **Token**: en memoria + `sessionStorage` (nunca `localStorage`); F5 mantiene la
  sesión, cerrar la pestaña la bota.
- **PWA**: `public/manifest.webmanifest` + `public/sw.js` (service worker
  shell-only: cachea solo assets estáticos del mismo origen; los datos SIEMPRE
  van a la red). Íconos en `public/icons/` (copiados de `src-tauri/icons/`).
- **Desktop-only**: `print_ticket` / `open_cash_drawer` (ESC/POS) y el updater
  degradan con el error controlado `"Disponible en la app de escritorio"`; las
  vistas ya hacen fallback (POS imprime con `window.print()`).

## Deploy web

Estático puro (`dist-web/`). Dos opciones:

1. **Vercel**:
   ```powershell
   $env:VITE_DEFAULT_SERVER_URL = "https://136.67.83.70.nip.io"  # o api.rutbusiness.cl cuando exista
   npm run build:web
   npx vercel deploy dist-web --prod
   ```
2. **Caddy de SP1** (misma VM): servir `dist-web/` como site estático junto al
   reverse-proxy de `pharma-api` (ver `docs/product/saas-web-cloud-ops.md`).

**CORS (obligatorio)**: el server bloquea cross-origin por defecto. En la VM,
agregar el origen del deploy en `config/local.toml` del working dir de
`pharma-api` (las listas NO se pueden setear por env `PHARMA__*`):

```toml
[cors]
allowed_origins = ["https://<deploy>.vercel.app"]
```

y reiniciar el servicio.

## Gate

`npm run gate` = build desktop + vitest (incluye unit del shim
`src/web-transport/shim.test.ts`) + `build:web` + e2e.
