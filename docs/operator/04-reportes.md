---
title: 04 · Mirar los reportes
audience: admin, dueña/o, encargado responsable
---

# Mirar los reportes

Los **Reportes** te dicen, en una sola pantalla, **cómo va el negocio hoy**.
Es la primera pantalla que el dueño o la administradora abren en la mañana,
y la última que miran antes de irse.

Para entrar, hacé click en **Reportes** en el menú de la izquierda.

> (ver captura: vista de Reportes con Ventas hoy arriba, Top 5 y Inventario abajo)

La pantalla tiene **tres secciones**, una debajo de la otra.

---

## Sección 1 — Ventas de hoy

Arriba de todo, cuatro tarjetas grandes con los números del día:

### Tarjeta "Ventas hoy"

El **total recaudado en el día** (efectivo + tarjetas + cualquier otro medio).
Por ejemplo:

```
Ventas hoy
$284.530
12 venta(s)
```

La cifra grande es la plata total. Abajo te dice cuántas **boletas** se
emitieron.

### Tarjeta "Boletas"

El **número de boletas** que emitiste hoy. Sirve para tener una idea del
**flujo de público**: muchas boletas pequeñas = mucha rotación, pocas boletas
grandes = ticket promedio alto.

### Tarjeta "Efectivo"

Cuánto se cobró **en efectivo** específicamente. Este número tiene que
**coincidir con la plata real en la caja** al cierre del día (después de
sumar el fondo de apertura y restar los retiros).

Mirá la sección [Cierre de caja](./05-fin-de-dia.md) para el detalle.

### Tarjeta "Tarjeta"

Cuánto se cobró con **débito + crédito** sumados. Esta plata no está en la
caja física: está en proceso de pago en Transbank/Redcompra y llega a la
cuenta corriente en uno o dos días hábiles.

---

## Sección 2 — Top 5 productos

Una tabla con los **cinco productos más vendidos del día**. Las columnas son:

| # | Producto | Unid. | Ingresos | ABC |
|---|---|---|---|---|
| 1 | Paracetamol 500mg x20 | 34 | $43.860 | **A** |
| 2 | Ibuprofeno 400mg x10 | 22 | $40.700 | **A** |
| 3 | ... | | | |

### La columna "ABC" — qué significa

ABC es una clasificación clásica de inventario. El sistema te dice si un
producto es:

- **A** (verde) — producto **clave**. Aporta una porción grande de tus
  ingresos. Nunca puede faltar. Si se agota, perdés plata visible.
- **B** (amarillo) — producto **importante**. Aporta una porción media. Si
  falta un día, no es el fin del mundo, pero no querés que se agote
  seguido.
- **C** (gris) — producto de **rotación baja**. Te dejan margen pero se venden
  de a poco. Tener stock excesivo de productos C es plata muerta.

Pasá el mouse por encima del badge **A/B/C** y te muestra el porcentaje exacto
de ingresos que aporta ese producto.

### Si la tabla dice "Aún no hay ventas para rankear"

Es porque todavía no hiciste ninguna venta hoy. Volvé después del primer
cobro.

---

## Sección 3 — Inventario

Un mini-resumen del inventario (las mismas tarjetas que en la pantalla de
**Inventario**, pero condensadas):

- **Valorización** — cuánto vale tu stock al precio de venta.
- **Stock bajo** — cuántos productos están bajo el mínimo. Borde amarillo si
  hay alguno.
- **Sin stock** — cuántos productos están agotados. Borde rojo si hay alguno.

Es un atajo: te ahorra ir al módulo **Inventario** cuando sólo querés saber
estos tres números.

---

## Reportes premium (requieren licencia Pro / Business)

Algunas pantallas **no aparecen en el plan Free**. Cuando intentás abrirlas,
el sistema muestra un mensaje **402** (en español) tipo "Esta función
requiere actualizar el plan a Pro o superior". No es un error: es el sistema
diciéndote que necesitás contratar un plan más alto.

Los reportes premium son:

- **Márgenes diarios** — cuánto ganaste **realmente** por día (ingresos −
  costo de la mercadería vendida). Disponible en **Pro** o por
  microtransacción.
- **Top productos extendido** — más de 5 productos, con filtros por fecha,
  categoría, laboratorio.
- **Rotación de stock** — cuántas veces giró tu inventario en el mes.
  Disponible en **Business**.
- **Próximos a vencer** — productos cuyo lote vence en los próximos 60 / 90 /
  180 días. Disponible en **Business**.
- **ABC clasificación completa** — el ABC de TODOS tus productos, no sólo los
  Top 5. Disponible en **Business**.

> ¿Cómo subir de plan? Tu admin compra el plan desde el portal de RutBusiness
> (web). Cuando paga, se descarga un archivo `.lic` y tu técnico lo importa
> en el sistema. El cambio es **inmediato**: no hace falta reiniciar nada.
> Mirá `docs/product/license-activation.md` para el flujo técnico (es para
> tu técnico, no para vos).

## ¿Qué hago si los números no me cuadran?

- Si las **ventas hoy** del reporte no coinciden con lo que vos esperabas:
  esperá unos minutos (a veces los reportes se actualizan cada cierto tiempo)
  y volvé a abrir la pantalla.
- Si **siguen sin cuadrar**, hablá con tu admin. El sistema tiene un **log
  completo de auditoría** que permite reconstruir todas las ventas del día.

> Siguiente paso: [Cierre de caja al final del día](./05-fin-de-dia.md).
