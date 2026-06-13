---
title: 07 · Respaldo de la información
audience: administradores y dueños de farmacia (con apoyo del técnico)
---

# Respaldo de la información

Este capítulo es para el **administrador**. Explica qué es un respaldo, cómo
verificar que se está haciendo, y qué hacer para recuperar la información si
algún día hiciera falta.

> Toda la información de la farmacia (productos, stock, ventas, clientes,
> boletas) vive en el computador del local. Un **respaldo** es una copia de
> seguridad de esa información, por si el computador se daña o se roba. Tenerlo
> al día es lo que te deja dormir tranquilo.

## Qué es un respaldo en este sistema

El sistema guarda un **snapshot**: un único archivo comprimido
(`pharma-backup-<fecha>.tar.gz`) con toda la base de datos. Por defecto se
guarda en la carpeta `backups` dentro de los datos del servidor. Ese archivo es
todo lo que necesitás para restaurar la farmacia en otro computador.

## Respaldo automático (recomendado)

El sistema puede sacar un respaldo **solo, todas las noches**, sin que nadie
tenga que acordarse. Esto lo deja activado **el técnico** una sola vez, eligiendo
la hora (lo habitual es las **3:00 AM**, cuando la farmacia está cerrada) y
cuántos días de respaldos guardar.

> Por defecto el respaldo automático viene **apagado**. Pedile al técnico que lo
> active al instalar; es el paso más importante de la puesta en marcha.

### Cómo verificar que el respaldo nocturno corrió

1. Pedí al técnico (o seguí su instructivo) para abrir la carpeta `backups`.
2. Mirá que haya un archivo `pharma-backup-…` con la **fecha de hoy** (o de la
   última noche).
3. Si el archivo más nuevo tiene varios días, el respaldo automático no está
   corriendo: avisá al técnico.

Una buena rutina: el primer día de cada semana, mirá que el respaldo más reciente
sea de la noche anterior.

## Respaldo manual (cuando quieras uno extra)

Antes de algo importante (actualizar el sistema, mover el computador) conviene
sacar un respaldo a mano. Lo hace el técnico o el administrador desde la consola
del servidor:

```text
pharma backup create
```

Esto genera un archivo nuevo en la carpeta `backups`. Para ver los respaldos
disponibles, del más nuevo al más viejo:

```text
pharma backup list
```

## Guardá una copia FUERA del computador

Un respaldo que vive en el **mismo** computador no sirve si ese computador se
daña o se roba. La regla de oro:

> Copiá el archivo de respaldo más reciente a un **pendrive** o a un **disco
> externo** una vez por semana, y guardalo en otro lugar. Si usás algún
> almacenamiento en la nube de tu confianza, también vale.

## Recuperar la información (restaurar)

Esto se hace **solo si pasó algo** (el computador se dañó, hay que reinstalar).
Lo ejecuta el técnico, porque **sobreescribe** la base actual y el servidor debe
estar **detenido** mientras se restaura:

```text
pharma backup restore <ruta-al-archivo.tar.gz>
```

Después se vuelve a iniciar el servicio y la farmacia queda tal como estaba en la
fecha de ese respaldo.

> **Importante**: restaurar reemplaza todo lo que haya ahora por lo que estaba en
> el respaldo. Nunca lo hagas "para probar" sobre la farmacia en uso — es una
> operación de emergencia. Ante la duda, llamá al técnico.

## Resumen para el administrador

| Tarea | Cada cuánto | Quién |
|---|---|---|
| Verificar que el respaldo nocturno corrió | Semanal | Administrador |
| Copiar el respaldo a un pendrive / disco externo | Semanal | Administrador |
| Respaldo manual antes de un cambio importante | Cuando aplique | Técnico / Admin |
| Restaurar desde un respaldo | Solo en emergencia | Técnico |
