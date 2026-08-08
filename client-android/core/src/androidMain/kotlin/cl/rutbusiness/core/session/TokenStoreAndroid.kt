package cl.rutbusiness.core.session

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Guarda el JWT cifrado con una llave AES-256 que se genera dentro del
 * AndroidKeyStore y nunca sale de ahí. Lo que queda en disco es ciphertext:
 * aunque alguien saque el `shared_prefs` de un teléfono rooteado, sin la llave
 * del Keystore no sirve.
 *
 * **Por qué a mano y no `androidx.security:security-crypto`**: esa librería está
 * deprecada por Google y ya no recibe correcciones. El camino recomendado hoy es
 * exactamente éste — Keystore + AES/GCM directo —, que además evita una
 * dependencia más en el arranque.
 *
 * Un solo camino: `KeyGenParameterSpec` existe desde API 23, que es el `minSdk`
 * de la app. La rama de API 21-22 —llave AES envuelta con un par RSA del
 * Keystore viejo— se borró junto con el cambio a `minSdk 23` (2026-08-07); era
 * código que nunca llegó a ejecutarse en un aparato real.
 */
internal class TokenStoreAndroid(context: Context) : TokenStore {

    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(ARCHIVO_PREFS, Context.MODE_PRIVATE)

    override suspend fun leer(): String? = withContext(Dispatchers.IO) {
        val guardado = prefs.getString(CLAVE_TOKEN, null) ?: return@withContext null
        runCatching { descifrar(guardado) }.getOrElse {
            // Llave rotada, backup restaurado en otro aparato, Keystore
            // corrupto: el token no se puede recuperar. Se borra y se pide
            // login de nuevo, que es la única salida honesta.
            borrarTodo()
            null
        }
    }

    override suspend fun guardar(token: String) = withContext(Dispatchers.IO) {
        prefs.edit().putString(CLAVE_TOKEN, cifrar(token)).apply()
    }

    override suspend fun borrar() = withContext(Dispatchers.IO) {
        borrarTodo()
    }

    private fun borrarTodo() {
        prefs.edit().remove(CLAVE_TOKEN).apply()
    }

    // --- cifrado ------------------------------------------------------------

    private fun cifrar(plano: String): String {
        val cipher = Cipher.getInstance(TRANSFORMACION_AES)
        cipher.init(Cipher.ENCRYPT_MODE, llaveAes())
        val iv = cipher.iv
        val ct = cipher.doFinal(plano.toByteArray(Charsets.UTF_8))
        return "${base64(iv)}:${base64(ct)}"
    }

    private fun descifrar(guardado: String): String {
        val partes = guardado.split(':')
        require(partes.size == 2) { "formato de token guardado desconocido" }
        val iv = desdeBase64(partes[0])
        val ct = desdeBase64(partes[1])
        val cipher = Cipher.getInstance(TRANSFORMACION_AES)
        cipher.init(Cipher.DECRYPT_MODE, llaveAes(), GCMParameterSpec(TAG_BITS, iv))
        return String(cipher.doFinal(ct), Charsets.UTF_8)
    }

    private fun llaveAes(): SecretKey {
        val ks = KeyStore.getInstance(PROVEEDOR_KEYSTORE).apply { load(null) }
        (ks.getEntry(ALIAS_AES, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }

        val generador = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVEEDOR_KEYSTORE)
        generador.init(
            KeyGenParameterSpec.Builder(
                ALIAS_AES,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generador.generateKey()
    }

    private fun base64(bytes: ByteArray): String = Base64.encodeToString(bytes, Base64.NO_WRAP)

    private fun desdeBase64(texto: String): ByteArray = Base64.decode(texto, Base64.NO_WRAP)

    private companion object {
        const val ARCHIVO_PREFS = "rutbusiness_sesion"
        const val CLAVE_TOKEN = "jwt"
        const val PROVEEDOR_KEYSTORE = "AndroidKeyStore"
        const val ALIAS_AES = "rutbusiness_token_aes"
        const val TRANSFORMACION_AES = "AES/GCM/NoPadding"
        const val TAG_BITS = 128
    }
}
