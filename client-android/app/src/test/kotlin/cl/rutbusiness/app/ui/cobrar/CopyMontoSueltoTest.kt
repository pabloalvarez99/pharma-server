package cl.rutbusiness.app.ui.cobrar

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy de venta suelta (cobro por monto sin catálogo).
 *
 * Puro JVM: si alguien reescribe las etiquetas inline en [PasoMontoSuelto]
 * y se olvida de feria, este test lo atrapa.
 */
class CopyMontoSueltoTest {

    @Test
    fun `feria pregunta como en la mesa`() {
        assertEquals("¿Cuánto es?", copyLabelMontoSuelto(feria = true))
        assertFalse(copyLabelMontoSuelto(feria = true).contains("cobras", ignoreCase = true))
    }

    @Test
    fun `retail conserva el copy formal del monto`() {
        assertEquals("¿Cuánto le cobras?", copyLabelMontoSuelto(feria = false))
    }

    @Test
    fun `ayuda feria no habla de cargado de catalogo formal`() {
        val feria = copyAyudaMontoSuelto(feria = true, simbolo = "$")
        assertTrue(feria.contains("$"))
        assertTrue(feria.contains("monto", ignoreCase = true))
        assertFalse(feria.contains("nada cargado", ignoreCase = true))

        val retail = copyAyudaMontoSuelto(feria = false, simbolo = "$")
        assertTrue(retail.contains("nada cargado", ignoreCase = true))
        assertTrue(retail.startsWith("En $"))
    }

    @Test
    fun `cta feria es corta en mesa`() {
        assertEquals("Cobrar", copyCtaMontoSuelto(feria = true, preparando = false))
        assertEquals("Anotando…", copyCtaMontoSuelto(feria = true, preparando = true))
    }

    @Test
    fun `cta retail sigue explicita`() {
        assertEquals("Cobrar este monto", copyCtaMontoSuelto(feria = false, preparando = false))
        assertEquals("Preparando…", copyCtaMontoSuelto(feria = false, preparando = true))
    }

    @Test
    fun `volver es el mismo en ambos packs`() {
        assertEquals("Volver", copyVolverMontoSuelto())
    }

    @Test
    fun `nota del pie sigue en CopyCobrarFeria y habla de puesto`() {
        // Contrato: el pie no se reescribe acá; se reusa copyNotaMontoSuelto.
        val feria = copyNotaMontoSuelto(feria = true)
        assertTrue(feria.contains("puesto", ignoreCase = true))
        assertTrue(feria.contains("Venta suelta"))
    }
}
