package cl.rutbusiness.core.backup

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ClaveDelNegocioTest {

    @Test
    fun `demo tiene 12 palabras y 8 bloques de 4`() {
        val c = claveDeDemostracion()
        assertEquals(12, c.palabras.size)
        assertEquals(8, c.bloques.size)
        assertTrue(c.bloques.all { it.length == 4 })
        assertTrue(c.fraseCompleta().split(" ").size == 12)
        assertTrue(c.bloquesCompletos().contains("-"))
    }

    @Test
    fun `misma semilla misma clave`() {
        val s = ByteArray(16) { (it * 3 + 7).toByte() }
        val a = generarClaveDelNegocio(s)
        val b = generarClaveDelNegocio(s)
        assertEquals(a, b)
    }
}
