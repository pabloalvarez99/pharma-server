package cl.rutbusiness.ui.components

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Fallas genéricas se leen como recado: qué pasó y qué hacer, sin stacktrace.
 *
 * `rbErrorCopy` alimenta catálogo, Hoy y cualquier `AppError.aCopy` de red.
 * En feria/nube no hay "computador del negocio" ni IP en el texto visible.
 */
class RbErrorCopyTest {

    @Test
    fun `offline no nombra computador ni PC`() {
        val copy = rbErrorCopy(RbErrorKind.Offline, "tus productos")
        assertEquals("No llegamos", copy.title)
        assertEquals(
            "No pudimos traer tus productos. Revisa que el teléfono tenga wifi o datos " +
                "prendidos e intenta de nuevo.",
            copy.message,
        )
        assertEquals("Reintentar", copy.retryLabel)
        assertSinStacktrace(copy)
        assertFalse(copy.title.contains("negocio"))
    }

    @Test
    fun `offline default what sigue sin PC`() {
        val copy = rbErrorCopy(RbErrorKind.Offline)
        assertTrue(copy.message.contains("esta parte"))
        assertSinStacktrace(copy)
    }

    @Test
    fun `timeout se lee como recado corto`() {
        val copy = rbErrorCopy(RbErrorKind.Timeout, "tus ventas")
        assertEquals("Se demoró", copy.title)
        assertEquals(
            "No alcanzamos a traer tus ventas. Espera un ratito e intenta de nuevo.",
            copy.message,
        )
        assertEquals("Reintentar", copy.retryLabel)
        assertSinStacktrace(copy)
    }

    @Test
    fun `server fault no habla de sistema ni codigo`() {
        val copy = rbErrorCopy(RbErrorKind.ServerFault, "el catálogo")
        assertEquals("No se pudo ahora", copy.title)
        assertTrue(copy.message.contains("No es culpa tuya"))
        assertEquals("Reintentar", copy.retryLabel)
        assertSinStacktrace(copy)
        assertFalse(copy.message.contains("sistema", ignoreCase = true))
        assertFalse(copy.message.contains("servidor", ignoreCase = true))
    }

    @Test
    fun `unknown pide reabrir sin jerga`() {
        val copy = rbErrorCopy(RbErrorKind.Unknown, "tus clientes")
        assertEquals("No salió", copy.title)
        assertTrue(copy.message.contains("tus clientes"))
        assertTrue(copy.message.contains("cierra la app"))
        assertEquals("Reintentar", copy.retryLabel)
        assertSinStacktrace(copy)
    }

    @Test
    fun `forbidden sin reintento y sin PC`() {
        val copy = rbErrorCopy(RbErrorKind.Forbidden, "el informe")
        assertEquals("Esto no está para tu cuenta", copy.title)
        assertNull(copy.retryLabel)
        assertTrue(copy.message.contains("el informe"))
        assertSinStacktrace(copy)
    }

    @Test
    fun `unauthorized manda a la entrada sin reintento`() {
        val copy = rbErrorCopy(RbErrorKind.Unauthorized)
        assertEquals("Hay que entrar de nuevo", copy.title)
        assertNull(copy.retryLabel)
        assertTrue(copy.message.contains("correo"))
        assertSinStacktrace(copy)
    }

    private fun assertSinStacktrace(copy: RbErrorCopy) {
        val texto = "${copy.title} ${copy.message}"
        assertFalse(texto.contains("computador", ignoreCase = true))
        assertFalse(texto.contains("PC", ignoreCase = false))
        assertFalse(texto.contains("pc del", ignoreCase = true))
        assertFalse(texto.contains("192.168"))
        assertFalse(texto.contains("pharma-server", ignoreCase = true))
        assertFalse(Regex("""\berror\s+\d{3}\b""", RegexOption.IGNORE_CASE).containsMatchIn(texto))
        assertFalse(texto.contains("stack", ignoreCase = true))
        assertFalse(texto.contains("exception", ignoreCase = true))
    }
}
