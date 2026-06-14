# Catálogo de rubros — onboarding multi-rubro (RutAgentIA)

> Al primer inicio, el operador elige su **rubro** de un catálogo. Esto materializa
> la visión MULTI-RUBRO ([`rutagentia-vision.md`](./rutagentia-vision.md)): el core
> es agnóstico; el rubro elegido configura datos demo + qué features se muestran.
> Farmacia es el beachhead, no el límite.

## Cómo funciona

1. **Selección** (ye / vista `configuracion`/onboarding): grid de tarjetas, una por
   rubro (label + icono + 1 línea). El operador elige una.
2. **Persistencia**: `admin_setting business.vertical` (string, en inglés interno —
   ver §naming). ye lo escribe; paul/marvin/lucy/bob lo leen (fallback `pharmacy`).
3. **Efecto en la UI**: secciones específicas se muestran/ocultan por rubro. Ej:
   `recetas`/`controlados` (Ley 20.000) solo `farmacia`. `boleta`/`factura`/DTE =
   universal (todo rubro chileno emite). Reportes/stock/caja = universal.
4. **Datos demo**: `pharma seed-demo --tenant <slug> --vertical <rubro>` (+ botón en
   la app vía `POST /api/v1/admin/seed-demo`). Pack por rubro; los que no tienen pack
   aún siembran vacío o caen a un pack genérico.

## Catálogo v1

| vertical (interno) | label UI (es) | icono | seed pack | features gated |
|---|---|---|---|---|
| `pharmacy`   | Farmacia            | 💊 | ✅ | recetas + controlados (Ley 20.000), interacciones, principio activo |
| `minimarket` | Minimarket / Almacén| 🛒 | ✅ | — (abarrotes; sin campos clínicos) |
| `restaurant` | Restaurant / Comida | 🍽 | ⬜ | (futuro: insumos, mesas/comandas) |
| `cafe`       | Café / Pastelería   | ☕ | ⬜ | (futuro: producción/recetas de cocina) |
| `tienda`     | Tienda / Retail     | 🛍 | ⬜ | — |
| `belleza`    | Belleza / Estética  | 💅 | ⬜ | servicios + agenda (poco stock físico) |
| `servicios`  | Servicios / Oficios | 🔧 | ⬜ | servicios sin inventario físico |
| `otro`       | Otro                | ➕ | ⬜ (vacío) | — |

✅ = pack seed existe · ⬜ = listado en el catálogo, pack se construye al validar el rubro.

**Disciplina** (tesis SaaS→Agentic §6.2 — no construir framework prematuro): el
catálogo LISTA todos los rubros desde ya, pero el pack seed + las features de cada
rubro se construyen **cuando ese rubro se valida con un cliente real**. No se
construyen 8 packs de una. Rubros de servicio (belleza/servicios) son buena prueba
del core agnóstico: ventas sin stock/lotes.

## Naming (bug conocido a resolver)

El interno usa **inglés** (`pharmacy`, `minimarket`, …) — lo que espera el CLI
`seed-demo` y el endpoint. La UI muestra **español** (`Farmacia`, …). El cliente
(`vertical.ts`) llegó a usar `farmacia` (es) como valor → **mapear es→en** al llamar
el endpoint, no romper el contrato. Valor canónico almacenado = inglés.

## Asset reusable — DSS (fundador)

[`https://dss-spa.vercel.app`](https://dss-spa.vercel.app) (Vercel + Cloudflare):
agencia web que arma sitios por rubro. Aporta:

1. **Taxonomía de rubros** — su form "Postular" lista Restaurant/Comida ·
   Café/Pastelería · Tienda/Retail · Belleza/Estética · Servicios/Oficios · Otro.
   Es la fuente del catálogo v1 de arriba (+ farmacia/minimarket propios).
2. **Portafolio de páginas estáticas por rubro** (flagship `tu-farmacia.cl`) →
   candidatas a **plantillas de storefront** cuando RutAgentIA ofrezca web por
   tenant (Fase 14 cloud companion + push de stock [ADR-0013] / interop [ADR-0012]).

**Regla de scope**: repos separados, **sin cross-import** de código. Se reusa la
TAXONOMÍA (datos) y, a futuro, las plantillas como **referencia de diseño** — no se
importa el código de DSS al server on-prem.

## Próximos pasos

- **ye**: vista de selección de rubro (grid desde este catálogo) + persistir
  `business.vertical` + mapear es→en al llamar `seed-demo`.
- **marvin**: `seed-demo` ya soporta pharmacy/minimarket; agregar packs nuevos = un
  array por rubro en `domain::seed` (cuando se valide el rubro).
- **Fase 14**: evaluar plantillas DSS como storefront por tenant (no ahora).
