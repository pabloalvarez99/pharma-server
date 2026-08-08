package cl.rutbusiness.app.ui.cobrar

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy + escáner por pack (ADR-0022).
 *
 * Puro JVM: no monta Compose ni cámara. Si alguien reescribe las etiquetas
 * inline en [PasoBuscar] y se olvida de feria, este test lo atrapa.
 */
class CopyCobrarFeriaTest {

    @Test
    fun `feria sin barcode habla de nombre no de EAN`() {
        val c = copyBuscarCobrar(barcode = false)
        assertEquals("¿Qué vendiste?", c.etiqueta)
        assertTrue(c.placeholder.lowercase().contains("tomate") ||
            c.placeholder.lowercase().contains("cilantro"))
        assertFalse(c.ayudaOnline.contains("código de barras", ignoreCase = true))
        assertTrue(c.ayudaOnline.contains("nombre", ignoreCase = true))
    }

    @Test
    fun `retail con barcode menciona codigo de barras`() {
        val c = copyBuscarCobrar(barcode = true)
        assertEquals("Buscar producto", c.etiqueta)
        assertTrue(c.placeholder.contains("código de barras", ignoreCase = true))
        assertTrue(c.ayudaOnline.contains("código de barras", ignoreCase = true))
    }

    @Test
    fun `escaner solo si pack y hardware`() {
        assertFalse(escanerVisible(barcode = false, hayCamara = true))
        assertFalse(escanerVisible(barcode = true, hayCamara = false))
        assertFalse(escanerVisible(barcode = false, hayCamara = false))
        assertTrue(escanerVisible(barcode = true, hayCamara = true))
    }
}
