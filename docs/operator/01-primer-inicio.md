---
title: 01 · Tu primer inicio de sesión
audience: todos los operadores, primer día
---

# Tu primer inicio de sesión

Hay **dos situaciones** distintas la primera vez que abrís RutBusiness:

1. **Servidor recién instalado, sin ninguna cuenta todavía** → te aparece la
   pantalla **Crea tu cuenta** (la dueña del negocio). Esto pasa una sola vez.
2. **El servidor ya tiene cuenta** (tu admin ya la creó) → te aparece la
   pantalla normal de **Iniciar sesión** con Sucursal, Correo y Contraseña.

Abajo explicamos las dos.

## Caso 1 — Crear la cuenta del negocio (primer arranque)

La primera vez que se abre un servidor nuevo, RutBusiness detecta que **todavía
no hay ninguna cuenta** y te muestra la pantalla **Crea tu cuenta**. A la
izquierda dice *Bienvenido. Es la primera vez que abres este servidor.* Esta
cuenta será la **dueña** del negocio.

> (ver captura: pantalla "Crea tu cuenta" del primer arranque)

Llená estos campos:

### Nombre del negocio

El nombre de tu local, como querés que aparezca. Por ejemplo `Almacén Don José`,
`Minimarket La Esquina` o el nombre de tu farmacia.

### Rubro

Un menú para elegir **a qué se dedica tu negocio**. Esto define qué secciones
del sistema se muestran. Las opciones:

- 💊 **Farmacia** — habilita Recetas y el Libro de controlados (Ley 20.000),
  principio activo y lotes.
- 🛒 **Minimarket / Almacén** — abarrotes y perecibles. POS, inventario y
  boletas. Sin recetas.
- 🍽 **Restaurant / Comida**
- ☕ **Café / Pastelería**
- 🛍 **Tienda / Retail**
- 💅 **Belleza / Estética** — servicios, poco stock físico.
- 🔧 **Servicios / Oficios** — ventas de servicios sin inventario físico.
- ➕ **Otro** — ERP genérico, sin secciones específicas de un rubro.

No te preocupes si dudás: el rubro lo podés **cambiar después** en
**Configuración**. Las boletas y facturas SII funcionan en todos los rubros.

### Correo

El correo electrónico de la cuenta dueña. Por ejemplo `dueno@minegocio.cl`. Con
este correo vas a iniciar sesión después.

### Contraseña

Tu clave. **Mínimo 8 caracteres.** Anotala en un lugar seguro: es la cuenta
dueña del negocio.

### Conexión avanzada

Igual que en el login normal: sólo la abrís si tu técnico te dictó otra
dirección de servidor.

Cuando todo esté completo, tocás **CREAR CUENTA Y ENTRAR**. El sistema crea la
cuenta, la sucursal y te lleva directo al panel principal — sin pasar por
comandos ni configuración técnica.

> **¿Querés probar con datos de ejemplo?** Una vez adentro, en
> **Configuración** podés tocar **Cargar datos demo** para llenar la aplicación
> con productos, ventas y clientes de prueba de tu rubro. Sirve para practicar
> antes de cargar tus datos reales. Ver
> [Cargar tu catálogo](./13-importar-datos.md).

## Caso 2 — Iniciar sesión (uso de todos los días)

Si el servidor ya tiene cuenta, al abrir RutBusiness vas a ver la **pantalla de
inicio de sesión** con tres campos. Vamos uno por uno.

## Paso 1 — Abrí la aplicación

Hacé doble click en el icono **RutBusiness** del escritorio. Esperá unos
segundos. Vas a ver cómo aparece el panel con el logo.

> (ver captura: pantalla de login al inicio)

## Paso 2 — Llená los campos

La pantalla tiene tres campos en este orden:

### Sucursal

Es el nombre corto de tu local. Tu administrador o tu técnico te lo va a decir.
En la mayoría de los locales dice simplemente `principal`, o el nombre del
barrio o la ciudad.

- Lo que escribís acá **no** se distingue entre mayúsculas y minúsculas, pero
  por orden, escribilo siempre en minúsculas.
- Si ves que ya viene con un valor escrito por defecto, dejalo así salvo que tu
  admin te haya dicho otra cosa.

### Correo

Es tu **correo electrónico personal** dentro del sistema. Lo registró el dueño
o el admin cuando te dieron acceso. Por ejemplo:

- `cajera@minegocio.cl`
- `juan.perez@minegocio.cl`
- `dueno@minegocio.cl`

Si no recordás cuál es el tuyo, preguntáselo a tu administradora. **No
inventés** un correo que no esté registrado: el sistema no te va a dejar entrar.

### Contraseña

Es tu clave personal. La elegiste cuando te dieron de alta o te la entregó tu
admin en un papel para que la cambies después.

A la derecha del campo hay un **ícono de ojo**. Si lo tocás, la contraseña se
muestra mientras escribís (por si tenés dudas). Si lo tocás de nuevo, se vuelve
a esconder.

**Importante**:
- La contraseña distingue mayúsculas de minúsculas. `Abc123` y `abc123` son
  contraseñas distintas.
- Si la escribís mal tres veces seguidas, el sistema NO te bloquea, pero te va
  a mostrar un mensaje claro en español diciendo que las credenciales no
  coinciden. Respirá, fijate qué escribiste, y volvé a intentar.

## Paso 3 — Tocá ENTRAR

Cuando los tres campos estén completos, hacé click en el botón grande que dice
**ENTRAR**.

Si todo está bien:
- El botón empieza a **pulsar** (indica que está procesando).
- Después de un segundo o dos, la pantalla cambia y aparece el **panel
  principal** con el menú a la izquierda.

Si algo está mal:
- Aparece un mensaje en rojo abajo del formulario o debajo del campo
  problemático.
- Mensajes típicos en español (tal como los vas a ver en pantalla):
  - "Indica tu sucursal." / "Indica tu correo." / "La contraseña es
    obligatoria." — te faltó llenar ese campo.
  - "No se pudo contactar al servidor." — avisá a tu técnico. El servidor del
    local podría estar apagado.
  - Si el correo o la contraseña no coinciden con los registrados, el sistema
    te lo dice y no te deja entrar — revisá lo que escribiste.

## El campo "Conexión avanzada" — qué es y cuándo abrirlo

Debajo del campo de contraseña vas a ver un texto chiquito gris que dice
**Conexión avanzada** con una flechita ▾.

- **Operador normal**: no lo abras nunca. El sistema ya viene configurado.
- **Tu técnico te pide cambiar el servidor**: lo abrís, ves un campo que dice
  **Servidor** con un valor tipo `http://127.0.0.1:8080`. Si el servidor corre
  en otro equipo de la red, escribís su IP y puerto (ej:
  `http://192.168.1.50:8080`), pero **sólo** si tu técnico te dictó el valor
  exacto. El botón **Probar conexión** te dice ahí mismo si el servidor responde,
  sin tener que iniciar sesión.

## ¿Y si me olvidé la contraseña?

No la podés recuperar vos sola/o. Lo que tenés que hacer:

1. Hablá con tu administrador o dueño.
2. Él/ella usa una herramienta de admin para generarte una contraseña nueva.
3. Te la entrega en un papel o por mensaje.
4. La primera vez que entrés con la clave nueva, el sistema te pide cambiarla.

Mirá la sección "Olvidé mi contraseña" en
[Problemas comunes](./06-problemas-comunes.md).

## ¿Qué pasa cuando termino mi turno?

En el menú de la izquierda, abajo de todo, hay un botón gris que dice
**Cerrar sesión**. Hacé click ahí.

El sistema te va a devolver a la pantalla de login. Tu sesión queda cerrada y
nadie puede usar el sistema con tu usuario hasta que vuelvas a poner tu
contraseña. **Cerrá sesión siempre antes de irte**, sobre todo si el computador
está en un lugar accesible al público.

> Siguiente paso: [Vender en el POS](./02-pos-cobrar.md).
