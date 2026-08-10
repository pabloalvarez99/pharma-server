package cl.rutbusiness.core.backup

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class HuellaVisualTest {

    @Test
    fun `misma payload misma huella`() {
        val p = "rutbusiness-rescue:v1:puesto:ABCD-EFGH-IJKL-MNOP-QRST-UVWX-YZ23-4567"
        val a = huellaAscii(p)
        val b = huellaAscii(p)
        assertEquals(a, b)
        assertEquals(HUELLA_SIZE, a.lines().size)
        assertTrue(a.lines().all { it.length == HUELLA_SIZE })
    }

    @Test
    fun `payload distinta cambia la grilla`() {
        val a = huellaAscii("rutbusiness-rescue:v1:a:ABCD-EFGH-IJKL-MNOP-QRST-UVWX-YZ23-4567")
        val b = huellaAscii("rutbusiness-rescue:v1:b:ABCD-EFGH-IJKL-MNOP-QRST-UVWX-YZ23-4567")
        assertNotEquals(a, b)
    }
}
