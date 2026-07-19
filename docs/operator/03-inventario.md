---
title: 03 · Revisar el inventario
audience: cajeros, encargados de stock, admin
---

# Revisar el inventario

El **Inventario** es donde mirás cuántas unidades de cada producto tenés en el
local, cuánto valen y qué productos están por agotarse o ya se agotaron.

Para entrar, hacé click en el botón **Inventario** del menú de la izquierda.

> (ver captura: vista de Inventario con KPIs arriba y tabla abajo)

Esta es la primera pantalla que aparece cuando entrás al sistema, porque es
la información que se mira con más frecuencia.

## Las cuatro tarjetas de arriba (KPI)

Arriba de todo vas a ver **cuatro tarjetas** con números grandes. Cada una te
da un dato clave:

### Tarjeta 1 — Productos

Te dice **cuántos productos distintos** tiene el sistema cargados, y debajo
cuántos están **activos** (es decir, se pueden vender). Por ejemplo:

```
Productos
1.842
1.780 activos
```

Si la diferencia entre el total y los activos es grande, es porque hay
productos dados de baja (descontinuados, retirados de mercado, etc.).

### Tarjeta 2 — Stock bajo

Te dice **cuántos productos están con poco stock** (menos de 5 unidades, o
debajo del mínimo que tenga configurado el producto). Si el número es mayor
que cero, la tarjeta se pone con borde **amarillo** para llamar tu atención.

Estos son los productos que tenés que pedir pronto al proveedor.

### Tarjeta 3 — Sin stock

Te dice **cuántos productos están agotados**. Si el número es mayor que cero,
la tarjeta se pone con borde **rojo**.

Estos productos **no se pueden vender** hasta que llegue mercadería nueva.

### Tarjeta 4 — Valorización

Te dice **cuánta plata vale tu inventario** sumando todos los productos al
precio de venta. Por ejemplo `$24.350.000` significa que si vendieras todo tu
stock al precio actual, recaudarías esa cifra.

Útil para el dueño cuando quiere saber el "tamaño" del local.

## La barra de búsqueda

Arriba a la derecha hay una barra que dice **Buscar producto...**.

Es igual a la del POS: escribís y la lista de abajo se filtra sola. Podés
buscar por:

- Nombre del producto
- Laboratorio
- Principio activo

Si no escribís nada, te muestra los primeros **60 productos** del catálogo.

## La tabla de productos

Debajo de las tarjetas y la búsqueda hay una **tabla** con cuatro columnas:

| Producto | Precio | Stock | Estado |
|---|---|---|---|
| Paracetamol 500mg x20 (Bagó) | $1.290 | 47 | **OK** |
| Ibuprofeno 400mg x10 (Andrómaco) | $1.850 | 3 | **Bajo** |
| Aspirina 100mg x30 (Bayer) | $2.100 | 0 | **Agotado** |

### La columna "Estado" — qué significa cada badge

El badge es la palabrita de colores al final de cada fila. Hay tres posibles:

- **OK** (verde) — el producto tiene stock suficiente, lo podés vender sin
  preocuparte.
- **Bajo** (amarillo) — el producto tiene 5 unidades o menos, o está debajo
  del mínimo configurado. Conviene anotarlo en la próxima orden de compra.
- **Agotado** (rojo) — el producto tiene 0 unidades. El sistema **no te va a
  dejar agregarlo al carrito** en el POS. Necesitás recibir mercadería antes
  de poder venderlo.

### El subtítulo debajo del nombre

Debajo del nombre del producto, en gris chiquito, aparece la **categoría** o la
**presentación** del producto. En el rubro **Farmacia** ahí se muestra el
**laboratorio** o el **principio activo** — te sirve para distinguir, por
ejemplo, dos paracetamoles de marcas distintas. En otros rubros esos campos no
se usan y simplemente no aparecen.

## Lo que NO podés hacer desde acá (todavía)

Esta pantalla es **sólo de consulta**. Desde el cajero o el encargado de stock:

- **No** podés cambiar el precio.
- **No** podés cargar mercadería nueva.
- **No** podés dar de baja un producto.

Esas operaciones son de **admin** y se hacen desde el panel de administración.
Si vos sos admin, hablá con tu técnico para que te muestre el flujo de
**compras / recepción** (orden de compra → recepción → ajuste de stock).

## Qué hacer si el stock del sistema no coincide con la realidad

Si contás físicamente un producto en la góndola y te da distinto al sistema:

1. **No** ajustes el stock por las tuyas. Eso deja un hueco en la auditoría.
2. Avisá al admin con: nombre del producto, conteo físico, conteo del sistema,
   hora del conteo.
3. El admin investiga si hubo una venta mal cargada, una merma, un robo o un
   error de recepción.

Todo movimiento de stock queda en un **log de auditoría** que se puede
consultar. Si alguien quiere saber "por qué se descontó esto", la respuesta
está siempre disponible.

## Tip — ¿qué productos están por vencer?

La tarjeta de "Stock bajo" no muestra vencimientos. Para ver productos que se
están por vencer, andá al módulo **Reportes** y mirá la sección
**Próximos a vencer** (si tu plan la incluye).

> Siguiente paso: [Mirar los reportes](./04-reportes.md).
