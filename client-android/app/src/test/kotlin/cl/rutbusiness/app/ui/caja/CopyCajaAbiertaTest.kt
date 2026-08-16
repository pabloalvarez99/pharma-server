package cl.rutbusiness.app.ui.caja

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate del desglose / error del puesto abierto (ADR-0022).
 *
 * Puro JVM: si el feriante vuelve a ver «cuenta», «cajón», «arqueo» o
 * «sistema» en esta pantalla, el test rompe el build.
 */
class CopyCajaAbiertaTest {

    private fun assertSinJergaLedger(donde: String, texto: String) {
        val lower = texto.lowercase()
        assertFalse(
            "$donde no debe decir «cuenta» como ledger: $texto",
            lower.contains("cuenta"),
        )
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
            "$donde no debe decir «sistema»: $texto",
            lower.contains("sistema"),
        )
        assertFalse(
            "$donde no debe decir «transacción»: $texto",
            lower.contains("transacción") || lower.contains("transaccion"),
        )
    }

    @Test
    fun `error feria habla de plata del puesto sin cuenta ni cajon`() {
        val msg = copyErrorEsperadoAbierta(feria = true)
        assertTrue(msg.lowercase().contains("puesto"))
        assertTrue(msg.lowercase().contains("plata") || msg.lowercase().contains("debería"))
        assertTrue(msg.contains("Actualizar"))
        assertSinJergaLedger("error esperado feria", msg)
    }

    @Test
    fun `error retail puede seguir con cuenta de la caja`() {
        val msg = copyErrorEsperadoAbierta(feria = false)
        assertTrue(msg.lowercase().contains("caja"))
        assertTrue(msg.contains("Actualizar"))
        assertFalse(msg.lowercase().contains("puesto"))
    }

    @Test
    fun `desglose feria es cuaderno y billete sin jerga de cajero`() {
        val d = copyDesgloseAbierta(feria = true)
        assertEquals("De dónde sale", d.titulo)
        assertEquals("Con lo que abriste", d.apertura)
        assertTrue(
            "feria nombra billete o lo cobrado, no solo efectivo de mall",
            d.ventasEfectivo.lowercase().contains("billete") ||
                d.ventasEfectivo.lowercase().contains("cobrado"),
        )
        assertEquals("Metiste a mano", d.entradas)
        assertEquals("Sacaste a mano", d.salidas)
        for (s in listOf(d.titulo, d.apertura, d.ventasEfectivo, d.entradas, d.salidas)) {
            assertSinJergaLedger("desglose feria", s)
        }
    }

    @Test
    fun `desglose retail conserva vendido en efectivo`() {
        val d = copyDesgloseAbierta(feria = false)
        assertEquals("De dónde sale", d.titulo)
        assertEquals("Vendido en efectivo", d.ventasEfectivo)
        assertEquals("Con lo que abriste", d.apertura)
    }

    @Test
    fun `palabra del movimiento dice el signo`() {
        assertEquals("Sacaste", copyPalabraMovimiento(esRetiro = true))
        assertEquals("Metiste", copyPalabraMovimiento(esRetiro = false))
    }
}
