package cl.rutbusiness.app.ui.caja

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate del picker multi-register y la nota al abrir (ADR-0022).
 *
 * Puro JVM: feria habla de mesa/puesto; si reaparece «caja» / «cajón» /
 * «sistema» en el picker feria, el test rompe el build.
 */
class CopyAbrirCajaExtraTest {

    private fun assertSinJergaCajaFormal(donde: String, texto: String) {
        val lower = texto.lowercase()
        assertFalse(
            "$donde no debe decir «caja»: $texto",
            // «caja» sola (caja registradora / cuál caja); no exige «cajón».
            Regex("""\bcaja\b""").containsMatchIn(lower),
        )
        assertFalse(
            "$donde no debe decir «cajón»: $texto",
            lower.contains("cajón") || lower.contains("cajon"),
        )
        assertFalse(
            "$donde no debe decir «sistema»: $texto",
            lower.contains("sistema"),
        )
        assertFalse(
            "$donde no debe decir «registradora»: $texto",
            lower.contains("registradora"),
        )
        assertFalse(
            "$donde no debe decir «sesión»: $texto",
            lower.contains("sesión") || lower.contains("sesion"),
        )
        assertFalse(
            "$donde no debe usar inglés de caja: $texto",
            lower.contains("session") || lower.contains("cash register") || lower.contains("drawer"),
        )
    }

    @Test
    fun `feria elige mesa o puesto sin caja ni cajon ni sistema`() {
        val c = copyElegirCajaApertura(feria = true)
        assertEquals("¿Cuál mesa?", c.tituloCard)
        assertTrue(
            "feria nombra mesa o puesto",
            c.ayuda.lowercase().contains("mesa") || c.ayuda.lowercase().contains("puesto"),
        )
        assertTrue(
            "feria suena a cuaderno",
            c.ayuda.lowercase().contains("cuaderno") || c.ayuda.lowercase().contains("anota"),
        )
        assertSinJergaCajaFormal("elegir feria título", c.tituloCard)
        assertSinJergaCajaFormal("elegir feria ayuda", c.ayuda)
    }

    @Test
    fun `retail sigue con cual caja y el local`() {
        val c = copyElegirCajaApertura(feria = false)
        assertEquals("¿Cuál caja?", c.tituloCard)
        assertTrue(c.ayuda.lowercase().contains("caja"))
        assertTrue(
            c.ayuda.lowercase().contains("negocio") || c.ayuda.lowercase().contains("local"),
        )
        assertFalse(c.tituloCard.lowercase().contains("mesa"))
        assertFalse(c.tituloCard.lowercase().contains("puesto"))
    }

    @Test
    fun `nota de apertura retail conserva copy de caja`() {
        val n = copyNotaApertura()
        assertEquals("¿Algo que anotar?", n.tituloCard)
        assertTrue(n.etiqueta.lowercase().contains("apertura"))
        assertTrue(n.ayuda.lowercase().contains("caja"))
        assertTrue(n.ayuda.lowercase().contains("cierre"))
        // No inventar montos nuevos: el ejemplo de anoche se queda.
        assertTrue(n.placeholder.contains("5.000") || n.placeholder.contains("anoche"))
    }
}
