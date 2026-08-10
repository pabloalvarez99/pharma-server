package cl.rutbusiness.core.backup

import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
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

    /**
     * PBKDF2-HMAC-SHA256 sobre los bytes **crudos** de [password] (RFC 8018).
     *
     * No usa `PBEKeySpec`, y esa es toda la razon por la que esta escrito a
     * mano. `PBEKeySpec` recibe un `CharArray`, asi que el camino obvio
     * -`password.decodeToString().toCharArray()`- pasa la semilla por un
     * decode UTF-8. La semilla son BYTES_SEMILLA bytes crudos, no texto:
     * cualquier byte >= 0x80 que no forme una secuencia UTF-8 valida se
     * reemplaza por U+FFFD, y semillas distintas terminan produciendo la misma
     * contrasena.
     *
     * Medido el 2026-08-10 sobre semillas aleatorias de 11 bytes: el espacio
     * efectivo caia a ~2^29 en vez de 2^84 (45 colisiones en 200.000 muestras,
     * donde 2^84 predice del orden de 10^-15). Solo el 0,05% de las semillas
     * sobrevivia intacta: las que dan todos sus bytes < 0x80.
     *
     * PBKDF2 define la contrasena como una cadena de octetos; convertirla a
     * caracteres es una comodidad de la API de Java, no parte del algoritmo.
     * Aca se implementa sobre `Mac`, que si toma bytes.
     */
    actual fun pbkdf2HmacSha256(
        password: ByteArray,
        salt: ByteArray,
        iterations: Int,
        outLen: Int,
    ): ByteArray {
        require(iterations >= 1) { "iterations >= 1" }
        require(outLen in 16..64) { "outLen 16..64" }
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(password, "HmacSHA256"))
        val hLen = mac.macLength
        val out = ByteArray(outLen)
        var escrito = 0
        var bloque = 1
        while (escrito < outLen) {
            // U_1 = PRF(P, S || INT_32_BE(i))
            mac.update(salt)
            mac.update(
                byteArrayOf(
                    (bloque ushr 24).toByte(),
                    (bloque ushr 16).toByte(),
                    (bloque ushr 8).toByte(),
                    bloque.toByte(),
                ),
            )
            var u = mac.doFinal()
            val t = u.copyOf()
            // T_i = U_1 xor U_2 xor ... xor U_c
            for (ronda in 2..iterations) {
                u = mac.doFinal(u)
                for (j in 0 until hLen) t[j] = (t[j].toInt() xor u[j].toInt()).toByte()
            }
            val n = minOf(hLen, outLen - escrito)
            t.copyInto(out, escrito, 0, n)
            escrito += n
            bloque++
        }
        return out
    }

    actual fun hmacSha256(key: ByteArray, message: ByteArray): ByteArray {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(key, "HmacSHA256"))
        return mac.doFinal(message)
    }
}
