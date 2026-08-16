package cl.rutbusiness.app.ui.assist

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate del título del agente (pack feria vs formal).
 *
 * Puro JVM: sin Compose ni ViewModel. Si el fallback sin nombre vuelve a
 * decir «negocio» en feria, este test lo atrapa. Farmacia queda intacta.
 */
class TituloAgenteTest {

    @Test
    fun `sin nombre feria pide al puesto`() {
        val copy = tituloAgente(nombre = null, feria = true)
        assertEquals("Pídele a tu puesto", copy)
        assertFalse(copy.lowercase().contains("negocio"))
        assertTrue(copy.lowercase().contains("puesto"))
    }

    @Test
    fun `sin nombre en blanco feria pide al puesto`() {
        assertEquals("Pídele a tu puesto", tituloAgente(nombre = "   ", feria = true))
        assertEquals("Pídele a tu puesto", tituloAgente(nombre = "", feria = true))
    }

    @Test
    fun `sin nombre farmacia sigue con negocio`() {
        val copy = tituloAgente(nombre = null, feria = false)
        assertEquals("Pídele a tu negocio", copy)
        assertFalse(copy.lowercase().contains("puesto"))
    }

    @Test
    fun `con nombre se usa el nombre en ambos modos`() {
        assertEquals("Pídele a Doña Rosa", tituloAgente(nombre = "Doña Rosa", feria = true))
        assertEquals("Pídele a Doña Rosa", tituloAgente(nombre = "Doña Rosa", feria = false))
        assertEquals(
            "Pídele a Mi farmacia",
            tituloAgente(nombre = "  Mi farmacia  ".trim(), feria = false),
        )
    }
}
