package cl.rutbusiness.app.ui.impresora

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Impresora (ADR-0022).
 *
 * Feria no tiene impresora ni computador de día 1: SinBluetooth no puede mandar
 * a reimprimir en un PC ni hablar de boleta. Las fallas de papel hablan del
 * puesto (ticket / rollo), no de boleta formal. Farmacia conserva esa salida.
 */
class CopyImpresoraTest {

    @Test
    fun `feria SinBluetooth no manda a computador ni boleta ni IP`() {
        val texto = FallaDeImpresion.SinBluetooth(feria = true).queHacer
        val lower = texto.lowercase()
        assertFalse("feria no menciona computador", lower.contains("computador"))
        assertFalse("feria no menciona boleta", lower.contains("boleta"))
        assertFalse("feria no filtra IP local", lower.contains("192.168"))
        assertTrue(texto.contains("anotada"))
        assertTrue(texto.contains("teléfono") || texto.contains("telefono"))
    }

    @Test
    fun `farmacia SinBluetooth sigue ofreciendo el computador del negocio`() {
        val texto = FallaDeImpresion.SinBluetooth(feria = false).queHacer
        assertTrue(texto.lowercase().contains("computador"))
        assertTrue(texto.lowercase().contains("boleta"))
    }

    @Test
    fun `helper feria y farmacia coinciden con la sealed class`() {
        assertTrue(
            copyQueHacerSinBluetooth(feria = true) ==
                FallaDeImpresion.SinBluetooth(feria = true).queHacer,
        )
        assertTrue(
            copyQueHacerSinBluetooth(feria = false) ==
                FallaDeImpresion.SinBluetooth(feria = false).queHacer,
        )
    }

    @Test
    fun `feria SinDatosDeLaBoleta habla de papel del puesto y no de boleta`() {
        val falla = FallaDeImpresion.SinDatosDeLaBoleta(feria = true)
        assertEquals("No pudimos traer el papel del puesto", falla.titulo)
        assertFalse(falla.titulo.lowercase().contains("boleta"))
        assertFalse(falla.queHacer.lowercase().contains("boleta"))
        assertFalse(falla.queHacer.lowercase().contains("sistema"))
        assertFalse(falla.queHacer.lowercase().contains("driver"))
        assertTrue(falla.queHacer.contains("anotada"))
        assertTrue(falla.queHacer.contains("Reintentar"))
        assertEquals(copyTituloSinDatosDeLaBoleta(true), falla.titulo)
        assertEquals(copyQueHacerSinDatosDeLaBoleta(true), falla.queHacer)
    }

    @Test
    fun `farmacia SinDatosDeLaBoleta conserva el titulo de boleta`() {
        val falla = FallaDeImpresion.SinDatosDeLaBoleta(feria = false)
        assertEquals("No pudimos traer la boleta", falla.titulo)
        assertTrue(falla.queHacer.contains("guardada"))
        assertEquals(copyTituloSinDatosDeLaBoleta(false), falla.titulo)
        assertEquals(copyQueHacerSinDatosDeLaBoleta(false), falla.queHacer)
    }

    @Test
    fun `feria SeCortoAMitad habla de ticket del puesto y no de boleta`() {
        val falla = FallaDeImpresion.SeCortoAMitad("Rollo", feria = true)
        assertEquals("Se cortó el papel a medias", falla.titulo)
        val lower = falla.queHacer.lowercase()
        assertFalse("feria no dice boleta", lower.contains("boleta"))
        assertFalse(lower.contains("sistema"))
        assertFalse(lower.contains("driver"))
        assertFalse(lower.contains("mac"))
        assertTrue(falla.queHacer.contains("ticket del puesto") || falla.queHacer.contains("rollo"))
        assertTrue(falla.queHacer.contains("Reintentar"))
        assertEquals(copyQueHacerSeCortoAMitad(true), falla.queHacer)
    }

    @Test
    fun `farmacia SeCortoAMitad sigue nombrando la boleta`() {
        val falla = FallaDeImpresion.SeCortoAMitad("Epson", feria = false)
        assertTrue(falla.queHacer.lowercase().contains("boleta"))
        assertEquals(copyQueHacerSeCortoAMitad(false), falla.queHacer)
    }
}
