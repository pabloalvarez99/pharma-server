package cl.rutbusiness.core.backup

/**
 * Primitivas de cripto del respaldo (ADR-0022).
 *
 * `expect`/`actual`: Android usa `javax.crypto` (AES-GCM + SHA-256 +
 * PBKDF2-HMAC-SHA256 + SecureRandom). Sin Keystore: la llave del feriante
 * es la del **cuaderno**, no la del hardware.
 *
 * KDF actual en sobre v1: **PBKDF2-HMAC-SHA256** (disponible en el JDK sin
 * NDK). Argon2id es el objetivo de producto cuando haya lib multiplatform;
 * el header del sobre dice el algoritmo real (`pbkdf2-hmac-sha256`).
 */
internal expect object CryptoPlataforma {
    fun randomBytes(n: Int): ByteArray
    fun sha256(data: ByteArray): ByteArray
    /**
     * AES-256-GCM. [nonce] 12 bytes. Devuelve ciphertext || tag (16 bytes).
     */
    fun aesGcmEncrypt(key: ByteArray, nonce: ByteArray, plain: ByteArray): ByteArray
    fun aesGcmDecrypt(key: ByteArray, nonce: ByteArray, cipherAndTag: ByteArray): ByteArray
    /**
     * PBKDF2-HMAC-SHA256 → [outLen] bytes (típicamente 32).
     * [iterations] >= 100_000 en producción (OWASP).
     *
     * [password] son **bytes crudos**, no texto: la semilla del cuaderno no es
     * UTF-8 y pasarla por un decode le come la entropía. La implementación de
     * Android lo explica en detalle; si algún día hay una segunda plataforma,
     * la regla es la misma.
     */
    fun pbkdf2HmacSha256(
        password: ByteArray,
        salt: ByteArray,
        iterations: Int,
        outLen: Int,
    ): ByteArray

    /** HMAC-SHA256. Lo usa la prueba de retiro (ver [PruebaDeRetiro]). */
    fun hmacSha256(key: ByteArray, message: ByteArray): ByteArray
}

/** Hex minúscula (salt/nonce/sha en el wire). */
fun bytesToHex(bytes: ByteArray): String = buildString(bytes.size * 2) {
    for (b in bytes) {
        val v = b.toInt() and 0xFF
        append("0123456789abcdef"[v ushr 4])
        append("0123456789abcdef"[v and 0x0f])
    }
}

fun hexToBytes(hex: String): ByteArray? {
    val s = hex.trim()
    if (s.length % 2 != 0) return null
    if (!s.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) return null
    return ByteArray(s.length / 2) { i ->
        s.substring(i * 2, i * 2 + 2).toInt(16).toByte()
    }
}
