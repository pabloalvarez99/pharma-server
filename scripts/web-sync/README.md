# scripts/web-sync

Scripts de **referencia** para conectar un storefront web externo (Next.js / Astro /
WordPress / etc.) al `pharma-server` on-prem siguiendo el contrato de
[ADR-0012](../../docs/adr/0012-web-onprem-interop.md).

> Estos scripts NO corren en pharma-server. Corren en el entorno del **web**
> (Vercel build step, GitHub Action, Cloud Function, cron de la VPS del storefront).
> Por eso viven acá como referencia copy-paste, no como crate ni binario del server.

## Contenido

| Archivo | Patrón | Lenguaje | Deps |
|---|---|---|---|
| `pull-catalog.mjs` | A (Pull) | Node ≥ 20 | Sólo `node:*` (zero npm) |

Más scripts (Patrones B y C) se agregarán cuando los endpoints respectivos del
pharma-server estén implementados.

---

## `pull-catalog.mjs`

Cliente del Patrón A de ADR-0012. Hace un fetch paginado a
`GET /api/v1/public/catalog` del pharma-server y emite un archivo SQL con UPSERTs
idempotentes para la tabla `products` del Cloud SQL del web.

### Requisitos

- Node ≥ 20 (usa `fetch` + `AbortController` + top-level await built-in).
- Pharma-server alcanzable desde el entorno del script (LAN, VPN, tunnel, o IP
  pública). Ver [`docs/strategy/web-interop.md` §Patrón A paso 1](../../docs/strategy/web-interop.md).
- API key read-only generada en pharma-server con scope `catalog:read`.

### Variables de entorno

| Variable | Requerida | Descripción |
|---|---|---|
| `PHARMA_SERVER_URL` | sí | URL base de pharma-server. Ej: `https://farmacia-acme.trycloudflare.com`. Sin trailing slash. |
| `PHARMA_TENANT_SLUG` | sí | Slug del tenant que se quiere sincronizar. Ej: `coquimbo-centro`. |
| `PHARMA_PUBLIC_READ_KEY` | sí | API key read-only emitida con `pharma public-key create --scope catalog:read`. |
| `OUTPUT_SQL_FILE` | no | Path del archivo SQL a emitir. Default: `./out/catalog_upsert.sql`. Se crea el directorio si no existe. |

### Uso

```bash
PHARMA_SERVER_URL=https://farmacia-acme.trycloudflare.com \
PHARMA_TENANT_SLUG=coquimbo-centro \
PHARMA_PUBLIC_READ_KEY=pk_live_xxxxxxxxxxxxxxxxxxxxxxxx \
OUTPUT_SQL_FILE=./out/catalog_upsert.sql \
node scripts/web-sync/pull-catalog.mjs
```

Dry-run (imprime el SQL a stdout, no escribe archivo):

```bash
node scripts/web-sync/pull-catalog.mjs --dry-run
```

Aplicar el SQL a Cloud SQL:

```bash
psql "$DATABASE_URL" -f out/catalog_upsert.sql
```

### Output esperado

- `out/catalog_upsert.sql` con:
  - `BEGIN` ... `COMMIT` transaccional.
  - `INSERT ... ON CONFLICT (sku) DO UPDATE` (UPSERT idempotente).
  - `UPDATE ... stock_status = 'out_of_stock'` para SKUs que dejaron de venir
    del source tenant (soft tombstone, NO `DELETE`).

Schema asumido del lado del web:

```sql
CREATE TABLE products (
  sku           TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  price_clp     INTEGER NOT NULL,
  category      TEXT,
  image_url     TEXT,
  stock_status  TEXT NOT NULL DEFAULT 'unknown',
  source_tenant TEXT NOT NULL,
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Si tu schema difiere, editá `itemToUpsert()` y `buildSql()` en el script.

### Exit codes

| Code | Significado |
|---|---|
| 0 | OK. SQL generado (o impreso en dry-run). |
| 1 | Error de configuración. Falta una env var. |
| 2 | Error de red / HTTP. Detalle en stderr. |
| 3 | Respuesta del server con shape inesperado. |

### Integración en Vercel Cron

`vercel.json` del web:

```json
{
  "crons": [
    {
      "path": "/api/cron/pull-catalog",
      "schedule": "0 */6 * * *"
    }
  ]
}
```

Y `app/api/cron/pull-catalog/route.ts` invoca el script (o re-implementa la misma
lógica directamente con `pg` driver, sin pasar por archivo SQL — el archivo es útil
para debugging / audit).

### Portabilidad

Si tu web no usa Node, este script es 100% portable a PHP/Python/Ruby porque sólo usa
JSON y HTTP. El "contrato" es:

1. `GET /api/v1/public/catalog?tenant=<slug>&limit=200[&cursor=<c>]` con
   `Authorization: Bearer <key>`.
2. Respuesta `{ schema_version, tenant, generated_at, items[], pagination }`.
3. Iterar mientras `pagination.next_cursor` no sea null.
4. Aplicar UPSERT por `sku`.

Ver [`docs/strategy/web-interop.md`](../../docs/strategy/web-interop.md) para el contrato
HTTP completo.

---

## Referencias

- [ADR-0012](../../docs/adr/0012-web-onprem-interop.md) — decisión arquitectónica.
- [`docs/strategy/web-interop.md`](../../docs/strategy/web-interop.md) — guía operador.
- [CLAUDE.md § "Scope de este repo"](../../CLAUDE.md) — por qué este script vive acá
  como referencia y no en el repo del web.
