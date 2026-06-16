---
title: 14 · Checklist del primer día (piloto)
audience: dueño/dueña que instala y prueba solo, sin técnico
status: vigente
---

# Checklist del primer día — del MSI a tu primera venta, sin ayuda

Esta hoja es para **el dueño o la dueña que arranca el negocio solo**, sobre una
instalación **recién hecha y vacía**, **sin tocar la línea de comandos** y sin
llamar al técnico. Si seguís los pasos en orden, en menos de diez minutos vas a
estar haciendo tu **primera venta de verdad**.

> Imprimí esta hoja o tenela en otra pantalla mientras lo hacés. Cada paso tiene
> una casilla **✓ Cómo sé que salió bien**: si lo que ves coincide, seguí; si no,
> mirá [Problemas comunes](./06-problemas-comunes.md).

Sirve para **cualquier rubro** (farmacia, minimarket, restaurant, tienda,
servicios…). Donde algo aplica sólo a farmacia, está marcado **(solo farmacia)**.

---

## Paso 0 — Instalar RutBusiness

1. Hacé doble click en el instalador (`RutBusiness-x.y.z.msi`).
2. Aceptá los pasos (Siguiente → Instalar). Pedirá permiso de administrador una
   vez: aceptá.
3. Al terminar, en el **escritorio** aparece el icono **RutBusiness**.

> **✓ Cómo sé que salió bien:** hay un icono **RutBusiness** en el escritorio y
> el servidor del local quedó andando solo (es un servicio de Windows, no tenés
> que abrir nada más).

> Si Windows muestra una advertencia de "editor desconocido" (SmartScreen),
> tocá **Más información → Ejecutar de todas formas**. Es normal en esta etapa
> de piloto; el instalador todavía no está firmado.

---

## Paso 1 — Abrir la app y crear tu cuenta (primer arranque)

1. Doble click en el icono **RutBusiness**. Esperá unos segundos.
2. Como el servidor está **recién instalado y sin ninguna cuenta**, la app NO te
   pide iniciar sesión: te muestra directamente la pantalla **Crea tu cuenta**.
   A la izquierda dice *Bienvenido. Es la primera vez que abres este servidor.*

Llená el formulario:

- **Nombre del negocio** — como querés que se vea (ej: `Almacén Don José`).
- **Rubro** — elegí del menú a qué se dedica tu negocio. Define qué secciones se
  muestran (las **Recetas** sólo en Farmacia). Lo podés cambiar después.
- **Correo** — el correo de la cuenta dueña (ej: `dueno@minegocio.cl`). Con este
  vas a entrar siempre.
- **Contraseña** — mínimo **8 caracteres**. Anotala en un lugar seguro.

Tocá **CREAR CUENTA Y ENTRAR**.

> **✓ Cómo sé que salió bien:** el botón pulsa, dice **LISTO**, y la pantalla
> cambia sola al **panel principal** con el menú a la izquierda. **No tuviste
> que abrir ninguna ventana negra de comandos.** Esto reemplaza por completo a
> `pharma tenant-create` y `pharma user-create`: el dueño ya no necesita CLI.

> Detalle: no llenaste **Sucursal** acá. El sistema crea la sucursal por vos a
> partir del nombre del negocio. La próxima vez que abras la app vas a entrar
> por la pantalla normal de login, y la **Sucursal** ya vendrá pre-cargada.

---

## Paso 2 — (Opcional pero recomendado) cargar datos demo para practicar

Antes de cargar tu catálogo real, conviene **practicar** con datos de ejemplo de
tu rubro. Así ves el POS, el inventario y los reportes con datos creíbles.

1. En el menú izquierdo, abrí **Configuración**.
2. Bajá hasta **Rubro del negocio**. Verificá que la grilla muestre tu rubro
   **seleccionado** (recuadro marcado). Los rubros con datos de ejemplo tienen la
   etiqueta **datos demo** (hoy: **Farmacia** y **Minimarket**).
3. Bajá al bloque **Cargar datos demo** y tocá **Cargar datos demo**. Confirmá el
   aviso (es sólo para una instalación de prueba).

> **✓ Cómo sé que salió bien:** aparece un mensaje verde tipo *"Listo: N
> productos, N lotes, N ventas."* Si tu rubro **no** tiene pack demo todavía, el
> botón está desactivado y dice *"Este rubro aún no tiene pack de datos demo."* —
> en ese caso saltá al Paso 3 e importá tu catálogo real.

> **No cargues datos demo sobre datos reales.** Cuando ya estés trabajando de
> verdad, no toques este botón. Ver [Cargar tu catálogo](./13-importar-datos.md).

---

## Paso 3 — (Si ya tenés tu catálogo) importarlo desde Excel/CSV

Si en vez de practicar querés arrancar con **tus** productos:

1. En el menú, abrí **Importar**.
2. Tocá **Elegir archivo CSV**, elegí tu planilla, y tocá **Previsualizar**.
3. Revisá el resumen (cuántos se crean / actualizan / fallan). Si cuadra, tocá
   **Confirmar importación**.

> **✓ Cómo sé que salió bien:** después de confirmar, **Inventario** muestra tus
> productos. El detalle de columnas y comodidades para Excel chileno está en
> [Cargar tu catálogo](./13-importar-datos.md).

---

## Paso 4 — Abrir la caja del día

No podés cobrar con la caja cerrada. Antes de vender:

1. En el menú, abrí **Caja**.
2. Tocá **Abrir caja** e indicá el **monto inicial** (la plata con la que
   arrancás el cajón). Confirmá.

> **✓ Cómo sé que salió bien:** la pantalla de Caja pasa a estado **abierta** y
> te deja registrar ventas. Detalle en
> [Cierre de caja al final del día](./05-fin-de-dia.md).

---

## Paso 5 — Tu primera venta

1. En el menú, abrí **POS**.
2. Buscá un producto (escribí su nombre o pasá el lector de código de barras) y
   agregalo al carrito. Ajustá la cantidad si hace falta.
3. Tocá **Cobrar**, elegí **Efectivo**, escribí con cuánto paga el cliente y
   confirmá. El sistema te muestra el **vuelto**.

> **✓ Cómo sé que salió bien:** se genera la venta, el carrito se vacía y el
> **stock baja** en Inventario. El paso a paso completo (descuentos, cliente,
> medios de pago) está en [Vender en el POS](./02-pos-cobrar.md).

> (solo farmacia) Si vendiste un producto con receta o controlado, el POS te lo
> pide en su momento. En otros rubros eso no aparece.

---

## Paso 6 — Cerrar el día

Al terminar la jornada:

1. **Caja → Cerrar caja**: contá la plata del cajón y registrá el arqueo. El
   sistema te dice si cuadra, sobra o falta.
2. **Cerrar sesión** (botón gris, abajo del menú) antes de irte.

> **✓ Cómo sé que salió bien:** el cierre queda registrado y volvés a la pantalla
> de login. Ver [Cierre de caja](./05-fin-de-dia.md).

> El **respaldo** de toda tu información corre **solo, cada noche**. No tenés que
> hacer nada. Para verificarlo, ver [Respaldo](./07-respaldo.md).

---

## Lo que NUNCA necesitás para el primer día

- **No** necesitás abrir la "ventana negra" de comandos (terminal/PowerShell).
- **No** necesitás escribir comandos `pharma ...`.
- **No** necesitás instalar nada más (ni Docker, ni base de datos aparte).
- **No** necesitás internet para vender: todo vive en el computador del local.

Lo único que un **técnico** podría hacer aparte: emitir **boletas electrónicas
SII** (necesita certificado digital y folios) — ver
[Boletas electrónicas SII](./08-boletas-sii.md). Para **vender y cobrar** no hace
falta nada de eso.

---

## Resumen en una línea

**Instalar → Crear cuenta y elegir rubro → (demo o importar) → Abrir caja →
Vender → Cerrar caja.** Todo desde la app, vos solo/a.

> Volvé al [índice](./README.md).
