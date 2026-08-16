package cl.rutbusiness.app.ui.common

import cl.rutbusiness.core.error.AppError
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Traducción de [AppError] al copy de UI: offline genérico sin PC del negocio.
 */
class ErroresUiTest {

    @Test
    fun `ServidorNoResponde usa offline sin computador ni PC`() {
        val copy = AppError.ServidorNoResponde("http://10.0.2.2:8080").aCopy("el catálogo")
        assertEquals("No llegamos", copy.title)
        assertEquals(
            "No pudimos traer el catálogo. Revisá que el teléfono tenga wifi o datos " +
                "prendidos e intentá de nuevo.",
            copy.message,
        )
        assertEquals("Reintentar", copy.retryLabel)
        val texto = "${copy.title} ${copy.message}"
        assertFalse(texto.contains("computador", ignoreCase = true))
        assertFalse(texto.contains("PC"))
        assertFalse(texto.contains("10.0.2.2"))
    }

    @Test
    fun `DireccionInvalida conserva el ejemplo LAN`() {
        val copy = AppError.DireccionInvalida().aCopy("lo que sea")
        assertEquals("La dirección no se entiende", copy.title)
        assertTrue(copy.message.contains("192.168"))
        assertNull(copy.retryLabel)
    }
}
