package cl.rutbusiness.app.ui.resumen

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Resumen (ADR-0022).
 *
 * Puro JVM: el feriante dice «venta» y «puesto», no «boleta» ni «computador».
 */
class CopyResumenTest {

    @Test
    fun `feria cuenta ventas en la tarjeta`() {
        assertEquals("1 venta.", copyTarjetaConteo(feria = true, conteo = 1L))
        assertEquals("42 ventas.", copyTarjetaConteo(feria = true, conteo = 42L))
        assertFalse(
            "feria no dice boleta",
            copyTarjetaConteo(feria = true, conteo = 3L).lowercase().contains("boleta"),
        )
    }

    @Test
    fun `farmacia sigue contando boletas`() {
        assertEquals("1 boleta.", copyTarjetaConteo(feria = false, conteo = 1L))
        assertEquals("42 boletas.", copyTarjetaConteo(feria = false, conteo = 42L))
        assertEquals(
            "Todavía no hay ninguna venta.",
            copyTarjetaConteo(feria = false, conteo = 0L),
        )
    }

    @Test
    fun `feria resume el dia en ventas`() {
        assertEquals(
            "$1.200 en 1 venta",
            copyResumenDelDia(feria = true, monto = "$1.200", conteo = 1L),
        )
        assertEquals(
            "$5.000 en 7 ventas",
            copyResumenDelDia(feria = true, monto = "$5.000", conteo = 7L),
        )
        assertFalse(
            copyResumenDelDia(feria = true, monto = "$1", conteo = 2L)
                .lowercase()
                .contains("boleta"),
        )
    }

    @Test
    fun `farmacia resume el dia en boletas`() {
        assertEquals(
            "$1.200 en 1 boleta",
            copyResumenDelDia(feria = false, monto = "$1.200", conteo = 1L),
        )
        assertEquals(
            "$5.000 en 7 boletas",
            copyResumenDelDia(feria = false, monto = "$5.000", conteo = 7L),
        )
    }

    @Test
    fun `feria explica la caja sin computador`() {
        val t = copyEnCajaExplicacion(feria = true, nombreDeCaja = null)
        assertTrue(t.lowercase().contains("puesto"))
        assertFalse(
            "feria no menciona computador",
            t.lowercase().contains("computador"),
        )
        assertTrue(t.contains("anotaste hoy") || t.contains("anotó"))
    }

    @Test
    fun `farmacia puede seguir hablando del computador del negocio`() {
        val t = copyEnCajaExplicacion(feria = false, nombreDeCaja = "Caja 1")
        assertTrue(t.contains("«Caja 1»"))
        assertTrue(t.lowercase().contains("computador"))
    }
}
