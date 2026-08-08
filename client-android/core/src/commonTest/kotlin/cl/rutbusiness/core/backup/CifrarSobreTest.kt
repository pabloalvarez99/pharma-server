package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CifrarSobreTest {

    private val keyFija = ByteArray(32) { (it * 7 + 3).toByte() }

    @Test
    fun `aes gcm roundtrip snapshot`() {
        val snap = armarSnapshotDesdeCola(
            tenantId = "puesto-rosa",
            createdAtUnix = 1_700_000_000L,
            pendingSales = listOf(
                VentaEnCola(
                    clave = "k1",
                    solicitud = SolicitudDeVenta(
                        items = listOf(
                            LineaDeVenta("p", "tomate", 2, "1000"),
                        ),
                        paymentMethod = "pos_cash",
                    ),
                    cobradaEn = 1L,
                    lineas = 1,
                ),
            ),
            rubro = "feria",
        )
        val plain = empaquetarSnapshot(snap).getOrThrow()
        val salt = ByteArray(KDF_SALT_LEN) { 1 }
        val nonce = ByteArray(AEAD_NONCE_LEN) { 2 }
        val sobre = cifrarSobreV1(
            key = keyFija,
            plaintext = plain,
            tenantId = "puesto-rosa",
            uploadedAtUnix = 99L,
            salt = salt,
            nonce = nonce,
            kdfLabel = "test-raw-key",
        ).getOrThrow()

        assertEquals(FORMAT_VERSION, sobre.meta.formatVersion)
        assertEquals(sobre.envelopeBytes.size.toLong(), sobre.meta.sizeBytes)
        assertNull(
            validarMeta(
                sobre.meta,
                sobre.envelopeBytes.size.toLong(),
                bytesToHex(CryptoPlataforma.sha256(sobre.envelopeBytes)),
            ),
        )

        val back = descifrarSobreV1(keyFija, sobre.envelopeBytes).getOrThrow()
        assertContentEquals(plain, back)
        val snapBack = desempaquetarSnapshot(back).getOrThrow()
        assertEquals("tomate", snapBack.pendingSales[0].solicitud.items[0].productName)
    }

    @Test
    fun `clave mala no descifra`() {
        val plain = "hola feria".encodeToByteArray()
        val sobre = cifrarSobreV1(
            key = keyFija,
            plaintext = plain,
            tenantId = "t",
            uploadedAtUnix = 1L,
            salt = ByteArray(16) { 9 },
            nonce = ByteArray(12) { 8 },
        ).getOrThrow()
        val otra = ByteArray(32) { 0 }
        assertTrue(descifrarSobreV1(otra, sobre.envelopeBytes).isFailure)
    }

    @Test
    fun `base64 envelope roundtrip`() {
        val plain = byteArrayOf(1, 2, 3, 4, 5)
        val sobre = cifrarSobreV1(
            key = keyFija,
            plaintext = plain,
            tenantId = "t",
            uploadedAtUnix = 1L,
            salt = ByteArray(16) { 3 },
            nonce = ByteArray(12) { 4 },
        ).getOrThrow()
        val b64 = envelopeToBase64(sobre.envelopeBytes)
        val back = base64ToEnvelope(b64)
        assertContentEquals(sobre.envelopeBytes, back)
        assertContentEquals(plain, descifrarSobreV1(keyFija, back).getOrThrow())
    }

    @Test
    fun `derivar provisional es determinista`() {
        val salt = ByteArray(16) { 5 }
        val a = derivarClaveProvisional("mesa silla pan", salt)
        val b = derivarClaveProvisional("mesa silla pan", salt)
        assertContentEquals(a, b)
        assertEquals(32, a.size)
    }
}
