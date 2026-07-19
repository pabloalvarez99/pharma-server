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
   (fuerza POS a escanear talla/SKU).
5. Producto sin `parent_id` y sin hijos = comportamiento actual (farmacia /
   minimarket / servicio) — no se rompe.

`attrs` (0033) guarda discriminadores de variante (`talla`, `color`, `sku`).

## API v1 (fase 1)

| Método | Path | Rol | Notas |
|---|---|---|---|
| `POST` | `/api/v1/products/{id}/variants` | admin+ | crea hijo + barcode opcional |
| `GET` | `/api/v1/products/{id}/variants` | auth | lista hijos del padre |
| `GET` | `/api/v1/products/by-barcode/{code}` | auth | resuelve barcode → product (variante o plano) |
| `GET` | `/api/v1/products?include_variants=true` | auth | por defecto **oculta** hijos |

Money = STRING. Errores en español. Bearer JWT.

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
