package cl.rutbusiness.app.ui.caja

/**
 * Copy del desglose y del error del monto esperado en la pantalla
 * «puesto / caja abierta».
 *
 * Chip y CTAs siguen en [copyCajaAbierta] ([CopyCaja]). Acá vive lo que antes
 * estaba hardcodeado en [PasoCajaAbierta]: el error cuando no llega el
 * esperado, el título del desglose, las cuatro líneas y la palabra del
 * movimiento.
 *
 * Feria: cuaderno / puesto / día / billete. Nunca sistema, cajón, arqueo,
 * sesión, transacción ni «cuenta» como ledger. Retail puede seguir con caja.
 *
 * Funciones puras para gate en JVM sin montar Compose.
 */

/** Cuando el server no mandó el monto grande de «debería haber». */
internal fun copyErrorEsperadoAbierta(feria: Boolean): String =
    if (feria) {
        "No pudimos traer cuánta plata debería haber en el puesto. El día " +
            "sigue abierto y podés seguir vendiendo; tocá «Actualizar» arriba " +
            "para volver a pedirla."
    } else {
        "No pudimos traer la cuenta de la caja. La caja sigue abierta y puedes " +
            "seguir vendiendo; toca «Actualizar» arriba para volver a pedirla."
    }

/**
 * Las cuatro líneas del desglose: solo etiquetas; los montos vienen del server
 * (`apertura`, `ventasEnEfectivo`, `entradas`, `salidas`) y no se recalculan.
 */
internal data class CopyDesgloseAbierta(
    val titulo: String,
    val apertura: String,
    val ventasEfectivo: String,
    val entradas: String,
    val salidas: String,
)

internal fun copyDesgloseAbierta(feria: Boolean): CopyDesgloseAbierta =
    if (feria) {
        CopyDesgloseAbierta(
            titulo = "De dónde sale",
            apertura = "Con lo que abriste",
            // Mismo vocabulario que el vacío de movimientos («cobrás en billete»).
            ventasEfectivo = "Cobrado en billete",
            entradas = "Metiste a mano",
            salidas = "Sacaste a mano",
        )
    } else {
        CopyDesgloseAbierta(
            titulo = "De dónde sale",
            apertura = "Con lo que abriste",
            ventasEfectivo = "Vendido en efectivo",
            entradas = "Metiste a mano",
            salidas = "Sacaste a mano",
        )
    }

/**
 * La palabra del movimiento dice el signo, no el color: quien no distingue el
 * rojo del verde lee exactamente lo mismo.
 */
internal fun copyPalabraMovimiento(esRetiro: Boolean): String =
    if (esRetiro) "Sacaste" else "Metiste"
