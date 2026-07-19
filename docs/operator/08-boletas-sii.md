---
title: 08 · Boletas electrónicas SII
audience: dueños y administradores
---

# Boletas electrónicas (SII) — paso a paso

> Las boletas y facturas electrónicas SII son **universales**: las emite
> cualquier negocio en Chile, sea farmacia, minimarket, restaurant o servicios.
> Este capítulo aplica a **todos los rubros**.

Este capítulo explica cómo dejar tu negocio emitiendo **boletas electrónicas**
válidas ante el SII, cómo emitirlas desde la aplicación, y cómo descargar el
**libro de ventas** del mes para tu contador.

> **Antes de empezar**: la parte de emitir boletas la puede hacer cualquier
> persona con rol cajero. La configuración inicial (certificado, folios, datos
> de la empresa) la hace **una sola vez** quien tenga rol administrador.

## Qué necesitás (una sola vez)

Para emitir boletas electrónicas el SII exige tres cosas:

1. **Certificado digital** (archivo `.pfx` con clave). Es la "firma" de la
   empresa. Se compra a un proveedor autorizado (E-CertChile, Acepta, etc.)
   con el RUT de la empresa.
2. **CAF** (Código de Autorización de Folios). Es un archivo XML que el SII
   te entrega gratis desde su sitio web y que autoriza un rango de números
   de boleta (folios). Cuando se acaban, pedís otro.
3. **Datos del emisor**: RUT, razón social, giro, dirección y comuna del
   negocio, tal como están registrados en el SII.

## Configuración inicial

### Paso 1 — Cargá los datos del emisor

1. Entrá a **Configuración** en el menú de la izquierda.
2. Buscá la sección **Emisor DTE (SII)**.
3. Completá RUT, razón social, giro, dirección y comuna **exactamente como
   figuran en el SII** y guardá.

### Paso 2 — Importá el certificado digital

El certificado se importa desde la consola del servidor (lo hace el técnico o
el administrador, una sola vez):

```text
pharma cert import C:\ruta\al\certificado.pfx
```

El sistema te pide la **clave del certificado** (la misma que te dio el
proveedor). La clave no queda guardada: cada vez que se firma una boleta, el
sistema la vuelve a pedir. El archivo queda cifrado dentro del servidor.

### Paso 3 — Importá el CAF (los folios)

Descargá el CAF para "Boleta electrónica" (tipo 39) desde el sitio del SII y
cargalo:

```text
pharma caf import C:\ruta\al\CAF39.xml
```

En la vista **Boletas** de la aplicación vas a ver arriba un aviso con cuántos
folios te quedan. Cuando esté en rojo, pedí un CAF nuevo al SII e importalo
igual que el primero — el sistema sigue solo con el rango nuevo.

### Paso 4 — Ambiente SII (dejalo como está si no sabés)

En **Configuración → Ambiente SII** podés elegir:

- **Sandbox (pruebas)** — las boletas se envían al ambiente de certificación
  del SII (`maullin.sii.cl`). Es el valor por defecto y sirve para probar sin
  consecuencias tributarias.
- **Producción** — boletas reales (`palena.sii.cl`). Cambiá a producción
  **sólo cuando el SII haya certificado a tu empresa**. El sistema te pide
  confirmación extra para este cambio.

## Emitir una boleta

1. Cobrá la venta en el **POS** como siempre. Al cobrar, el sistema te
   muestra el **número de orden** (algo como `order:abc123`).
2. Entrá a **Boletas** en el menú de la izquierda.
3. En **Emitir boleta de una venta**, pegá el número de orden, escribí la
   **clave del certificado** y (opcional) el RUT del cliente si pidió boleta
   con su RUT. Si lo dejás vacío, sale como consumidor final.
4. Hacé click en **Emitir y firmar**.

La boleta queda **firmada localmente** con tu certificado y aparece en el
listado de abajo con estado **Firmada**. Eso ya es una boleta electrónica
completa: tiene su folio, su timbre electrónico y la firma de la empresa.

### ¿Y el envío al SII?

Depende de tu plan:

- **Plan Free** — el envío automático no está incluido, pero **no estás
  bloqueado**: con el botón **XML** descargás el archivo firmado de cada
  boleta y lo subís a mano en el sitio del SII (sección "envío de DTE").
  Tus datos siempre son tuyos.
- **Plan Pro o superior** — botón **Enviar SII** directo desde el listado.
  Después usás **Consultar** para ver si el SII la aceptó o rechazó. Si la
  rechaza, el sistema te muestra el motivo.

### Anular una boleta

Si te equivocaste y la boleta **todavía no fue enviada** al SII, usá el botón
**Anular** (te pide el motivo). El folio queda registrado como anulado.

Si la boleta **ya fue aceptada** por el SII, no se anula: se emite una **nota
de crédito** que la deja sin efecto (ver más abajo).

## Libro de ventas mensual (para tu contador)

En la vista **Boletas**, panel **Libro de ventas mensual**:

1. Elegí el **mes**.
2. **Descargar XML** te baja el libro sin firma — sirve para que tu contador
   revise el detalle del mes.
3. **Descargar firmado** (pide la clave del certificado) te baja el libro
   firmado, listo para subir al portal del SII.

El libro incluye **sólo las boletas aceptadas** por el SII en ese mes, con el
resumen por tipo de documento y el detalle folio por folio.

## Facturas, notas de crédito/débito y guías de despacho

Además de boletas, el sistema emite **factura electrónica (33)**, **nota de
débito (56)**, **nota de crédito (61)** y **guía de despacho (52)** desde la
pantalla **Facturas**. El paso a paso completo está en el capítulo
[Facturas, notas y guías](./09-facturas-notas-guias.md). Lo esencial:

- Cada tipo necesita **su propio CAF** (igual que la boleta: se descarga del
  SII y se importa con `pharma caf import`).
- La factura exige los **datos completos del cliente**: RUT, razón social,
  giro, dirección y comuna.
- Las notas de crédito/débito siempre **referencian el documento original**
  (qué folio corrigen y por qué: anula / corrige texto / corrige montos).
- La guía de despacho lleva el **motivo del traslado** (venta, traslado
  interno entre sucursales, etc.). Un traslado interno puede ir con valores
  en cero.

## Problemas comunes

| Qué ves | Qué significa | Qué hacer |
|---|---|---|
| "No hay folios autorizados cargados" | Falta importar el CAF | `pharma caf import <CAF.xml>` |
| "Folios agotados" | Se terminó el rango del CAF | Pedir CAF nuevo al SII e importarlo |
| "passphrase incorrecta" al emitir | La clave del certificado no es | Reintentá; si la perdiste, pedí reemisión del certificado al proveedor |
| "Falta configurar el emisor DTE" | No están los datos de la empresa | Configuración → Emisor DTE (SII) |
| Boleta **Rechazada** | El SII no la aceptó | Mirá el motivo en la columna SII; corregí y reenviá |
| "Certificado digital expirado" | El certificado venció | Comprá la renovación al proveedor e importá el `.pfx` nuevo |

> **Regla de oro**: la clave del certificado **no se guarda nunca** en el
> sistema ni viaja por internet en claro. Si alguien te pide "dejarla
> grabada", la respuesta es no.
