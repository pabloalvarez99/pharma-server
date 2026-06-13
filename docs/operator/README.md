---
title: Manual del operador — Tu Farmacia
audience: cajeros, químicos farmacéuticos, dueños y administradores de farmacia
status: vigente
idioma: español (Chile)
last_review: 2026-05-24
---

# Manual del operador — Tu Farmacia

Bienvenida y bienvenido. Este manual está pensado para que cualquier persona que
trabaja en la farmacia pueda **usar el sistema desde el día uno**, sin saber de
computadores ni de programación. Está escrito en español, con palabras simples,
mostrando exactamente lo que vas a ver en la pantalla.

Si encontrás un botón o una palabra que no entendés, buscá esa palabra en el
índice de abajo. Cada sección es corta y va al grano.

## Para quién es este manual

- **Cajeras y cajeros** — la persona que está en el mostrador atendiendo público
  y registrando ventas.
- **Químicos y químicas farmacéuticas** — quien revisa stock, vencimientos y
  recetas.
- **Dueño o administradora** — quien mira los reportes del día, hace el cierre
  de caja y verifica que el sistema esté funcionando.

**No** es un manual para programadores ni para el técnico que instala el
servidor. Para eso existe el manual técnico (`docs/ops/`).

## Índice

1. [Bienvenida y vista general](./00-bienvenida.md)
   — qué es la aplicación, cómo se ve, qué módulos tiene.
2. [Tu primer inicio de sesión](./01-primer-inicio.md)
   — cómo entrar al sistema la primera vez, qué significa cada campo.
3. [Vender en el POS](./02-pos-cobrar.md)
   — cómo buscar un producto, armar el carrito y cobrar.
4. [Revisar el inventario](./03-inventario.md)
   — cómo ver el stock, las valorizaciones y los productos por estado.
5. [Mirar los reportes](./04-reportes.md)
   — ventas del día, productos más vendidos, márgenes.
6. [Cierre de caja al final del día](./05-fin-de-dia.md)
   — cómo cerrar la jornada y qué hacer con la plata.
7. [Problemas comunes y cómo resolverlos](./06-problemas-comunes.md)
   — qué hacer si algo no funciona como esperás.
8. [Respaldo de la información](./07-respaldo.md)
   — para el administrador: cómo verificar que el respaldo nocturno corrió.
9. [Boletas electrónicas SII](./08-boletas-sii.md)
   — certificado digital, folios (CAF), emitir y firmar boletas, libro de
   ventas mensual para el contador.
10. [Facturas, notas y guías](./09-facturas-notas-guias.md)
   — emitir factura (33), nota de crédito/débito (61/56) y guía de despacho
   (52): validación del RUT, desglose neto/IVA en vivo y envío al SII.

## Cómo imprimir todo el manual

Si necesitás una copia en papel para tenerla en el mostrador, abrí cada archivo
de la lista de arriba en tu navegador o editor de Markdown, y usá la opción
**Imprimir** del menú **Archivo**. Recomendado: imprimir las páginas 3, 4 y 7
plastificadas, son las que más se consultan en el día a día.

Si tu computador tiene **Pandoc** o **glow** instalados, tu técnico puede armar
un PDF único de todo el manual con un solo comando (no es tarea del operador,
se la pedís a él/ella).

## Una nota antes de empezar

El sistema **funciona sin internet**. Eso quiere decir que aunque se corte la
luz del módem o el WiFi se caiga, vos podés seguir cobrando, registrando ventas
y consultando el stock con normalidad. Toda la información vive en el computador
de la farmacia, no en una nube lejana. Si el computador se apaga, la
información sigue ahí cuando se prende otra vez.

Cualquier duda que no resuelva este manual, escribíle a soporte.
