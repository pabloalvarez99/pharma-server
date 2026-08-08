package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.AlmacenDeMentira
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlinx.coroutines.test.runTest

class SobreLocalTest {

    @Test
    fun `guarda y lee base64 del sobre sin frase`() = runTest {
        val disco = AlmacenDeMentira()
        val bytes = "RB1\n{\"k\":1}\nct".encodeToByteArray()
        guardarSobreLocal(disco, bytes)
        val leido = leerSobreLocal(disco)
        assertNotNull(leido)
        assertContentEquals(bytes, leido)
        assertEquals(envelopeToBase64(bytes), leerSobreLocalBase64(disco))
    }
}
