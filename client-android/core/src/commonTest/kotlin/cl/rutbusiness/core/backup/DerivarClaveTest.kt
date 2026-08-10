package cl.rutbusiness.core.backup

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class DerivarClaveTest {

    // Una clave REAL de la tarjeta: la frase tiene que ser del vocabulario y
    // cuadrar con los bloques, porque el KDF corre sobre la semilla canónica y
    // no sobre el texto. Palabras inventadas ya no derivan llave, a propósito.
    private val clave = claveDeDemostracion()
    private val frase = MaterialRecuperacion.Frase(clave.palabras)

    @Test
    fun `misma frase y salt dan la misma llave`() {
        val salt = ByteArray(KDF_SALT_LEN) { it.toByte() }
        // Iterations rebajadas en test sería ideal; producción exige 100k+.
        // Usamos el default de producción (puede tardar un poco en CI).
        val a = derivarClaveDeMaterial(frase, salt)
        val b = derivarClaveDeMaterial(frase, salt)
        assertEquals(AES_KEY_LEN, a.size)
        assertContentEquals(a, b)
    }

    @Test
    fun `salt distinto cambia la llave`() {
        val salt1 = ByteArray(KDF_SALT_LEN) { 1 }
        val salt2 = ByteArray(KDF_SALT_LEN) { 2 }
        val a = derivarClaveDeMaterial(frase, salt1)
        val b = derivarClaveDeMaterial(frase, salt2)
        assertNotEquals(a.toList(), b.toList())
    }

    @Test
    fun `roundtrip cifrar con llave derivada`() {
        val salt = CryptoPlataforma.randomBytes(KDF_SALT_LEN)
        val key = derivarClaveDeMaterial(frase, salt)
        val plain = """{"snapshot_version":1,"tenant":"x"}""".encodeToByteArray()
        val sobre = cifrarSobreV1(
            key = key,
            plaintext = plain,
            tenantId = "tenant:1",
            uploadedAtUnix = 1L,
            salt = salt,
            kdfLabel = KDF_ALG,
        ).getOrThrow()
        assertEquals(KDF_ALG, sobre.header.kdf)
        val back = descifrarSobreV1(key, sobre.envelopeBytes).getOrThrow()
        assertContentEquals(plain, back)
    }

    @Test
    fun `preparar con material cifra la cola`() {
        val v = VentaEnColaDemo.una()
        val r = prepararRespaldoDesdeCola(
            tenantId = "puesto",
            cola = listOf(v),
            createdAtUnix = 100L,
            materialRecuperacion = frase,
        ).getOrThrow()
        assertTrue(r.hayContenido)
        assertTrue(r.sobre != null)
        assertTrue(r.mensaje.contains("Cifrado", ignoreCase = true))
    }
}

/** Helper mínimo sin depender de SolicitudDeVenta verbose en varios tests. */
private object VentaEnColaDemo {
    fun una() = cl.rutbusiness.core.offline.VentaEnCola(
        clave = "k1",
        solicitud = cl.rutbusiness.core.pos.SolicitudDeVenta(
            items = listOf(
                cl.rutbusiness.core.pos.LineaDeVenta(
                    product = "p",
                    productName = "tomate",
                    quantity = 1,
                    unitPrice = "1000",
                ),
            ),
            paymentMethod = "pos_cash",
        ),
        cobradaEn = 1L,
        lineas = 1,
    )
}
