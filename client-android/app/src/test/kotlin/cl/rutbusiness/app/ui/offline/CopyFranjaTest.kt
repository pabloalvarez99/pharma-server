package cl.rutbusiness.app.ui.offline

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy de la franja offline: recado, no error de red; feria sin
 * "enviar" / gerundio inglés / computador.
 */
class CopyFranjaTest {

    @Test
    fun `feria en camino suena a cuaderno, no a Enviando`() {
        assertEquals("Va 1 en camino", tituloEnCamino(1, feria = true))
        assertEquals("Van 3 en camino", tituloEnCamino(3, feria = true))
        val junto = "${tituloEnCamino(2, true)} ${detalleFranja(true, 2, 0, true)}"
        assertFalse(junto, junto.lowercase().contains("enviando"))
        assertFalse(junto, junto.contains("..."))
        assertFalse(junto, junto.lowercase().contains("error"))
    }

    @Test
    fun `retail en camino deja el gerundio ingles afuera`() {
        assertEquals("1 venta en camino", tituloEnCamino(1, feria = false))
        assertEquals("2 ventas en camino", tituloEnCamino(2, feria = false))
        assertFalse(tituloEnCamino(1, false).lowercase().contains("enviando"))
    }

    @Test
    fun `feria rechazo se anota en el puesto`() {
        assertEquals(
            "1 venta no se anotó en el puesto",
            tituloRechazadas(1, feria = true),
        )
        assertEquals(
            "2 ventas no se anotaron en el puesto",
            tituloRechazadas(2, feria = true),
        )
        val t = tituloRechazadas(1, true).lowercase()
        assertFalse(t, t.contains("enviar"))
        assertFalse(t, t.contains("sistema"))
        assertFalse(t, t.contains("computador"))
        assertTrue(t, t.contains("puesto"))
    }

    @Test
    fun `retail rechazo conserva enviar`() {
        assertEquals("1 venta no se pudo enviar", tituloRechazadas(1, feria = false))
        assertEquals("3 ventas no se pudieron enviar", tituloRechazadas(3, feria = false))
    }

    @Test
    fun `sin senal y cola el detalle promete la senal y es corto`() {
        assertEquals(
            "Sale sola al volver la señal",
            detalleFranja(conectado = false, esperando = 1, rechazadas = 0, feria = true),
        )
        assertEquals(
            "Salen solas al volver la señal",
            detalleFranja(conectado = false, esperando = 2, rechazadas = 0, feria = false),
        )
        val d = detalleFranja(false, 2, 0, true)
        assertTrue(d.length < 40)
        assertFalse(d.lowercase().contains("error"))
        assertFalse(d.lowercase().contains("offline"))
    }

    @Test
    fun `titulo sin senal reusa el recado compartido`() {
        assertEquals(
            "Sin conexión · 1 venta esperando",
            tituloFranja(conectado = false, esperando = 1, rechazadas = 0, feria = true),
        )
        assertEquals(
            "Sin conexión",
            tituloFranja(conectado = false, esperando = 0, rechazadas = 0, feria = true),
        )
        assertEquals(
            detalleSinConexion(),
            detalleFranja(conectado = false, esperando = 0, rechazadas = 0, feria = true),
        )
    }

    @Test
    fun `feria invita con voseo al tocar`() {
        assertEquals(
            "Tocá para ver cuáles.",
            detalleFranja(conectado = true, esperando = 0, rechazadas = 1, feria = true),
        )
        assertEquals(
            "Tocá para verlas.",
            detalleFranja(conectado = true, esperando = 1, rechazadas = 0, feria = true),
        )
    }

    @Test
    fun `singular de ventas se lee como persona`() {
        assertEquals("1 venta", ventas(1))
        assertEquals("4 ventas", ventas(4))
    }
}
