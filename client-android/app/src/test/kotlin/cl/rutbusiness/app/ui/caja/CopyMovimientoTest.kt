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

    // --- motivosDeUnToque: chips de un toque ---------------------------------

    @Test
    fun `feria retiro ofrece las razones reales de sacar plata en un toque`() {
        val chips = motivosDeUnToque(feria = true, esRetiro = true)
        assertEquals(
            listOf("Almuerzo", "Le pagué la mercadería", "Movilización", "Plata para la casa"),
            chips,
        )
        chips.forEach { assertSinJergaCajaFormal("chip feria retiro", it) }
    }

    @Test
    fun `feria ingreso ofrece las razones reales de meter plata en un toque`() {
        val chips = motivosDeUnToque(feria = true, esRetiro = false)
        assertEquals(listOf("Vuelto de la casa", "Plata para dar cambio"), chips)
        chips.forEach { assertSinJergaCajaFormal("chip feria ingreso", it) }
    }

    @Test
    fun `retail no ofrece chips, el motivo se escribe entero`() {
        assertTrue(motivosDeUnToque(feria = false, esRetiro = true).isEmpty())
        assertTrue(motivosDeUnToque(feria = false, esRetiro = false).isEmpty())
    }

    @Test
    fun `los chips de feria caben sin scroll y no usan jerga contable`() {
        val todos = motivosDeUnToque(feria = true, esRetiro = true) +
            motivosDeUnToque(feria = true, esRetiro = false)

        assertTrue(todos.isNotEmpty())
        todos.forEach { chip ->
            // Letra grande, sin scroll: nada de frases largas disfrazadas de chip.
            assertTrue("«$chip» es muy largo para un chip", chip.length <= 24)
            assertFalse(
                "«$chip» no debe decir «egreso» / «ingreso de caja» / «concepto»",
                chip.lowercase().let {
                    it.contains("egreso") || it.contains("ingreso de caja") || it.contains("concepto")
                },
            )
        }
    }
}
