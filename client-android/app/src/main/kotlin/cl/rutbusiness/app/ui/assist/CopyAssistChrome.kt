package cl.rutbusiness.app.ui.assist

/**
 * Copy del chrome de la charla con el agente (bienvenida + redactor).
 *
 * Extraído de la UI para tests JVM sin Compose: feria suena a puesto
 * (Dime / Decirle / anotando); retail se queda cerca del copy actual.
 * Los chips de sugerencia viven en [AssistSugerencias] — no reescribirlos acá.
 */
internal data class CopyAssistChrome(
    val tituloBienvenida: String,
    val etiquetaChips: String,
    val etiquetaCampo: String,
    val placeholderSinVoz: String,
    val placeholderConVoz: String,
    val pensando: String,
    val enviar: String,
    val reintentar: String,
)

internal fun copyAssistChrome(feria: Boolean): CopyAssistChrome =
    if (feria) {
        CopyAssistChrome(
            tituloBienvenida = "Dime qué pasó",
            etiquetaChips = "Toca una y arrancamos",
            etiquetaCampo = "Vendí, fié, ¿cuánto saqué?",
            placeholderSinVoz = "Escribe o toca una de arriba",
            placeholderConVoz = "Habla o escribe",
            pensando = "Estoy anotando…",
            enviar = "Decirle",
            reintentar = "Decirle de nuevo",
        )
    } else {
        CopyAssistChrome(
            tituloBienvenida = "Pídeme lo que necesites",
            etiquetaChips = "Toca una para empezar",
            etiquetaCampo = "¿Qué necesitas?",
            placeholderSinVoz = "Escribe o toca una sugerencia",
            placeholderConVoz = "Habla o escribe",
            pensando = "Estoy revisando tus datos…",
            enviar = "Enviar",
            reintentar = "Volver a preguntar",
        )
    }
