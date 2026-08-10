package cl.rutbusiness.app.entrada

import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class QrRescateTest {

    @Test
    fun `payload valido produce matriz no vacia`() {
        val payload =
            "rutbusiness-rescue:v1:puesto-rosa:ABCD-EFGH-IJKL-MNOP-QRST-UVWX-YZ23-4567"
        val m = matrizQrRescate(payload)
        assertNotNull(m)
        assertTrue(m!!.size >= 21)
        assertTrue(m.any { row -> row.any { it } })
    }

    @Test
    fun `payload vacio o sin prefijo no codifica`() {
        assertNull(matrizQrRescate(""))
        assertNull(matrizQrRescate("solo-texto"))
    }
}
