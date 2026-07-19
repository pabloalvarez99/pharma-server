# Variantes multi-SKU — UI cliente (C)

> Estado: **pro** (post light banner). Rama de trabajo típica: `feat/variants-ui-pro`.
> API dueño: **B** (`parent_id`, `by-barcode`, `GET/POST .../variants`, `variants_stock`, `variant_count`).

## Capturas en texto (wireframes)

### Inventario · listado

```
┌ Nombre              │ Precio  │ Stock      │ Estado          ┐
│ Polera básica       │ $9.990  │ 12 u. var. │ 2 variantes     │
│ Vender por barcode  │         │            │                 │
│ hijo                │         │            │                 │
│ Aspirina 500        │ $1.200  │ 0          │ Agotado         │
└─────────────────────┴─────────┴────────────┴─────────────────┘
```

### Detalle padre · tabla + alta barcode-first

```
Polera básica
Precio $9.990 · Stock: En variantes · 12 u.

[ Variantes multi-SKU ]              [ + Agregar variante ]
Tiene 2 variantes · vender por código de barras del hijo…

┌ Variante        │ Cód. barras   │ Precio  │ Stock   ┐
│ Polera — M Negro│ 780…0013      │ $9.990  │ 5       │
│ Polera — L Negro│ 780…0020      │ $9.990  │ Agotado │
└─────────────────┴───────────────┴─────────┴─────────┘
Editar o desactivar: por API/CSV (próximamente en panel).

Combinaciones sugeridas sin alta: M · Blanco, L · Blanco…
```

### Formulario “Agregar variante”

```
Nueva variante · Polera básica
Código de barras primero — el POS vende escaneando este SKU.
Enter en el código crea la variante · Esc cierra el formulario.

[ barcode * _______________ ]   ← focus inicial / escáner
[ nombre opcional ] [ stock ]
[ precio opcional ] [ costo ]
[ talla ] [ color ] [ sku ]
              [ Cancelar ] [ Crear variante ]
```

### POS · padre vs hijo

```
Buscar producto o escanear código de barras (Enter)…

[ Polera básica          Multi-SKU · stock en variantes: 12 ]
                         escanear barcode hijo

Scan 780…0013 → línea carrito «Polera — M Negro»
Click padre   → «Polera básica» tiene variantes. Escanea…
```

## Teclado / a11y

| Acción | Tecla |
|---|---|
| Cerrar detalle producto | `Esc` |
| Cerrar form variante (sin cerrar detalle) | `Esc` en el form |
| Crear variante tras scan | `Enter` en barcode |
| Foco inicial form | input barcode |

- `role="dialog"` + `aria-modal` en detalle
- Filas de variante con `aria-label` (nombre, attrs, stock/agotado)
- `aria-busy` mientras carga detalle

## Multi-rubro

- Solo rubros con `physicalStock` (tienda, farmacia, …)
- Servicios (belleza): sin toggle ni panel multi-SKU
- Attrs del pack (`talla`/`color`/`sku`) alimentan el form; fallback offline talla/color/sku

## BLOCKED_API (UI)

- **PATCH/DELETE variante** en panel: no hay comando Tauri fino; hint “por API/CSV”
- **Matriz talla×color completa**: helper `matrixComboSuggestions` (thin) solo sugiere combos faltantes; no POST masivo

## Tests

```bash
cd client
npx tsc --noEmit
npm test -- --run src/views/variants-ui.test.ts src/views/pos-service.test.ts
npm test -- --run   # suite completa; no regresar inventory-perf
```

## Demo 5 pasos

1. Login demo, rubro tienda  
2. Nuevo producto + *tiene variantes*  
3. Detalle → 2 variantes con barcode  
4. POS scan hijo OK; click padre error ES  
5. Cobrar variante  
