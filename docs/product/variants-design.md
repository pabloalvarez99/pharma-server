# Variantes / multi-SKU (fase 1) — diseño

> **Estado**: implementado en domain + API (migración 0034).  
> **Rubro objetivo**: `tienda` (retail Chile); core multi-rubro agnóstico.  
> **Dueño**: Terminal B. UI (cliente) = Terminal C después.

## Decisión: Opción A (producto padre + hijos)

**Elegida: A.** Variantes = filas `product` hijas con `parent_id → product` padre,
`attrs` (talla/color/sku), barcode propio (`product_barcode`) y `stock` propio.

### Por qué no B (`product_variant` separada)

| Dimensión | A (hijos `product`) | B (`product_variant`) |
|---|---|---|
| Surreal / migraciones | 1 campo + índice en `product` | tabla nueva + FKs + sync |
| POS barcode | ya resuelve `product_barcode → product` | hay que bifurcar lookup |
| Stock / FEFO / movimientos | path actual sin tocar | dual-path o reescritura |
| Compras / recepción | OC línea = `product` (variante) | línea apunta a variant id |
| FE / reportes | un DTO; `parent_id` opcional | dos modelos en wire |
| Costo fase 1 | bajo, reusa catálogo | alto, sin beneficio inmediato |

B tiene sentido si un día el padre debe ser “abstracto” sin stock y las
variantes no deben contaminar listados/stats globales. Fase 1 no lo necesita:
listados excluyen hijos por defecto (`parent_id = NONE`).

## Modelo (migración `0034_product_parent.surql`)

```
product.parent_id: option<record<product>>
INDEX product_tenant_parent (tenant, parent_id)
```

Reglas de dominio:

1. Solo un nivel: un hijo no puede tener hijos.
2. Padre y hijo mismo tenant.
3. Stock vendible vive en la **variante** (o en el producto plano si no hay
   variantes). El padre retail suele quedar en stock 0.
4. Venta del **padre** se rechaza si tiene al menos una variante activa
   (fuerza POS a escanear talla/SKU). Código HTTP 400 `INVALID_INPUT`;
   mensaje ES estable con fragmentos `tiene variantes` y `escanee el código`
   (contrato del client POS).
5. Producto sin `parent_id` y sin hijos = comportamiento actual (farmacia /
   minimarket / servicio) — no se rompe.
6. **No** materializar `parent.stock = Σ hijos` en DB (rompería el ledger
   `product.stock == Σ stock_movement` y el path farmacia). Read-side:
   `ProductDto.variants_stock` en GET padre + suma en UI.
7. Barcode de variante es tenant-único; create usa `CREATE` (no UPSERT) para
   no robar EAN en carrera. Padre **puede** no tener barcode de caja.

`attrs` (0033) guarda discriminadores de variante (`talla`, `color`, `sku`).

## API v1 (fase 1)

| Método | Path | Rol | Notas |
|---|---|---|---|
| `POST` | `/api/v1/products/{id}/variants` | admin+ | crea hijo + barcode opcional; 409 si EAN tomado |
| `GET` | `/api/v1/products/{id}/variants` | auth | lista **hijos activos** `ORDER BY name`; `stock` + `barcode` |
| `DELETE` | `/api/v1/products/{id}/variants/{variant_id}` | admin+ | soft-delete variante; libera barcode; stock/movimientos se conservan |
| `PATCH` | `/api/v1/products/{variant_id}` | admin+ | **edit completo** del hijo (name/price/attrs/`active`/`barcode`); stock vía `POST .../stock` |
| `GET` | `/api/v1/products/by-barcode/{code}` | auth | resuelve barcode → product activo (variante o plano); **400** si padre con variantes; **404** si soft-deleted |
| `GET` | `/api/v1/products?include_variants=true` | auth | por defecto **oculta** hijos |
| `GET` | `/api/v1/products` | auth | padres multi-SKU: `variants_stock` = Σ stock hijos activos (batch; flag client) |
| `GET` | `/api/v1/products/{id}` | auth | padre con hijos: `variants_stock` = Σ stock hijos activos |

Money = STRING. Errores en español. Bearer JWT.

### Delete / update — invariantes

1. **Soft-delete** (`active = false`): no hard-delete. Ventas y `stock_movement`
   siguen apuntando al `product` id; el ledger no se reescribe.
2. **Barcode liberado** al eliminar (DELETE variante o DELETE producto): el EAN
   puede reutilizarse en otra variante.
3. **List / `variants_stock` / `variant_count`** solo cuentan hijos `active = true`.
4. **No borrar padre** con variantes activas (`400 INVALID_INPUT`); borrar hijos primero.
5. **No borrar** si `variant_id` no es hijo del `{id}` del path (`400`).
6. **Edit**: no hay `PATCH .../variants/{id}` dedicado — el hijo es un `product`;
   `PATCH /products/{variant_id}` + campo opcional `barcode` cubre el panel.

### curl (admin Bearer)

```bash
# Crear variante
curl -s -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"stock":5,"barcode":"7804999701010","attrs":{"talla":"M"}}' \
  "$BASE/api/v1/products/$PARENT_ID/variants"

# Editar variante (precio + barcode + attrs)
curl -s -X PATCH -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"price":"10990","barcode":"7804999701096","attrs":{"talla":"M","color":"Negro"}}' \
  "$BASE/api/v1/products/$VARIANT_ID"

# Eliminar variante
curl -s -X DELETE -H "Authorization: Bearer $TOKEN" \
  "$BASE/api/v1/products/$PARENT_ID/variants/$VARIANT_ID"
```

## Fuera de alcance (fase 1)

- UI / etiquetas de precio (C)
- Multi-bodega
- Variantes anidadas / matrices talla×color en server (attrs bastan)
- Reescritura del seed tienda (sigue OK como SKUs planos)

## Verificación

```bash
RUSTC_WRAPPER="" cargo test -p domain
RUSTC_WRAPPER="" cargo test -p api --test variants_sku
```
