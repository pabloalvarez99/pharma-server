package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.rubro.RubroPack

/**
 * Chips de bienvenida del Agente (ADR-0022).
 *
 * Frases **completas y tocables** que el parser de `crates/assist` reconoce.
 * Extraídas de la UI para tests sin montar Compose: si alguien "mejora" el
 * copy y rompe kg/fiado/hoy, el test de feria lo atrapa.
 *
 * No inventar sintaxis: cada chip es una frase que day-1 ya parsea
 * (`crates/assist` / feria_chips). Venta suelta, fiado con nombre, total del
 * día y "quién me debe" bastan para enseñar el lenguaje del puesto.
 */
object AssistSugerencias {

    val default: List<String> = listOf(
        "¿Cuánto vendí hoy?",
        "¿Quién me debe plata?",
        "¿Qué se está por acabar?",
        "Registra un gasto de 5000 en arriendo",
        "¿Cuánto vendí este mes?",
    )

    /**
     * Day-1 feria: venta por kg con precio dicho, fiado con nombre, total del día.
     * ` a 2000` / ` a 1500` alimentan `precio_dicho` (primera venta sin SKU).
     * kg/atado/granel se limpian en `clean_venta_product` del server.
     * Precio va **antes** de «fiado a …» para que el parse no se coma la cola.
     */
    val feria: List<String> = listOf(
        "Vendí 2 kg de tomates a 2000",
        "Anota 2 kg de tomates a 2000 fiado a Don Juan",
        "¿Cuánto vendí hoy?",
        "Anota 1 atado de cilantro a 1500 fiado a doña Ana",
        "¿Quién me debe plata?",
    )

    fun para(pack: RubroPack): List<String> =
        if (pack.features.agentHome || pack.rubro == "feria") feria else default

    fun intro(pack: RubroPack): String =
        if (pack.features.agentHome || pack.rubro == "feria") {
            // Hablado, de puesto: sin "asistente virtual" ni jerga de app.
            "Hablame como en el puesto — vendí, fié, ¿cuánto saqué hoy? " +
                "Antes de anotar nada, te lo digo de vuelta para que lo revises."
        } else {
            "Escríbeme como le hablarías a un empleado de confianza. " +
                "Antes de guardar nada, te lo muestro para que lo revises."
        }
}
