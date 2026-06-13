---
title: 09 · Facturas, notas y guías (SII)
audience: dueños, administradores y cajeros con permiso de facturación
---

# Facturas, notas de crédito/débito y guías — paso a paso

Este capítulo explica cómo emitir, desde la pantalla **Facturas**, los otros
documentos tributarios electrónicos (DTE) que usa una farmacia además de la
boleta:

- **Factura electrónica (33)** — cuando el cliente es una empresa y pide
  factura (con su RUT, razón social y giro).
- **Nota de crédito (61)** — anula o rebaja una factura ya emitida (una
  devolución, un descuento posterior, un error de monto).
- **Nota de débito (56)** — aumenta el monto de una factura ya emitida (un
  cargo adicional, un interés).
- **Guía de despacho (52)** — acompaña el traslado de mercadería (por ejemplo
  un envío entre sucursales) sin que sea necesariamente una venta.

> **Antes de empezar**: si todavía no configuraste el certificado digital, los
> folios (CAF) y los datos de tu empresa, hacelo primero siguiendo el capítulo
> [Boletas electrónicas SII](./08-boletas-sii.md). La configuración es la misma;
> lo único distinto es que **cada tipo de documento necesita su propio CAF**.

## Qué necesitás (una sola vez)

Lo mismo que para las boletas, con una diferencia importante en los folios:

1. **Certificado digital** (`.pfx` con su clave) — el mismo de las boletas.
2. **Un CAF por cada tipo que vayas a emitir.** Los folios de boleta (39) **no**
   sirven para factura. Si vas a emitir facturas, descargá del SII el CAF para
   "Factura electrónica" (33) e importalo:

   ```text
   pharma caf import C:\ruta\al\CAF33.xml
   ```

   Repetí lo mismo para los tipos 61, 56 y 52 si los vas a usar.
3. **Datos del emisor** cargados en **Configuración → Emisor DTE (SII)** — los
   mismos que para la boleta.

## La pantalla Facturas

Entrá a **Facturas** en el menú de la izquierda. Vas a ver tres partes:

1. Arriba, un **aviso de folios**: te dice cuántos folios te quedan del tipo
   de documento que tengas seleccionado. Si está en rojo, pedí un CAF nuevo al
   SII e importalo.
2. En el medio, el formulario **Emitir documento**.
3. Abajo, el **listado** de documentos ya emitidos, con sus acciones.

## Emitir una factura

### Paso 1 — Elegí el tipo

En **Tipo de documento** elegí "Factura electrónica (33)". El aviso de folios
de arriba se actualiza al tipo elegido.

### Paso 2 — Completá los datos del cliente

La factura exige identificar al cliente **completo**:

- **RUT** — escribilo con o sin puntos; el sistema lo revisa al instante:
  - en **verde** te muestra el RUT bien escrito (`76.123.456-7`) cuando el
    dígito verificador es correcto;
  - en **rojo** te avisa *"RUT inválido: revisá el dígito verificador"* cuando
    está mal tipeado. **No vas a poder emitir** hasta corregirlo. Esto evita
    emitir una factura a un RUT que no existe.
- **Razón social**, **Giro**, **Dirección** y **Comuna** — tal como figuran en
  los datos del cliente. Los cuatro son obligatorios para la factura.

### Paso 3 — Cargá los productos

En **Items** agregá una línea por producto con **+ Agregar item**:

- **Descripción** — el nombre del producto.
- **Cantidad** y **Precio unit.** — el precio se escribe **con IVA incluido**
  (el mismo precio de la góndola).
- **Exento** — marcalo sólo si ese producto no paga IVA (es poco común en
  farmacia; dejalo sin marcar salvo que sepas que corresponde).

A medida que cargás los productos, abajo a la derecha aparece el **resumen** que
se va a emitir:

| Línea | Qué es |
|---|---|
| **Neto** | El valor sin IVA. |
| **IVA 19%** | El impuesto. |
| **Exento** | Sólo aparece si marcaste algún producto como exento. |
| **Total** | Lo que paga el cliente (Neto + IVA + Exento). |

Estos son **exactamente** los montos que el sistema va a estampar en la factura,
así que podés revisarlos con el cliente antes de firmar.

### Paso 4 — Firmá

Escribí la **clave del certificado** y hacé click en **Emitir y firmar**. La
factura queda firmada y aparece en el listado de abajo con estado **Firmado**,
con su folio y su timbre electrónico.

> La clave del certificado **no se guarda**: el sistema la vuelve a pedir cada
> vez que firmás un documento.

## Emitir una nota de crédito (61) o de débito (56)

Una nota **siempre corrige un documento anterior**. Por eso, al elegir nota de
crédito o de débito, aparece un bloque extra **Documento que corrige**:

1. Elegí el tipo (61 si rebajás/anulás, 56 si aumentás).
2. Completá los datos del cliente y los items igual que en la factura.
3. En **Documento que corrige** indicá:
   - **Tipo doc. original** (por ejemplo `33` si corrige una factura),
   - **Folio original** (el número de la factura),
   - **Fecha original**,
   - **Motivo**: *anula documento*, *corrige texto* o *corrige montos*.
4. Firmá igual que la factura.

> **Caso típico**: un cliente con factura devuelve un producto. Emitís una
> **nota de crédito (61)** que referencia esa factura, con motivo *corrige
> montos* (o *anula documento* si devuelve todo), por el valor devuelto.

## Emitir una guía de despacho (52)

Al elegir guía aparece el campo **Motivo del traslado**. Elegí el que
corresponda (venta, traslado interno entre sucursales, consignación, etc.). Un
**traslado interno** puede ir con valores bajos o de costo; una guía por **venta**
lleva los precios reales. El resto (cliente, items) se completa igual.

## Qué hacer con el documento emitido

En el listado de abajo, cada documento tiene los mismos botones que la boleta:

- **XML** — descarga el archivo firmado. En **plan Free** este es tu camino:
  lo subís a mano en el sitio del SII. Tus datos siempre son tuyos.
- **Enviar SII** — envío automático (requiere **plan Business**). Después usás
  **Consultar** para ver si el SII lo aceptó o rechazó.
- **Anular** — sólo **antes** de enviarlo al SII, te pide el motivo. Si el
  documento ya fue aceptado por el SII, no se anula: se emite una **nota de
  crédito** que lo deja sin efecto.

Para ver documentos de otro tipo, cambiá el selector **Ver** arriba del listado.

## Problemas comunes

| Qué ves | Qué significa | Qué hacer |
|---|---|---|
| El RUT queda en rojo y no podés emitir | El dígito verificador no calza | Revisá el número; el último dígito (o la K) tiene que coincidir |
| "Sin CAF tipo 33" en el aviso de folios | No importaste folios de ese tipo | `pharma caf import <CAF33.xml>` (cada tipo lleva su CAF) |
| "Completá todos los datos del cliente" | Falta razón social, giro, dirección o comuna | La factura exige los cinco campos del receptor |
| "La nota requiere el documento original" | Falta el folio/tipo/fecha que corrige | Completá el bloque **Documento que corrige** |
| "passphrase incorrecta" al firmar | La clave del certificado no es | Reintentá; si la perdiste, pedí reemisión al proveedor |
| Documento **Rechazado** | El SII no lo aceptó | Mirá el motivo en la columna SII; corregí y reemití |

> **Regla de oro** (igual que con las boletas): la clave del certificado **no se
> guarda nunca** ni viaja por internet en claro. Si alguien te pide "dejarla
> grabada", la respuesta es no.
