package cl.rutbusiness.core.backup

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

/**
 * La prueba de retiro tiene que dar **exactamente** lo mismo en Kotlin y en
 * Rust, o la restauración desde cero no funciona: el teléfono deriva la prueba
 * y el server compara contra el hash que guardó al subir. Un byte de diferencia
 * y la respuesta es 404 para alguien que copió bien su tarjeta.
 *
 * Los vectores de acá se calcularon con una tercera implementación
 * independiente (Python `hashlib`/`hmac`, 2026-08-10) y están fijados igual en
 * `crates/domain/src/user_backup.rs`. Que los tres coincidan es el punto.
 */
class PruebaDeRetiroTest {

    /** Semilla canónica de [claveDeDemostracion], en hex. */
    private val semillaDemoHex = "0b16212c37424d58630a10"
    private val slug = "puesto-rosa"

    private fun materialDemo() = MaterialRecuperacion.Frase(claveDeDemostracion().palabras)

    @Test
    fun la_semilla_de_demo_es_la_que_dicen_los_vectores() {
        assertEquals(
            semillaDemoHex,
            bytesToHex(semillaDeMaterial(materialDemo()).getOrThrow()),
        )
    }

    @Test
    fun el_salt_de_retiro_calza_con_el_de_rust() {
        assertEquals("7eabebcd62983c130861a9765975fb90", bytesToHex(PruebaDeRetiro.salt(slug)))
    }

    /**
     * El vector completo. Si este número cambia, **todas** las pruebas de retiro
     * ya registradas dejan de servir: los sobres siguen ahí y siguen abriéndose
     * con la tarjeta, pero no se pueden bajar sin sesión. Cambiarlo es subir la
     * versión del formato, no editar la constante.
     */
    @Test
    fun la_prueba_y_su_hash_calzan_con_los_de_rust() {
        val prueba = PruebaDeRetiro.derivar(materialDemo(), slug).getOrThrow()
        assertEquals(
            "498effb6e6a30b3a07792f5e0bdca765800d06541f589ec62a9ddd38ce67e478",
            bytesToHex(prueba),
        )
        assertEquals(
            "8cf5c70b1ab8fcccfa038a98eaecbb5692c9d873dd17b31f7715202ff90afb89",
            PruebaDeRetiro.hashHex(prueba),
        )
    }

    /** Las 12 palabras y los 5 bloques son la misma semilla, así que la misma prueba. */
    @Test
    fun la_frase_y_los_bloques_dan_la_misma_prueba() {
        val clave = claveDeDemostracion()
        val porFrase = PruebaDeRetiro.derivarHex(MaterialRecuperacion.Frase(clave.palabras), slug)
        val porBloques = PruebaDeRetiro.derivarHex(MaterialRecuperacion.Bloques(clave.bloques), slug)
        assertEquals(porFrase.getOrThrow(), porBloques.getOrThrow())
    }

    /**
     * Alguien tipeando en un teléfono nuevo no puede perder su negocio por un
     * espacio de más o una mayúscula.
     */
    @Test
    fun el_slug_se_normaliza_igual_que_en_el_server() {
        val a = PruebaDeRetiro.derivarHex(materialDemo(), "  Puesto-Rosa ").getOrThrow()
        val b = PruebaDeRetiro.derivarHex(materialDemo(), slug).getOrThrow()
        assertEquals(b, a)
    }

    /** Dos negocios con la misma tarjeta imaginaria no comparten prueba. */
    @Test
    fun otro_slug_da_otra_prueba() {
        assertNotEquals(
            PruebaDeRetiro.derivarHex(materialDemo(), slug).getOrThrow(),
            PruebaDeRetiro.derivarHex(materialDemo(), "otro-puesto").getOrThrow(),
        )
    }

    /**
     * Las dos ramas de la derivación no se tocan.
     *
     * Si la prueba coincidiera con la llave del sobre, mandarla al server sería
     * mandarle la llave — y todo el diseño de conocimiento cero se caería por
     * esta puerta. El salt distinto es lo que lo garantiza, pero se fija con un
     * test porque un salt es una línea fácil de "simplificar" sin darse cuenta.
     */
    @Test
    fun la_prueba_de_retiro_no_es_la_llave_del_sobre() {
        val material = materialDemo()
        val prueba = PruebaDeRetiro.derivar(material, slug).getOrThrow()
        // Peor caso a propósito: la llave derivada con el MISMO salt de retiro.
        val llave = derivarClaveDeMaterial(material, PruebaDeRetiro.salt(slug))
        assertNotEquals(bytesToHex(llave), bytesToHex(prueba))
    }

    @Test
    fun sin_slug_no_hay_prueba() {
        assertTrue(PruebaDeRetiro.derivar(materialDemo(), "   ").isFailure)
    }

    /**
     * Regresión de la pérdida de entropía del PBKDF2 (2026-08-10).
     *
     * La implementación anterior en Android hacía `password.decodeToString()`
     * para poder usar `PBEKeySpec`, que exige `CharArray`. La semilla son 11
     * bytes crudos, no UTF-8: cualquier byte >= 0x80 que no forme una secuencia
     * válida se convertía en U+FFFD, y semillas distintas colapsaban en la
     * misma contraseña. Medido sobre semillas al azar: 45 colisiones en 200.000
     * y 454 en 800.000, o sea un espacio efectivo de ~2^29 en vez de los 2^84
     * que la tarjeta promete. Eso borraba en silencio el margen de 2^101 que
     * documenta [ClaveDelNegocio].
     *
     * Estas dos semillas se diferencian **sólo** en bytes altos y en un patrón
     * que el decode viejo aplastaba. Si alguien vuelve a meter un `CharArray`
     * en el camino, esto se pone rojo.
     */
    @Test
    fun dos_semillas_que_solo_difieren_en_bytes_altos_dan_llaves_distintas() {
        val salt = ByteArray(KDF_SALT_LEN) { 7 }
        val a = byteArrayOf(
            0x80.toByte(), 0x81.toByte(), 0x82.toByte(), 0x83.toByte(), 0x84.toByte(),
            0x85.toByte(), 0x86.toByte(), 0x87.toByte(), 0x88.toByte(), 0x89.toByte(),
            0x8a.toByte(),
        )
        val b = byteArrayOf(
            0x90.toByte(), 0x91.toByte(), 0x92.toByte(), 0x93.toByte(), 0x94.toByte(),
            0x95.toByte(), 0x96.toByte(), 0x97.toByte(), 0x98.toByte(), 0x99.toByte(),
            0x9a.toByte(),
        )
        val ka = CryptoPlataforma.pbkdf2HmacSha256(a, salt, 100_000, AES_KEY_LEN)
        val kb = CryptoPlataforma.pbkdf2HmacSha256(b, salt, 100_000, AES_KEY_LEN)
        assertNotEquals(bytesToHex(ka), bytesToHex(kb))
    }

    /**
     * PBKDF2 contra el vector de RFC 6070 (el de SHA-1 no sirve; éste es el
     * vector de SHA-256 publicado y verificado con `hashlib` el 2026-08-10).
     *
     * Con `PBEKeySpec` este test también pasaba —"password" y "salt" son ASCII—
     * y por eso el bug pudo vivir tanto: los vectores estándar usan texto, y el
     * caso que rompía era justamente el que ningún vector cubre. Va igual, para
     * que el PBKDF2 escrito a mano no se desvíe del algoritmo.
     */
    @Test
    fun el_pbkdf2_calza_con_el_vector_publicado() {
        val k = CryptoPlataforma.pbkdf2HmacSha256(
            password = "password".encodeToByteArray(),
            salt = "salt".encodeToByteArray(),
            iterations = 4096,
            outLen = 32,
        )
        assertEquals("c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a", bytesToHex(k))
    }

    /** HMAC-SHA256 contra el vector de RFC 4231 (caso 2). */
    @Test
    fun el_hmac_calza_con_el_vector_publicado() {
        val mac = CryptoPlataforma.hmacSha256(
            key = "Jefe".encodeToByteArray(),
            message = "what do ya want for nothing?".encodeToByteArray(),
        )
        assertEquals("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843", bytesToHex(mac))
    }
}
