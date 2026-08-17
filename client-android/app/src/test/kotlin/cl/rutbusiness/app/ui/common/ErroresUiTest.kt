package cl.rutbusiness.app.ui.common

import cl.rutbusiness.core.error.AppError
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Traducción de [AppError] al copy de UI: recado humano, sin stacktrace ni PC.
 */
class ErroresUiTest {

    @Test
    fun `ServidorNoResponde usa offline sin computador ni PC`() {
        val copy = AppError.ServidorNoResponde("http://10.0.2.2:8080").aCopy("el catálogo")
        assertEquals("No llegamos", copy.title)
        assertEquals(
            "No pudimos traer el catálogo. Revisa que el teléfono tenga wifi o datos " +
                "prendidos e intenta de nuevo.",
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

    @Test
    fun `ErrorDelServidor tecnico se lee como recado sin codigo HTTP`() {
        val copy = AppError.ErrorDelServidor(
            status = 503,
            code = null,
            serverMessage = null,
        ).aCopy("tus productos")
        assertEquals("No se pudo ahora", copy.title)
        assertTrue(copy.message.contains("No es culpa tuya"))
        assertEquals("Reintentar", copy.retryLabel)
        val texto = "${copy.title} ${copy.message}"
        assertFalse(texto.contains("503"))
        assertFalse(texto.contains("servidor", ignoreCase = true))
        assertFalse(texto.contains("error 503", ignoreCase = true))
        assertFalse(texto.contains("computador", ignoreCase = true))
        assertFalse(texto.contains("192.168"))
    }

    @Test
    fun `ErrorDelServidor con mensaje de dominio se respeta`() {
        val copy = AppError.ErrorDelServidor(
            status = 409,
            code = "STOCK",
            serverMessage = "No queda stock de Arroz Grado 1",
        ).aCopy("el cobro")
        assertEquals("No se pudo", copy.title)
        assertEquals("No queda stock de Arroz Grado 1", copy.message)
        assertEquals("Reintentar", copy.retryLabel)
    }

    @Test
    fun `Inesperado usa recado desconocido sin jerga`() {
        val copy = AppError.Inesperado("NullPointerException at line 12").aCopy("tus ventas")
        assertEquals("No salió", copy.title)
        assertTrue(copy.message.contains("tus ventas"))
        val texto = "${copy.title} ${copy.message}"
        assertFalse(texto.contains("NullPointer", ignoreCase = true))
        assertFalse(texto.contains("exception", ignoreCase = true))
        assertFalse(texto.contains("computador", ignoreCase = true))
    }

    @Test
    fun `SesionExpirada manda a entrar de nuevo`() {
        val copy = AppError.SesionExpirada().aCopy("lo que sea")
        assertEquals("Hay que entrar de nuevo", copy.title)
        assertNull(copy.retryLabel)
    }

    @Test
    fun `mensaje tecnico detecta codigo HTTP y fallback de servidor`() {
        assertTrue(mensajeDeServidorEsTecnico("El servidor tuvo un problema (error 500). Intenta de nuevo en un momento."))
        assertTrue(mensajeDeServidorEsTecnico("Algo falló: error 502"))
        assertTrue(mensajeDeServidorEsTecnico(""))
        assertFalse(mensajeDeServidorEsTecnico("No queda stock de Arroz Grado 1"))
    }
}
