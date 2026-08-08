package cl.rutbusiness.core.backup

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SobreCifradoTest {

    @Test
    fun `meta valida con sha y tamano`() {
        val meta = MetaBackupCifrado(
            tenantId = "tenant:1",
            formatVersion = 1,
            ciphertextSha256Hex = "a".repeat(64),
            sizeBytes = 10,
            uploadedAtUnix = 1,
        )
        assertNull(validarMeta(meta, 10, "a".repeat(64)))
    }

    @Test
    fun `meta rechaza tamano distinto`() {
        val meta = MetaBackupCifrado(
            tenantId = "t",
            formatVersion = 1,
            ciphertextSha256Hex = "b".repeat(64),
            sizeBytes = 99,
            uploadedAtUnix = 1,
        )
        val err = validarMeta(meta, 10, "b".repeat(64))
        assertTrue(err is ErrorValidacionBackup.Tamanio)
    }

    @Test
    fun `qr rescate roundtrip`() {
        val bloques = listOf("AB3K", "9F2Q", "M7NP", "4RST", "WXY2", "HJKL", "QRST", "VBNM")
        val payload = assertNotNull(payloadQrRescate("Puesto-Rosa", bloques))
        assertTrue(payload.startsWith("rutbusiness-rescue:v1:puesto-rosa:"))
        val (slug, blocks) = assertNotNull(parsearPayloadQrRescate(payload))
        assertEquals("puesto-rosa", slug)
        assertEquals(bloques.joinToString("-"), blocks.replace(' ', '-'))
    }

    @Test
    fun `qr sin slug o bloques malos es null`() {
        assertNull(payloadQrRescate("", listOf("AB3K")))
        assertNull(payloadQrRescate("x", List(8) { "AB" }))
    }
}
