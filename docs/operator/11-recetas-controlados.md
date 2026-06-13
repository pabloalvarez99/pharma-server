---
title: 11 · Recetas y libro de controlados
audience: químicos farmacéuticos y administradores
---

# Recetas y libro de controlados (Ley 20.000)

El módulo **Recetas** lleva el registro de las recetas que dispensás y, en
particular, el **libro de medicamentos controlados** que exige la Ley 20.000.
Cada receta queda guardada de forma **permanente**: una vez registrada **no se
puede editar ni borrar** (así lo exige la ley), por eso conviene cargarla con
cuidado.

> Este módulo lo usa el **químico farmacéutico**. El administrador también tiene
> acceso.

## Para qué sirve

- Dejar registro de **quién** retiró un medicamento, **qué** médico lo recetó y
  **cuándo** se dispensó.
- Mantener el **libro de controlados** al día, listo para una fiscalización del
  ISP / Seremi de Salud.
- Exportar ese libro a un archivo **CSV** para tu respaldo o tu contador.

## Registrar una receta

1. Entrá a **Recetas** en el menú de la izquierda.
2. **Nueva receta**.
3. Completá:
   - **Paciente** y **RUT del paciente**.
   - **Medicamento controlado (Ley 20.000)** — marcá esta casilla si el
     medicamento es controlado. **Importante**: al marcarla, el **médico** y el
     **RUT del médico** pasan a ser **obligatorios** (la ley exige identificar
     al profesional que prescribe un controlado).
   - **Médico** y **RUT del médico**.
   - **Folio** de la receta (si corresponde).
4. Guardá. La receta queda registrada con la fecha y hora de dispensación.

> Revisá los datos **antes** de guardar: la receta no se puede modificar después.
> Si te equivocaste, registrá una nota en tu sistema interno y, si hace falta,
> consultá con tu contador o la autoridad sanitaria cómo corregir el libro.

## Buscar y consultar recetas

- Usá el buscador **Filtrar por RUT del paciente** para ver todas las recetas de
  una persona.
- Activá **Solo controlados** para ver únicamente los medicamentos controlados.

La lista muestra paciente, RUT, médico, folio y fecha de cada receta.

## El libro de controlados

Más abajo está la sección **Libro de recetas · controlados**, que reúne sólo las
recetas de medicamentos controlados — es tu libro legal en pantalla.

### Exportar a CSV

El botón **Exportar CSV** te baja el libro en un archivo de planilla
(Excel / LibreOffice). Usalo para:

- Guardar un respaldo periódico del libro.
- Entregárselo a tu contador o a la autoridad si te lo piden.

El archivo respeta el filtro activo: si estás filtrando por un RUT, exporta solo
esas recetas; sin filtro, exporta todo el libro.

## Buenas prácticas

- Registrá la receta **en el momento** de dispensar, no al final del día (es más
  fácil que queden datos correctos y completos).
- Para controlados, tené a la vista la receta física: vas a necesitar el RUT del
  médico y el folio.
- Exportá el CSV del libro **una vez al mes** y guardalo junto con tus respaldos
  (ver el capítulo [Respaldo de la información](./07-respaldo.md)).

## Problemas comunes

| Qué ves | Qué significa | Qué hacer |
|---|---|---|
| No deja guardar un controlado | Falta el médico o su RUT | Completá médico + RUT del médico (obligatorios en controlados) |
| No encuentro una receta | El filtro por RUT no coincide | Revisá el RUT escrito, o limpiá el filtro para ver todas |
| Me equivoqué y ya guardé | Las recetas son inmutables (Ley 20.000) | No se edita; dejá constancia y consultá el procedimiento de corrección |
| El CSV salió incompleto | Tenías un filtro activo | Limpiá el filtro y exportá de nuevo para el libro completo |
