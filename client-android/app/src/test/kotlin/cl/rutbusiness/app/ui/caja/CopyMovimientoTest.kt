package cl.rutbusiness.app.ui.caja

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy del motivo al sacar/meter plata (cuaderno feria vs caja retail).
 */
class CopyMovimientoTest {

    private fun assertSinJergaCajaFormal(donde: String, texto: String) {
        val lower = texto.lowercase()
        assertFalse(
            "$donde no debe decir «cajón»: $texto",
            lower.contains("cajón") || lower.contains("cajon"),
        )
        assertFalse(
            "$donde no debe decir «arqueo»: $texto",
            lower.contains("arqueo"),
        )
        assertFalse(
            "$donde no debe decir «sesión»: $texto",
            lower.contains("sesión") || lower.contains("sesion"),
        )
        assertFalse(
            "$donde no debe decir «sistema» / «transacción»: $texto",
            lower.contains("sistema") || lower.contains("transacción") || lower.contains("transaccion"),
        )
        assertFalse(
            "$donde no debe usar inglés de caja: $texto",
            lower.contains("session") || lower.contains("cash") || lower.contains("drawer"),
        )
    }

    @Test
    fun `feria saca plata con motivo de cuaderno sin jerga de cajon`() {
        val m = copyMotivoMovimiento(feria = true, esRetiro = true)
        assertTrue(m.tituloCard.lowercase().contains("sacás") || m.tituloCard.lowercase().contains("sacas"))
        assertEquals("En una línea", m.etiqueta)
        assertEquals("Le pagué al del pan", m.placeholder)
        assertTrue(m.ayuda.lowercase().contains("cuaderno"))
        assertSinJergaCajaFormal("titulo feria retiro", m.tituloCard)
        assertSinJergaCajaFormal("etiqueta feria retiro", m.etiqueta)
        assertSinJergaCajaFormal("ayuda feria retiro", m.ayuda)
    }

    @Test
    fun `feria mete plata con motivo de cuaderno sin jerga de cajon`() {
        val m = copyMotivoMovimiento(feria = true, esRetiro = false)
        assertTrue(m.tituloCard.lowercase().contains("dónde") || m.tituloCard.lowercase().contains("donde"))
        assertEquals("En una línea", m.etiqueta)
        assertEquals("Traje cambio de mi casa", m.placeholder)
        assertTrue(m.ayuda.lowercase().contains("cuaderno"))
        assertSinJergaCajaFormal("titulo feria ingreso", m.tituloCard)
        assertSinJergaCajaFormal("ayuda feria ingreso", m.ayuda)
    }

    @Test
    fun `retail mantiene tono de caja y motivo formal`() {
        val sacar = copyMotivoMovimiento(feria = false, esRetiro = true)
        assertEquals("¿Por qué?", sacar.tituloCard)
        assertEquals("Motivo", sacar.etiqueta)
        assertEquals("Le pagué al del pan", sacar.placeholder)
        assertTrue(sacar.ayuda.lowercase().contains("cierre"))

        val meter = copyMotivoMovimiento(feria = false, esRetiro = false)
        assertEquals("Traje cambio de mi casa", meter.placeholder)
        assertEquals("Motivo", meter.etiqueta)
    }
}
