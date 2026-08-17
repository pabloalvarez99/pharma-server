package cl.rutbusiness.app.ui.alta

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy de los carteles del alta: feria sin jerga de sistema/IP;
 * on-prem [PasoDonde] sí nombra computador y dirección de LAN.
 */
class CopyPasosAltaTest {

    private fun assertSinJergaNubeVisible(donde: String, texto: String) {
        val lower = texto.lowercase()
        for (prohibida in listOf("usuario", "computador", "sistema", "192.168", "10.0.2.2")) {
            assertFalse(
                "$donde no debe decir «$prohibida»: $texto",
                lower.contains(prohibida),
            )
        }
    }

    @Test
    fun `feria PasoCuenta no dice usuario IP computador ni sistema`() {
        val c = copyPasoCuenta(esFeria = true)
        assertEquals("marta@correo.cl", c.placeholderCorreo)
        assertTrue(c.ayudaCorreo.lowercase().contains("puesto"))
        assertTrue(c.ayudaClave.contains("por ti"))
        assertTrue(c.ayudaClave.contains(LARGO_MINIMO_DE_CLAVE.toString()))
        assertFalse(c.placeholderCorreo.contains("negocio", ignoreCase = true))
        assertFalse(c.placeholderCorreo.contains("minegocio", ignoreCase = true))
        assertSinJergaNubeVisible("titulo", c.titulo)
        assertSinJergaNubeVisible("placeholder", c.placeholderCorreo)
        assertSinJergaNubeVisible("ayudaCorreo", c.ayudaCorreo)
        assertSinJergaNubeVisible("ayudaClave", c.ayudaClave)
        assertSinJergaNubeVisible("labelCorreo", c.labelCorreo)
        assertSinJergaNubeVisible("labelClave", c.labelClave)
    }

    @Test
    fun `retail PasoCuenta conserva placeholder minegocio`() {
        val c = copyPasoCuenta(esFeria = false)
        assertEquals("dueno@minegocio.cl", c.placeholderCorreo)
        assertTrue(c.ayudaCorreo.lowercase().contains("usuario"))
        assertTrue(c.ayudaClave.contains("por ti"))
        assertTrue(c.ayudaClave.contains(LARGO_MINIMO_DE_CLAVE.toString()))
    }

    @Test
    fun `PasoDonde on-prem sigue con direccion de computador y ejemplo LAN`() {
        val d = copyPasoDonde()
        assertEquals("Dirección del computador", d.labelDireccion)
        assertEquals("192.168.1.10:8080", d.placeholder)
        assertTrue(d.titulo.lowercase().contains("computador"))
        assertTrue(d.cuerpo.lowercase().contains("computador"))
        assertFalse(
            "ayuda on-prem suaviza «sistema»",
            d.ayudaSinDireccion.lowercase().contains("sistema"),
        )
        assertTrue(d.ayudaSinDireccion.lowercase().contains("instaló") ||
            d.ayudaSinDireccion.lowercase().contains("instalo"))
    }

    @Test
    fun `feria PasoNegocio sigue con Huevos de Marta y puesto`() {
        val n = copyPasoNegocio(esFeria = true)
        assertEquals("Huevos de Marta", n.placeholder)
        assertTrue(n.titulo.lowercase().contains("puesto"))
        assertFalse(n.titulo.lowercase().contains("computador"))
        assertFalse(n.ayuda.lowercase().contains("sistema"))
    }

    @Test
    fun `retail PasoNegocio conserva Almacen Dona Rosa`() {
        val n = copyPasoNegocio(esFeria = false)
        assertEquals("Almacén Doña Rosa", n.placeholder)
        assertTrue(n.titulo.lowercase().contains("negocio"))
    }
}
