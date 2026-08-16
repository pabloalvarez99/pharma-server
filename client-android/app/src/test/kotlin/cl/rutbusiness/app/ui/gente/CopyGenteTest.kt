package cl.rutbusiness.app.ui.gente

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Etiquetas del recado: se cuentan y se recuerdan, no se "exportan".
 */
class CopyGenteTest {

    @Test
    fun `el dia se cuenta, no se exporta un resumen`() {
        assertEquals("Contar cómo va el día", etiquetaCompartirDia(feria = true))
        assertEquals("Contarle cómo va el día", etiquetaCompartirDia(feria = false))

        val formal = etiquetaCompartirDia(false)
        assertFalse(formal, formal.contains("resumen", ignoreCase = true))
        assertFalse(formal, formal.contains("export", ignoreCase = true))
        assertFalse(formal, formal.contains("compartir", ignoreCase = true))
    }

    @Test
    fun `la deuda se manda o se recuerda, no se comparte el saldo`() {
        assertEquals("Mandarle lo que debe", etiquetaCompartirDeuda(feria = true))
        assertEquals("Recordarle lo que debe", etiquetaCompartirDeuda(feria = false))

        val formal = etiquetaCompartirDeuda(false)
        assertFalse(formal, formal.contains("saldo", ignoreCase = true))
        assertFalse(formal, formal.contains("compartir", ignoreCase = true))
    }

    @Test
    fun `la pista habla de chat, no de archivo ni export`() {
        val pista = pistaDelRecado()
        assertTrue(pista, pista.contains("chat", ignoreCase = true))
        assertFalse(pista, pista.contains("export", ignoreCase = true))
        assertFalse(pista, pista.contains("CSV", ignoreCase = true))
        assertFalse(pista, pista.contains("archivo", ignoreCase = true))
    }
}
