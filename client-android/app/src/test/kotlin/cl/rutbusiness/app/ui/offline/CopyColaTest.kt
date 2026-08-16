package cl.rutbusiness.app.ui.offline

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Gate de copy feria en la cola vacía (ADR-0022).
 *
 * Puro JVM: el feriante no ve "sistema del negocio" ni "computador".
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
}
