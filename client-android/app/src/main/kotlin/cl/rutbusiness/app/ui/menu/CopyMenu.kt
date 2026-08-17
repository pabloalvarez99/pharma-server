package cl.rutbusiness.app.ui.menu

/**
 * Copy del menú del puesto según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose, mismo criterio que
 * `CopyResumen.kt`. Este menú no es un panel de ajustes de sistema: es la
 * puerta a funciones que ya existen y funcionan pero no tenían ningún camino
 * desde la interfaz (impresora antes de la primera venta, ventas que faltan
 * subir, contar la plata en feria, cambiar de rubro).
 *
 * Nunca en feria: sistema, cajón, arqueo, computador, stock, catálogo,
 * boleta. Nunca "respaldo" ni "cola de subida" en ningún rubro — son palabras
 * de desarrollador, no del puesto.
 */

// --- pantalla principal del menú --------------------------------------------

/** Título del top bar del menú. */
internal fun tituloMenu(feria: Boolean): String =
    if (feria) "Tu puesto" else "Más opciones"

/** Subtítulo del top bar del menú. Igual en los dos rubros: no es jerga. */
internal fun subtituloMenu(): String = "Todo lo que puedes hacer aquí"

// --- encabezados de grupo (GrupoDeMenu) --------------------------------------
// Mismo texto en los dos rubros: agrupa por ritmo de uso, no por vocabulario
// de negocio, así que no hace falta rama feria.

/** Encabezado del grupo de arriba: lo que se toca en medio de un día de puesto. */
internal fun tituloGrupoDiario(): String = "Lo que usas seguido"

/** Encabezado del grupo de abajo: lo que se toca una vez y se olvida por semanas. */
internal fun tituloGrupoMensual(): String = "Una vez al mes"

// --- entrada: contar la plata (caja) ----------------------------------------

/**
 * En feria no se dice "caja" ni "arqueo": se cuenta la plata del puesto.
 * Retail formal sigue con "la caja", que es como ya lo llama el resumen.
 */
internal fun tituloCaja(feria: Boolean): String =
    if (feria) "Contar la plata del puesto" else "La caja"

internal fun subtituloCaja(feria: Boolean): String =
    if (feria) {
        "Cuánto hay para empezar o cerrar el día"
    } else {
        "Abrir, cerrar y contar lo que hay"
    }

// --- entrada: la impresora ---------------------------------------------------
// Sólo se ofrece cuando el pack de rubro trae impresora (nunca en feria), así
// que no hace falta rama feria acá — quien la lee ya no está en feria.

internal fun tituloImpresora(): String = "La impresora"

internal fun subtituloImpresora(): String = "Emparejarla o probar que imprima"

// --- entrada: ventas que faltan subir ---------------------------------------
// Misma frase en los dos rubros: es sobre el teléfono, no sobre el negocio.
// "Falta subir" y no "respaldo"/"cola": eso último es vocabulario de
// desarrollador (Regla dura del encargo), esto es lo que le pasa a la venta.

internal fun tituloVentasPendientes(): String = "Ventas que faltan subir"

internal fun subtituloVentasPendientes(): String =
    "Se guardan en el teléfono y suben solas cuando vuelva la señal"

// --- entrada: cambiar de rubro ------------------------------------------------

internal fun tituloRubro(feria: Boolean): String =
    if (feria) "Cómo vendo" else "Tipo de negocio"

internal fun subtituloRubro(): String = "Cambia si tu negocio cambió"

/** Encabezado de "esto es lo que tienes ahora" en la pantalla de rubro. */
internal fun etiquetaRubroActual(nombreDelRubro: String): String = "Ahora: $nombreDelRubro"

internal fun tituloConfirmarRubro(): String = "¿Cambiar tu tipo de negocio?"

internal fun mensajeConfirmarRubro(nombreNuevo: String): String =
    "Vas a cambiar a \"$nombreNuevo\". Esto cambia lo que ves en la app — el agente, " +
        "el fiado, el escáner y la impresora — para que se parezca a tu día."

internal fun botonConfirmarRubro(): String = "Sí, cambiar"

internal fun guardandoRubro(): String = "Guardando..."

internal fun mensajeSinConexionRubro(): String =
    "No se pudo guardar sin conexión. Prueba de nuevo cuando tengas señal."
