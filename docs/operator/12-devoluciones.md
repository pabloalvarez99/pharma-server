---
title: 12 · Devoluciones y reembolsos
audience: cajeros, dueños y administradores de farmacia
---

# Devoluciones y reembolsos

Cuando un cliente devuelve un producto, registrás una **devolución** sobre la
venta original. Esto deja constancia del dinero reembolsado y de qué se devolvió.

> **Lo más importante de este capítulo**: la devolución registra el **dinero** y
> el **motivo**, pero **no reingresa el stock automáticamente**. Si el producto
> devuelto está en buen estado y lo vas a volver a vender, tenés que
> **reingresar el stock a mano** por Inventario (ver más abajo). Es a propósito:
> muchas devoluciones son de productos que **no** vuelven a la góndola (vencidos,
> dañados, abiertos), y el sistema no asume por vos cuáles sí.

## Registrar una devolución

1. Entrá a **Devoluciones** en el menú de la izquierda.
2. **Nueva devolución**.
3. Escribí el **número de orden** de la venta original (el `order:…` que te dio
   el POS o que figura en la boleta) y hacé click en **Cargar boleta**.
4. Aparecen los productos de esa venta. Elegí **cuánto** devolver de cada uno
   (parcial o todo).
5. Completá:
   - **Tipo**: parcial (algunos productos) o total (toda la venta).
   - **Método de reembolso**: efectivo, tarjeta o transferencia (cómo le
     devolvés la plata al cliente).
   - **Motivo** (obligatorio): por qué se devuelve.
   - **Notas** (opcional).
6. **Confirmar devolución**.

La venta queda marcada como devuelta (total o parcialmente) y la devolución
aparece en el listado **Devoluciones recientes** con su fecha, monto y método.

## Reingresar el stock (cuando corresponde)

Si el producto devuelto **vuelve a la venta**, reingresá su stock:

1. Entrá a **Inventario**.
2. Buscá el producto.
3. Usá **Ajustar stock** y sumá la cantidad que volvió, anotando el motivo
   (por ejemplo "devolución orden …").

Si el producto **no** vuelve a la venta (vencido, dañado, abierto), **no**
reingreses stock: la plata se devolvió, pero esa unidad ya no existe como
vendible.

## Devolución vs. nota de crédito

- **Devolución** (este módulo) — para ventas con **boleta**. Registra el
  reembolso al consumidor final.
- **Nota de crédito** (módulo Facturas) — cuando la venta original fue una
  **factura** a una empresa. Ahí no se usa este módulo: se emite una nota de
  crédito (61) que referencia la factura. Ver el capítulo
  [Facturas, notas y guías](./09-facturas-notas-guias.md).

## Efectivo y caja

Si reembolsás en **efectivo**, estás sacando plata del cajón. Tenelo presente al
hacer el **cierre de caja**: el arqueo va a esperar menos efectivo. Anotá el
motivo en la nota de cierre si hace falta (ver
[Cierre de caja](./05-fin-de-dia.md)).

## Problemas comunes

| Qué ves | Qué significa | Qué hacer |
|---|---|---|
| "Cargar boleta" no trae nada | El número de orden no existe o está mal escrito | Revisá el `order:…` en la boleta original |
| No me deja confirmar | Falta elegir cantidades o el motivo | Cargá la boleta, elegí qué devolver y completá el motivo |
| Devolví pero el stock no subió | Es el comportamiento esperado | Reingresá el stock a mano por Inventario si el producto vuelve a la venta |
| El cliente tiene factura, no boleta | Este módulo es para boletas | Emití una **nota de crédito** desde Facturas |
