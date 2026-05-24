---
title: 02 · Vender en el POS (cobrar)
audience: cajeros y cajeras
---

# Vender en el POS — paso a paso

El **POS** (Punto de Venta) es donde armás el carrito de la persona que está
frente al mostrador y le cobrás. Es la pantalla que más vas a usar.

Para entrar al POS, hacé click en el botón **POS** del menú de la izquierda.

> (ver captura: vista del módulo POS dividida en dos columnas)

La pantalla se divide en **dos columnas**:

- **Izquierda — buscador de productos.** Una barra arriba que dice **Buscar
  producto para agregar...** y debajo una lista con los productos que el
  sistema encuentra.
- **Derecha — el carrito y el cobro.** El título **Carrito**, debajo las líneas
  de productos que vas agregando, debajo el **Total** en grande, después tres
  botones para elegir el medio de pago, y abajo de todo un botón azul que dice
  **Cobrar**.

## Paso 1 — Buscá el producto

Hacé click en la barra de búsqueda de la izquierda y empezá a escribir. Podés
buscar por:

- **Nombre del producto** — "paracetamol", "alka", "ibuprofeno".
- **Laboratorio** — "Bagó", "Roche".
- **Principio activo** — "amoxicilina".

A medida que escribís, la lista de abajo se va filtrando sola. No hace falta
apretar ningún botón de "buscar": el sistema busca a medida que tipeás.

**Si la lista te muestra "Sin resultados"**:
- Revisá si escribiste bien el nombre.
- Probá con un trozo del nombre (en vez de "Paracetamol 500mg" probá sólo con
  "paracetamol").
- Si igual no aparece, mirá la entrada **"El producto no aparece"** en
  [Problemas comunes](./06-problemas-comunes.md).

## Paso 2 — Agregá productos al carrito

Cada resultado es una **tarjeta** clickeable que muestra el nombre, el precio,
el stock disponible y un pequeño estado (**OK**, **Bajo**, **Agotado**).

Hacé click sobre la tarjeta del producto que querés vender. Vas a ver que
aparece en el carrito de la derecha con cantidad **1**.

**Si querés vender más de una unidad**:

En la línea del carrito vas a ver dos botones chicos:
- El botón **−** (menos) baja la cantidad.
- El botón **+** (más) sube la cantidad.

Hacé click en **+** las veces que necesites. La cantidad se va actualizando y
el subtotal de esa línea también.

**Si te equivocaste y agregaste un producto que no querías**:

Hacé click en **−** hasta que la cantidad llegue a 0. La línea desaparece sola
del carrito.

**Si tocaste un producto que está agotado**:

El sistema no te deja agregarlo. La tarjeta no responde al click. El badge
**Agotado** aparece en rojo.

## Paso 3 — Mirá el total

Debajo del carrito, en grande, está el **Total** con el monto en pesos
chilenos (formato `$1.234`). Se actualiza solito cada vez que agregás, sacás o
cambiás la cantidad de algún producto.

Decíle el monto a la persona que está pagando.

## Paso 4 — Elegí el medio de pago

Debajo del Total hay **tres botones**:

- **Efectivo** — la persona paga con billetes y monedas.
- **Débito** — paga con tarjeta de débito (Redcompra).
- **Crédito** — paga con tarjeta de crédito.

Hacé click en el que corresponda. El botón elegido queda con borde más oscuro
para que sepas cuál está activo.

> Por ahora el sistema acepta **un solo medio de pago por venta**. Si la
> persona quiere pagar la mitad en efectivo y la mitad con tarjeta, hablá con
> tu admin para que te muestre cómo dividir la venta (esa función va a llegar
> en una próxima versión).

## Paso 5 — Tocá COBRAR

Hacé click en el botón azul grande que dice **Cobrar**.

**Lo que pasa cuando funciona**:
1. El botón empieza a pulsar (procesando).
2. Aparece arriba un mensaje verde tipo "Venta registrada · 3 ítem(es) ·
   $4.530".
3. El carrito se vacía solo.
4. La lista de productos de la izquierda se actualiza para mostrar el stock
   que ya bajó.
5. Listo, ya podés atender al siguiente cliente.

**La boleta**: si tu local tiene impresora térmica configurada y boleta SII
activa, la boleta se imprime automáticamente. Si no, podés tomar el número de
la venta del mensaje verde para anotarla a mano.

## Qué hacer si aparece "Stock insuficiente"

Este es el error más común. Significa que, mientras vos estabas armando el
carrito, alguien más vendió las últimas unidades de un producto desde otra
caja, **o** el stock que mostraba el sistema estaba desactualizado.

**Acción del cajero**:

1. Mirá la línea del carrito que el sistema indica (queda marcada).
2. Reducí la cantidad de ese producto con el botón **−** hasta el máximo que
   diga el sistema, o quitalo del todo si ya no queda nada.
3. Volvé a tocar **Cobrar**.
4. Si la persona se enoja porque "estaba ahí hace dos minutos", explicale que
   se vendió en otra caja del local en ese mismo momento.

**Acción del admin / químico** (después, no en el momento):
- Hacé un conteo físico de ese producto.
- Si el stock del sistema y el físico no coinciden, hay un problema más grande
  que hay que investigar — avisá al dueño.

## Otros mensajes de error y qué significan

- **"El carrito está vacío"** — apretaste Cobrar sin agregar nada. Agregá al
  menos un producto.
- **"No se pudo conectar al servidor"** — el servidor del local podría estar
  caído. Mirá [Problemas comunes](./06-problemas-comunes.md), sección
  "Servidor caído".
- **"Producto inactivo"** — el producto fue dado de baja por el admin. No se
  puede vender. Sacalo del carrito.

## Tip — atajos de teclado útiles

- **Tab** te mueve entre el buscador y el carrito sin que tengas que sacar la
  mano del teclado.
- **Esc** limpia el campo de búsqueda.
- **Enter** (cuando el foco está en el botón Cobrar) ejecuta el cobro.

## Devolución de una venta — no se hace desde el POS

Si la persona vuelve al rato pidiendo cambiar un producto o devolverlo, **no
hagas otra venta en negativo**. Pedíle ayuda al admin: las devoluciones se
manejan desde otro módulo y dejan rastro en el sistema. Mirá la entrada
"El cliente quiere devolución" en [Problemas comunes](./06-problemas-comunes.md).

> Siguiente paso: [Revisar el inventario](./03-inventario.md).
