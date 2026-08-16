# Cara pública RutAgent (Vercel)

Landing estática de una pantalla: historia de feria + “la app no pide una IP”.
**No es el ERP.** La API de lab sigue en Hetzner (`89.167.106.26`).

## Contenido

| Archivo       | Rol                                      |
|---------------|------------------------------------------|
| `index.html`  | Markup es-CL                             |
| `styles.css`  | Afiche papel + teal / hoja de tomate     |
| `vercel.json` | Headers + rewrite opcional `/api/*` → lab |

## Deploy

Desde esta carpeta (cuenta Vercel ya logueada):

```bash
cd deploy/cara
npx vercel          # preview
npx vercel --prod   # producción → *.vercel.app
```

Primera vez en la máquina: `npx vercel login` (interactivo).

Sin CLI: arrastrá esta carpeta en [vercel.com/new](https://vercel.com/new).

No hace falta build step ni variables de entorno. No hay secretos en estos archivos.

## Rewrite `/api`

`vercel.json` reescribe `/api/:path*` → `http://89.167.106.26/:path*` para que el hostname
`*.vercel.app` pueda frontar el lab. Si el proxy de Vercel falla (HTTP plano, CORS, timeouts),
la cara sigue sirviendo sola: la app Android apunta directo al lab o a la URL que configure el equipo.

IP del lab = pública de prueba; no commitear tokens JWT, SSH ni API keys de Hetzner.
