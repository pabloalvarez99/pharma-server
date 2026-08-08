package cl.rutbusiness.core.backup

/**
 * Primitivas de cripto del respaldo (ADR-0022).
 *
 * `expect`/`actual`: Android usa `javax.crypto` (AES-GCM + SHA-256 +
 * SecureRandom). Sin Keystore: la llave del feriante es la del **cuaderno**,
 * no la del hardware.
 *
 * Argon2id (KDF del format v1) **no** vive acá todavía: se deriva la llave
 * fuera y se pasa a [cifrarSobreV1] / [descifrarSobreV1].
 */
internal expect object CryptoPlataforma {
    fun randomBytes(n: Int): ByteArray
    fun sha256(data: ByteArray): ByteArray
    /**
     * AES-256-GCM. [nonce] 12 bytes. Devuelve ciphertext || tag (16 bytes).
     */
    fun aesGcmEncrypt(key: ByteArray, nonce: ByteArray, plain: ByteArray): ByteArray
    fun aesGcmDecrypt(key: ByteArray, nonce: ByteArray, cipherAndTag: ByteArray): ByteArray
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
