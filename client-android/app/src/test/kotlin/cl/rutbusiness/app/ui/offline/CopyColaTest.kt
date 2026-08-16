package cl.rutbusiness.app.ui.offline

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en la cola vacía (ADR-0022) + tono de recado offline.
 *
 * Puro JVM: el feriante no ve "sistema del negocio" ni "computador"; sin
 * conexión se dice como recado, no como error de red.
 */
class CopyColaTest {

    @Test
    fun `feria cola vacia no habla del sistema del negocio ni computador`() {
        val hint = hintColaVacia(feria = true)
        assertEquals("Todas las ventas que cobraste ya se anotaron.", hint)
        assertFalse(
            "feria no menciona sistema del negocio",
            hint.lowercase().contains("sistema del negocio"),
        )
        assertFalse(
            "feria no menciona computador",
            hint.lowercase().contains("computador"),
        )
    }

    @Test
    fun `farmacia cola vacia conserva el string exacto`() {
        assertEquals(
            "Todas las ventas que cobraste ya llegaron al sistema del negocio.",
            hintColaVacia(feria = false),
        )
    }

    @Test
    fun `sin conexion se dice como recado, no como error de red`() {
        val titulo = recadoSinConexion()
        val detalle = detalleSinConexion()
        assertEquals("Sin conexión", titulo)
        assertEquals("Ves lo último que se cargó.", detalle)

        val junto = "$titulo $detalle".lowercase()
        assertFalse(junto, junto.contains("error"))
        assertFalse(junto, junto.contains("falló") || junto.contains("fallo"))
        assertFalse(junto, junto.contains("problema"))
        assertFalse(junto, junto.contains("offline"))
        assertFalse(junto, junto.contains("computador"))
        assertFalse(junto, junto.contains("192.168"))
        assertTrue(junto, junto.contains("conexión") || junto.contains("conexion"))
    }
}
