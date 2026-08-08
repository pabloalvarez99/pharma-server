package cl.rutbusiness.core.backup

import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.PBEKeySpec
import javax.crypto.spec.SecretKeySpec

/**
 * AES-256-GCM + PBKDF2-HMAC-SHA256 del respaldo feriante (llave del
 * cuaderno, no Keystore).
 */
internal actual object CryptoPlataforma {
    private const val TRANSFORM = "AES/GCM/NoPadding"
    private const val TAG_BITS = 128

    actual fun randomBytes(n: Int): ByteArray {
        val out = ByteArray(n)
        SecureRandom().nextBytes(out)
        return out
    }

    actual fun sha256(data: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(data)

    actual fun aesGcmEncrypt(key: ByteArray, nonce: ByteArray, plain: ByteArray): ByteArray {
        require(key.size == 32) { "AES-256: key 32 bytes, hay ${key.size}" }
        require(nonce.size == 12) { "GCM nonce 12 bytes, hay ${nonce.size}" }
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(
            Cipher.ENCRYPT_MODE,
            SecretKeySpec(key, "AES"),
            GCMParameterSpec(TAG_BITS, nonce),
        )
        return cipher.doFinal(plain)
    }

    actual fun aesGcmDecrypt(key: ByteArray, nonce: ByteArray, cipherAndTag: ByteArray): ByteArray {
        require(key.size == 32) { "AES-256: key 32 bytes, hay ${key.size}" }
        require(nonce.size == 12) { "GCM nonce 12 bytes, hay ${nonce.size}" }
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(
            Cipher.DECRYPT_MODE,
            SecretKeySpec(key, "AES"),
            GCMParameterSpec(TAG_BITS, nonce),
        )
        return cipher.doFinal(cipherAndTag)
    }

    actual fun pbkdf2HmacSha256(
        password: ByteArray,
        salt: ByteArray,
        iterations: Int,
        outLen: Int,
    ): ByteArray {
        require(iterations >= 1) { "iterations >= 1" }
        require(outLen in 16..64) { "outLen 16..64" }
        // PBEKeySpec quiere CharArray; UTF-8 de la frase del cuaderno.
        val chars = password.decodeToString().toCharArray()
        try {
            val spec = PBEKeySpec(chars, salt, iterations, outLen * 8)
            val skf = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
            return skf.generateSecret(spec).encoded
        } finally {
            chars.fill('\u0000')
        }
    }
}
