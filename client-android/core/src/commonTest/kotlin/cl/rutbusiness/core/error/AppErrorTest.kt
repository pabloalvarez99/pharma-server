package cl.rutbusiness.core.error

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/**
 * Copy genérico de red: no manda a "prender el PC" ni filtra hosts al usuario.
 *
 * Catálogo / Hoy / fallbacks reusan [AppError.ServidorNoResponde.userMessage] o
 * el [cl.rutbusiness.ui.components.rbErrorCopy] Offline; un leak de "PC del
 * negocio" o de `10.0.2.2` en el texto visible confunde en nube y feria.
 */
class AppErrorTest {

    @Test
    fun `ServidorNoResponde no nombra PC ni host en el mensaje visible`() {
        val err = AppError.ServidorNoResponde("http://10.0.2.2:8080", technical = "timeout")
        assertEquals(
            "No pudimos conectar. Revisá que el teléfono tenga internet e intentá de nuevo.",
            err.userMessage,
        )
        assertFalse(err.userMessage.contains("PC", ignoreCase = true))
        assertFalse(err.userMessage.contains("computador", ignoreCase = true))
        assertFalse(err.userMessage.contains("10.0.2.2"))
        assertFalse(err.userMessage.contains("192.168"))
        assertEquals("timeout", err.technical)
        assertEquals("http://10.0.2.2:8080", err.baseUrl)
    }

    @Test
    fun `ServidorNoResponde guarda la URL en technical si no hay detalle`() {
        val err = AppError.ServidorNoResponde("https://nube.example")
        assertEquals("https://nube.example", err.technical)
        assertFalse(err.userMessage.contains("nube.example"))
    }

    @Test
    fun `DireccionInvalida sigue con ejemplo LAN local`() {
        val err = AppError.DireccionInvalida()
        assertTrue(err.userMessage.contains("192.168"))
    }
}
