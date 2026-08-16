package cl.rutbusiness.ui.components

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Fallas genéricas de red no mandan a prender un PC del negocio.
 *
 * `rbErrorCopy(Offline)` alimenta catálogo, Hoy y cualquier `AppError.aCopy`
 * de red. En feria/nube no hay "computador del negocio".
 */
class RbErrorCopyTest {

    @Test
    fun `offline no nombra computador ni PC`() {
        val copy = rbErrorCopy(RbErrorKind.Offline, "tus productos")
        assertEquals("No llegamos", copy.title)
        assertEquals(
            "No pudimos traer tus productos. Revisá que el teléfono tenga wifi o datos " +
                "prendidos e intentá de nuevo.",
            copy.message,
        )
        assertEquals("Reintentar", copy.retryLabel)
        val texto = "${copy.title} ${copy.message}"
        assertFalse(texto.contains("computador", ignoreCase = true))
        assertFalse(texto.contains("PC", ignoreCase = false))
        assertFalse(texto.contains("pc del", ignoreCase = true))
        assertFalse(copy.title.contains("negocio"))
    }

    @Test
    fun `offline default what sigue sin PC`() {
        val copy = rbErrorCopy(RbErrorKind.Offline)
        assertTrue(copy.message.contains("esta parte"))
        assertFalse(copy.message.contains("computador", ignoreCase = true))
        assertFalse(copy.message.contains("PC"))
    }
}
