package cl.rutbusiness.core.backup

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MaterialRecuperacionTest {

    @Test
    fun `doce palabras`() {
        val frase = (1..12).joinToString(" ") { "p$it" }
        val m = parsearMaterialRecuperacion(frase).getOrThrow()
        assertTrue(m is MaterialRecuperacion.Frase)
        assertEquals(12, (m as MaterialRecuperacion.Frase).palabras.size)
    }

    @Test
    fun `ocho bloques con guion`() {
        val b = "AB3K-9F2Q-M7NP-4RST-WXY2-HJKL-QRST-VBNM"
        val m = parsearMaterialRecuperacion(b).getOrThrow() as MaterialRecuperacion.Bloques
        assertEquals(8, m.bloques.size)
        assertEquals("AB3K", m.bloques[0])
    }

    @Test
    fun `payload qr`() {
        val bloques = listOf("AB3K", "9F2Q", "M7NP", "4RST", "WXY2", "HJKL", "QRST", "VBNM")
        val payload = payloadQrRescate("puesto", bloques)!!
        val m = parsearMaterialRecuperacion(payload).getOrThrow() as MaterialRecuperacion.Bloques
        assertEquals(8, m.bloques.size)
    }

    @Test
    fun `rechaza basura`() {
        assertTrue(parsearMaterialRecuperacion("solo tres cosas").isFailure)
        assertTrue(parsearMaterialRecuperacion("").isFailure)
    }
}
