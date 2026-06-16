---
title: 10 · Compras a proveedores
audience: dueños y administradores
---

# Compras a proveedores

El módulo **Compras** lleva el ciclo completo de abastecimiento: tus
**proveedores**, las **órdenes de compra** (OC) que les hacés, la **recepción**
de la mercadería que llega (que sube el stock automáticamente) y la **cuenta por
pagar** de cada orden.

> Ver la lista de compras la puede hacer cualquier persona con rol cajero. Crear
> órdenes, recibir mercadería y registrar pagos es tarea del **administrador**.

## Proveedores

Antes de hacer una orden necesitás tener cargado al proveedor.

1. Entrá a **Compras** en el menú de la izquierda.
2. En la sección de proveedores, **Nuevo proveedor**.
3. Completá al menos el **nombre**. RUT y datos de contacto (persona, teléfono,
   correo) son opcionales pero recomendados. Guardá.

## Crear una orden de compra (OC)

1. **Nueva OC**.
2. Elegí el **proveedor** de la lista.
3. Agregá una línea por producto con **+ Agregar línea**: descripción, cantidad
   y **costo unitario** (lo que te cobra el proveedor, sin importar el precio de
   venta). El sistema calcula el subtotal de cada línea y el total de la orden.
4. (Opcional) Agregá una **referencia** (N° de la cotización o factura del
   proveedor) y **notas**.
5. Guardá. La orden queda en estado **Borrador**.

> El costo unitario es importante: cuando recibís la mercadería, el sistema usa
> ese costo para recalcular el **costo promedio ponderado** del producto, que es
> la base de tus márgenes en Reportes.

## Estados de una orden

Filtrá la lista con el selector de arriba. Una OC pasa por:

- **Borrador** — recién creada, todavía podés cancelarla.
- **Enviada** — ya se la pasaste al proveedor.
- **Parcial** — recibiste una parte de lo pedido.
- **Recibida** — llegó todo.
- **Cancelada** — se anuló.

## Recibir mercadería

Cuando llega el pedido, registrá la recepción para que **suba el stock**:

1. En la lista, hacé click en la orden para abrir su **detalle**.
2. **Recibir mercadería**.
3. Por cada línea, el sistema propone la cantidad pendiente. Ajustá si llegó
   menos de lo pedido (no podés recibir más de lo pendiente).
4. (Opcional) Agregá una nota (N° de guía de despacho del proveedor, por
   ejemplo). Confirmá.

Al confirmar, el sistema **sube el stock** de cada producto, recalcula su costo
promedio, deja registro del movimiento, y marca la orden como **Recibida** (o
**Parcial** si todavía falta). Si recibís en tandas, repetí el proceso cuando
llegue el resto.

## Cuenta por pagar (pagos al proveedor)

En el **detalle** de cada orden, abajo, está el bloque **Cuenta por pagar**:

- **Total OC / Pagado / Saldo** — cuánto vale la orden, cuánto ya pagaste y
  cuánto debés.
- El listado de **pagos** que ya registraste (fecha, medio, referencia, monto).

Para registrar un pago:

1. **+ Registrar pago**.
2. El **monto** viene prellenado con el saldo pendiente; ajustalo si pagás solo
   una parte. No podés registrar más que el saldo.
3. Elegí el **medio de pago**: transferencia, depósito bancario, tarjeta o
   efectivo.
4. (Opcional) Agregá una **referencia** (N° de transferencia o comprobante).
5. **Registrar pago**.

El saldo se actualiza al instante. Cuando llega a cero, la orden queda marcada
como **Pagada**.

> **Pagos en efectivo**: si pagás al proveedor con plata del cajón y tenés una
> caja abierta, el egreso se suma al **arqueo** de esa caja, para que el cierre
> cuadre. Por eso conviene registrar el pago en el momento.

## Problemas comunes

| Qué ves | Qué significa | Qué hacer |
|---|---|---|
| "Registra un proveedor antes de crear una orden" | No hay proveedores cargados | Creá el proveedor primero |
| No puedo recibir una orden en Borrador | Solo se recibe desde Enviada/Parcial | Marcala como enviada (o el flujo que uses) antes de recibir |
| "No puedes recibir más de lo pendiente" | La cantidad supera lo que falta | Ajustá la cantidad recibida |
| El monto del pago supera el saldo | Estás registrando de más | El pago no puede ser mayor al saldo pendiente |
| Sin acceso a compras | Tu rol no tiene permiso | Pedile al administrador (crear/recibir/pagar es admin) |
