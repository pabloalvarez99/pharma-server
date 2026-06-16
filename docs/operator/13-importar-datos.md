---
title: 13 · Cargar tu catálogo desde un Excel/CSV
audience: administradores y dueños
---

# Cargar tu catálogo desde un Excel/CSV

Cuando recién empezás con RutBusiness no querés cargar tus productos uno por uno.
El módulo **Importar** te deja subir **todo tu catálogo de una vez** desde una
planilla de Excel guardada como **CSV**. También sirve para **migrar** desde otro
sistema: exportás tu catálogo viejo a CSV, lo ajustás y lo subís acá.

> Esta sección es para el **administrador**. En el menú de la izquierda se llama
> **Importar**.

## Antes de tus datos reales: los datos demo

Si lo que querés es **practicar** sin riesgo, no importes nada todavía. Andá a
**Configuración** y usá el bloque **Cargar datos demo**:

1. Elegí tu **rubro** en la grilla (Farmacia, Minimarket, etc.). Los rubros que
   tienen pack de ejemplo muestran la etiqueta **datos demo**.
2. Tocá **Cargar datos demo**. El sistema llena la aplicación con productos,
   proveedores, ventas y clientes de prueba de ese rubro, para que veas el POS,
   el inventario y los reportes con datos creíbles.
3. Está marcado como **DEMO** y te pide confirmación. Si ya había datos demo
   cargados, te pregunta antes de regenerarlos.

> **No uses datos demo sobre datos reales.** Es sólo para una instalación de
> prueba. Cuando estés listo para trabajar de verdad, instalá limpio o pedí a tu
> técnico que borre los datos demo, y recién ahí importá tu catálogo real.

## Importar tu catálogo real (paso a paso)

En el módulo **Importar / Exportar productos**:

### Paso 1 — Prepará la planilla

Tu archivo tiene que ser **CSV** (en Excel: *Guardar como → CSV*). La **primera
fila es la cabecera** (los nombres de las columnas).

**Columnas obligatorias:**

- `name` (o `nombre`) — el nombre del producto.
- `price` (o `precio`, o `sale_price`) — el precio de venta.

**Columnas opcionales que el sistema reconoce:**

`external_id` · `barcode` (código de barras) · `cost_price` (costo) · `stock`
(existencia) · `category` (categoría) · `presentation` (presentación) ·
`discount_percent` · `description` · `image_url` · `slug`

Y, **solo para farmacia**: `laboratory` (laboratorio) · `active_ingredient`
(principio activo) · `therapeutic_action` · `prescription_type` (tipo de receta).

**Comodidades pensadas para Chile** (no tenés que pelear con el formato):

- Acepta cabeceras en **español**: `nombre`, `precio`, `código`, `existencia`…
- Acepta separador **`;`** o **`,`** (el que usa el Excel chileno).
- Acepta precios con **punto de miles**: `1.990` se entiende como `1990`.
- Ignora el carácter invisible (BOM) que Excel a veces agrega al inicio.

### Paso 2 — Elegí el archivo y previsualizá

1. Tocá **Elegir archivo CSV** y seleccioná tu planilla.
2. Tocá **Previsualizar**.

La vista previa **no guarda nada todavía**. Te muestra un cartel
*"Vista previa — todavía no se guardó nada. Revisa los números y confirma para
importar."* y un resumen con cuántos productos se van a **crear**, cuántos
**actualizar**, cuántos **fallidos** y el **total ok**.

Si alguna fila tiene problemas, aparece una tabla **Filas rechazadas** que te
dice la **línea** y el **motivo** exacto (por ejemplo, falta el precio). Corregí
esas filas en tu Excel y volvé a previsualizar.

### Paso 3 — Confirmá

Cuando los números te cuadren, tocá **Confirmar importación**. Recién ahí se
guarda. El sistema importa **exactamente lo que viste en la vista previa**. Si
te arrepentís antes de confirmar, tocá **Cancelar**.

## Reimportar sin duplicar (actualizar precios/stock)

Si tu planilla incluye la columna `external_id` (un código único por producto),
podés **volver a subir el mismo archivo** después de editarlo y el sistema
**actualiza** los productos en vez de duplicarlos. Es la forma cómoda de
cambiar precios en masa: exportás, editás en Excel, reimportás.

## Exportar tu catálogo

El botón **Exportar catálogo CSV** descarga **todo tu catálogo actual** en un
archivo con la fecha en el nombre (ej: `catalogo-2026-06-16.csv`). Usa **las
mismas columnas** que la importación, así que el ciclo natural es:
**exportar → editar en Excel → reimportar**. También te sirve como respaldo
rápido de tus productos, y porque tus datos son tuyos: te los llevás cuando
quieras.

> Volvé al [índice](./README.md).
