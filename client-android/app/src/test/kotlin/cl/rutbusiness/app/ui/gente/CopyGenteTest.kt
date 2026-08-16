package cl.rutbusiness.app.ui.gente

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Etiquetas del recado: se mandan por chat o se recuerdan, no se "exportan".
 */
class CopyGenteTest {

    @Test
    fun `el dia se manda por chat, no se exporta un resumen`() {
        assertEquals("Mandar por chat", etiquetaCompartirDia(feria = true))
        assertEquals("Mandar por chat", etiquetaCompartirDia(feria = false))

        val formal = etiquetaCompartirDia(false)
        assertFalse(formal, formal.contains("resumen", ignoreCase = true))
        assertFalse(formal, formal.contains("export", ignoreCase = true))
        assertFalse(formal, formal.contains("compartir", ignoreCase = true))
        assertFalse(formal, formal.contains("Share", ignoreCase = false))
        assertTrue(formal, formal.contains("chat", ignoreCase = true))
    }

    @Test
    fun `la deuda se recuerda con nombre o se manda por chat`() {
        assertEquals("Mandar por chat", etiquetaCompartirDeuda(feria = true))
        assertEquals("Mandar por chat", etiquetaCompartirDeuda(feria = false))
        assertEquals(
            "Recordarle a Don Juan",
            etiquetaCompartirDeuda(feria = true, nombre = "Don Juan"),
        )
        assertEquals(
            "Recordarle a Rosa",
            etiquetaCompartirDeuda(feria = false, nombre = "  Rosa  "),
        )

        val formal = etiquetaCompartirDeuda(false)
        assertFalse(formal, formal.contains("saldo", ignoreCase = true))
        assertFalse(formal, formal.contains("compartir", ignoreCase = true))
        assertFalse(formal, formal.contains("Share"))
    }

    @Test
    fun `la pista habla de chat, no de archivo ni export`() {
        val pista = pistaDelRecado()
        assertTrue(pista, pista.contains("chat", ignoreCase = true))
        assertFalse(pista, pista.contains("export", ignoreCase = true))
        assertFalse(pista, pista.contains("CSV", ignoreCase = true))
        assertFalse(pista, pista.contains("archivo", ignoreCase = true))
        assertFalse(pista, pista.contains("compartir", ignoreCase = true))
    }
}
