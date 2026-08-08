package cl.rutbusiness.core.backup

import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * AES-256-GCM + SHA-256 del respaldo feriante (mismo stack que el token store,
 * pero con llave del usuario, no Keystore).
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
}
