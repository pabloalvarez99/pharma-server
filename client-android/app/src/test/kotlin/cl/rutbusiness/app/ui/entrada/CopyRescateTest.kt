package cl.rutbusiness.app.ui.entrada

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy del rescate con tarjeta de papel (ADR-0023).
 * Nube/feria: puesto + tarjeta + 12 palabras; cero computador/IP/sistema.
 */
class CopyRescateTest {

    private val jergaNube = listOf(
        "computador",
        "servidor",
        "sistema",
        "192.168",
        "10.0.2.2",
        "hetzner",
    )

    private fun todosLosTextos(c: CopyRescate): List<String> = listOfNotNull(
        c.tituloBarra,
        c.subtituloBarra,
        c.tituloCartel,
        c.cuerpoCartel,
        c.tituloCampos,
        c.labelNombre,
        c.ayudaNombre,
        c.labelDireccion,
        c.placeholderDireccion,
        c.ayudaDireccion,
        c.tituloPalabras,
        c.labelPalabras,
        c.ayudaPalabras,
        c.avisoPrivacidad,
        c.ctaRescatar,
        c.ctaBuscando,
        c.ctaEntrar,
        c.tituloFalla,
        c.tituloListo,
        c.errorDireccion,
    )

    private fun assertSinJergaNube(donde: String, c: CopyRescate) {
        for (texto in todosLosTextos(c)) {
            val lower = texto.lowercase()
            for (palabra in jergaNube) {
                assertFalse(
                    "$donde no debe decir «$palabra»: $texto",
                    lower.contains(palabra),
                )
            }
        }
    }

    @Test
    fun `nube feria habla de puesto tarjeta y 12 palabras sin computador ni IP`() {
        val c = copyRescate(pideDireccion = false, esFeria = true)
        assertEquals("Recuperar mi puesto", c.tituloBarra)
        assertTrue(c.subtituloBarra.contains("tarjeta", ignoreCase = true))
        assertTrue(c.tituloCartel.contains("perdiste el teléfono", ignoreCase = true))
        assertTrue(c.cuerpoCartel.contains("puesto", ignoreCase = true))
        assertTrue(c.cuerpoCartel.contains("12 palabras", ignoreCase = true))
        assertTrue(c.cuerpoCartel.contains("tarjeta", ignoreCase = true))
        assertEquals("¿Cómo se llama tu puesto?", c.tituloCampos)
        assertNull(c.labelDireccion)
        assertNull(c.placeholderDireccion)
        assertNull(c.ayudaDireccion)
        assertTrue(c.tituloPalabras.contains("12 palabras"))
        assertTrue(c.labelPalabras.contains("12 palabras"))
        assertEquals("Entrar a mi puesto", c.ctaEntrar)
        assertTrue(c.errorDireccion.contains("RutAgent", ignoreCase = true))
        assertSinJergaNube("rescate nube+feria", c)
    }

    @Test
    fun `nube sin feria no nombra computador ni IP`() {
        val c = copyRescate(pideDireccion = false, esFeria = false)
        assertEquals("Recuperar mi negocio", c.tituloBarra)
        assertNull(c.labelDireccion)
        assertNull(c.placeholderDireccion)
        assertTrue(c.tituloPalabras.contains("12 palabras"))
        assertTrue(c.errorDireccion.contains("Reintenta", ignoreCase = true))
        assertSinJergaNube("rescate nube", c)
    }

    @Test
    fun `on-prem sigue con direccion del computador e IP de ejemplo`() {
        val c = copyRescate(pideDireccion = true, esFeria = false)
        assertEquals("Dirección del computador", c.labelDireccion)
        assertEquals("192.168.1.10:8080", c.placeholderDireccion)
        assertNotNull(c.ayudaDireccion)
        assertEquals("¿Dónde está tu negocio?", c.tituloCampos)
        assertTrue(c.errorDireccion.contains("192.168"))
        assertFalse(
            "on-prem no dice sistema del negocio",
            todosLosTextos(c).joinToString(" ").lowercase().contains("sistema del negocio"),
        )
    }

    @Test
    fun `on-prem feria usa puesto pero puede nombrar computador`() {
        val c = copyRescate(pideDireccion = true, esFeria = true)
        assertEquals("Recuperar mi puesto", c.tituloBarra)
        assertEquals("¿Dónde está tu puesto?", c.tituloCampos)
        assertEquals("Dirección del computador", c.labelDireccion)
        assertTrue(c.cuerpoCartel.contains("puesto", ignoreCase = true))
        assertTrue(c.tituloPalabras.contains("12 palabras"))
    }
}
