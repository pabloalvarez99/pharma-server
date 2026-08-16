package cl.rutbusiness.app.ui.entrada

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy del login (formulario de entrar): nube/feria sin jerga on-prem.
 */
class CopyEntradaTest {

    private fun assertSinJergaNube(donde: String, texto: String) {
        val lower = texto.lowercase()
        for (palabra in listOf(
            "sistema del negocio",
            "computador",
            "servidor",
            "192.168",
            "10.0.2.2",
        )) {
            assertFalse(
                "$donde no debe decir «$palabra»: $texto",
                lower.contains(palabra),
            )
        }
    }

    @Test
    fun `nube sin destino no nombra sistema ni computador`() {
        val s = copyAvisoGoogleSinDestino(pideDireccion = false)
        assertTrue(s.contains("RutAgent", ignoreCase = true))
        assertTrue(s.contains("Reintentá", ignoreCase = true) || s.contains("reintent", ignoreCase = true))
        assertSinJergaNube("aviso Google sin destino (nube)", s)
    }

    @Test
    fun `on-prem sin destino pide el computador no el sistema`() {
        val s = copyAvisoGoogleSinDestino(pideDireccion = true)
        assertTrue(s.contains("computador", ignoreCase = true))
        assertFalse(
            "on-prem no debe decir «sistema del negocio»",
            s.lowercase().contains("sistema del negocio"),
        )
        assertFalse(s.lowercase().contains("servidor"))
        assertFalse(s.contains("192.168"))
    }

    @Test
    fun `fallo Google manda a correo y clave sin decir servidor`() {
        val s = copyAvisoGoogleFalloLogin("No se pudo entrar con Google.")
        assertTrue(s.contains("correo y clave", ignoreCase = true))
        assertFalse(
            "el aviso de Google no debe decir «servidor»",
            s.lowercase().contains("servidor"),
        )
        assertFalse(s.lowercase().contains("sistema del negocio"))
    }
}
