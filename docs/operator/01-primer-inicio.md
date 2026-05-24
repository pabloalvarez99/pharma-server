---
title: 01 · Tu primer inicio de sesión
audience: todos los operadores, primer día
---

# Tu primer inicio de sesión

La primera vez que abrís Tu Farmacia, vas a ver la **pantalla de inicio de
sesión**. No te asustés, son sólo cuatro campos. Vamos uno por uno.

## Paso 1 — Abrí la aplicación

Hacé doble click en el icono **Tu Farmacia** del escritorio. Esperá unos
segundos. Vas a ver cómo aparece el panel azul con el logo.

> (ver captura: pantalla de login al inicio)

## Paso 2 — Llená los campos

La pantalla tiene tres campos en este orden:

### Sucursal

Es el nombre corto de tu local. Tu administrador o tu técnico te lo va a decir.
En la mayoría de las farmacias dice simplemente `principal`, o `coquimbo`, o el
nombre del barrio.

- Lo que escribís acá **no** se distingue entre mayúsculas y minúsculas, pero
  por orden, escribilo siempre en minúsculas.
- Si ves que ya viene con un valor escrito por defecto (por ejemplo
  `tufarmacia`), dejalo así salvo que tu admin te haya dicho otra cosa.

### Correo

Es tu **correo electrónico personal** dentro del sistema. Lo registró el dueño
o el admin cuando te dieron acceso. Por ejemplo:

- `cajera@tufarmacia.cl`
- `juan.perez@tufarmacia.cl`
- `dueno@tufarmacia.cl`

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
- Mensajes típicos en español:
  - "Sucursal o credenciales inválidas" — revisá los tres campos.
  - "No se pudo conectar al servidor" — avisá a tu técnico. El servidor del
    local podría estar apagado.
  - "Contraseña requerida" — te olvidaste de llenar el campo.

## El campo "Conexión avanzada" — qué es y cuándo abrirlo

Debajo del campo de contraseña vas a ver un texto chiquito gris que dice
**Conexión avanzada** con una flechita ▾.

- **Operador normal**: no lo abras nunca. El sistema ya viene configurado.
- **Tu técnico te pide cambiar el servidor**: lo abrís, ves un campo que dice
  **Servidor** con un valor tipo `http://127.0.0.1:8080`. Cambialo SOLO si tu
  técnico te dictó el valor exacto.

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
