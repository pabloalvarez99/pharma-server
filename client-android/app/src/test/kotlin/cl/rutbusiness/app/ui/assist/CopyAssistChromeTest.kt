package cl.rutbusiness.app.ui.assist

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de chrome de charla feria: suena a puesto, no a chatbot SaaS.
 */
class CopyAssistChromeTest {

    private fun assertSinJergaChatbot(donde: String, texto: String) {
        val lower = texto.lowercase()
        for (palabra in listOf(
            "sistema",
            "asistente virtual",
            "asistente",
            "transacción",
            "transaccion",
            "dashboard",
            "chatbot",
            "ia ",
            "bot ",
        )) {
            assertFalse(
                "$donde no debe sonar a chatbot («$palabra»): $texto",
                lower.contains(palabra),
            )
        }
    }

    @Test
    fun `feria habla de puesto con Dime Decirle y anotando`() {
        val c = copyAssistChrome(feria = true)
        assertEquals("Dime qué pasó", c.tituloBienvenida)
        assertEquals("Decirle", c.enviar)
        assertEquals("Estoy anotando…", c.pensando)
        assertEquals("Vendí, fié, ¿cuánto saqué?", c.etiquetaCampo)
        assertEquals("Decirle de nuevo", c.reintentar)
        assertTrue(
            "chips feria invitan a tocar: ${c.etiquetaChips}",
            c.etiquetaChips.contains("Toca", ignoreCase = true),
        )
        val todo = listOf(
            c.tituloBienvenida,
            c.etiquetaChips,
            c.etiquetaCampo,
            c.placeholderSinVoz,
            c.placeholderConVoz,
            c.pensando,
            c.enviar,
            c.reintentar,
        ).joinToString(" | ")
        assertSinJergaChatbot("chrome feria", todo)
        assertFalse(
            "feria no habla de «datos» del sistema: $todo",
            todo.lowercase().contains("datos"),
        )
    }

    @Test
    fun `retail conserva el chrome actual`() {
        val c = copyAssistChrome(feria = false)
        assertEquals("Pídeme lo que necesites", c.tituloBienvenida)
        assertEquals("Toca una para empezar", c.etiquetaChips)
        assertEquals("¿Qué necesitas?", c.etiquetaCampo)
        assertEquals("Escribe o toca una sugerencia", c.placeholderSinVoz)
        assertEquals("Habla o escribe", c.placeholderConVoz)
        assertEquals("Estoy revisando tus datos…", c.pensando)
        assertEquals("Enviar", c.enviar)
        assertEquals("Volver a preguntar", c.reintentar)
    }
}
