package cl.rutbusiness.app.ui.caja

/**
 * Copy propio de [PasoArqueo] que no vive en [copyArqueoCaja] (`CopyCaja.kt`,
 * ADR-0022): la tranquilidad de que sobrar o faltar no traba el cierre.
 *
 * Archivo nuevo para no tocar `CopyCaja.kt` ni `Diferencia.kt` -son de otro
 * asiento de esta ola-, aunque las tres frases hablan del mismo momento.
 * `CopyCaja.kt` escribe la feria en voseo ("Contá", "escribí"); acá se usa
 * tuteo ("cuenta", "escribe"), igual que `CopyEscaner.kt` y `CopyCuenta.kt`.
 * Es un roce de tono entre archivos de distintos dueños que no se puede
 * resolver desde acá -ver el reporte de cierre de esta ola-.
 */

/**
 * Línea de tranquilidad, entre la nota y el botón de cerrar.
 *
 * Sin esta frase la dueña no sabe si un número que no calza la deja pegada
 * en la pantalla: se puede cerrar igual, sobre o falte, y esta es la única
 * parte del paso que lo dice **antes** de tocar el botón. Nunca «arqueo»,
 * «cuadrar», «descuadre», «saldo teórico» ni «conciliar» -esas palabras no
 * las usa la dueña de un puesto ni de un almacén-, y nunca "te sobra" o
 * "te falta" en tono de reto: la plata sobra o falta, la persona no se
 * equivocó.
 */
internal fun copyTranquilidadArqueo(feria: Boolean): String =
    if (feria) {
        "Si sobra o falta un poco, no importa: el día se cierra igual. " +
            "Después te mostramos cómo quedó."
    } else {
        "Si sobra o falta un poco, no importa: la caja se cierra igual. " +
            "Después te mostramos cómo quedó contra lo anotado."
    }
