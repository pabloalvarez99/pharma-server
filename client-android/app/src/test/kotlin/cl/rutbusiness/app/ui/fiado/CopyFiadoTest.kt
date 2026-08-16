package cl.rutbusiness.app.ui.fiado

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Fiado (ADR-0022).
 *
 * Puro JVM: si alguien reintroduce «computador» o «cajón» en el camino feria,
 * este test lo atrapa. Formal queda intacto.
 */
class CopyFiadoTest {

    @Test
    fun `feria sin cuenta pide senal no computador`() {
        val copy = errorSinCuenta(feria = true)
        assertTrue(copy.contains("señal") || copy.contains("senal") || copy.contains("revisá la señal"))
        assertFalse(
            "modoFeria no exige computador",
            copy.lowercase().contains("computador"),
        )
        assertTrue(copy.contains("intentá de nuevo") || copy.contains("intenta de nuevo"))
    }

    @Test
    fun `farmacia sin cuenta sigue nombrando el computador del negocio`() {
        val copy = errorSinCuenta(feria = false)
        assertTrue(copy.lowercase().contains("computador"))
        assertFalse(copy.contains("señal"))
    }

    @Test
    fun `feria efectivo dice que la plata cuenta en el dia y no cajon`() {
        val copy = ayudaComoPaga(feria = true, entraALaCaja = true)
        assertEquals("Esta plata cuenta en el día.", copy)
        assertFalse(copy.lowercase().contains("cajón"))
        assertFalse(copy.lowercase().contains("computador"))
    }

    @Test
    fun `feria transferencia habla de puesto o plata del dia sin cajon`() {
        val copy = ayudaComoPaga(feria = true, entraALaCaja = false)
        assertFalse(copy.lowercase().contains("cajón"))
        assertTrue(
            copy.lowercase().contains("día") || copy.lowercase().contains("puesto"),
        )
    }

    @Test
    fun `retail efectivo y transferencia mantienen cajon o caja`() {
        val efectivo = ayudaComoPaga(feria = false, entraALaCaja = true)
        assertTrue(efectivo.lowercase().contains("caja") || efectivo.lowercase().contains("cierre"))

        val transferencia = ayudaComoPaga(feria = false, entraALaCaja = false)
        assertTrue(transferencia.lowercase().contains("cajón"))
    }

    @Test
    fun `remate de abono feria no habla de cajon ni computador`() {
        val feria = remateAbonoEfectivo(feria = true)
        assertTrue(feria.lowercase().contains("día") || feria.lowercase().contains("dia"))
        assertFalse(feria.lowercase().contains("cajón"))
        assertFalse(feria.lowercase().contains("computador"))

        val formal = remateAbonoEfectivo(feria = false)
        assertTrue(formal.lowercase().contains("caja") || formal.lowercase().contains("cierre"))
    }
}
