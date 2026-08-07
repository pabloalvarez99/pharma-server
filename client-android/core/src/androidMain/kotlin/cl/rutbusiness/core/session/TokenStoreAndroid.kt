// `KeyPairGeneratorSpec` está deprecado desde Android 6, y es exactamente por eso
// que se usa: es la única API de Keystore que existe en Android 5 (API 21-22).
@file:Suppress("DEPRECATION")

package cl.rutbusiness.core.session

import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.security.KeyPairGeneratorSpec
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.annotation.RequiresApi
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.util.Calendar
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec
import javax.security.auth.x500.X500Principal

/**
 * Guarda el JWT cifrado con una llave que vive en el AndroidKeyStore y nunca
 * sale de ahí. Lo que queda en disco es ciphertext: aunque alguien saque el
 * `shared_prefs` de un teléfono rooteado, sin la llave del Keystore no sirve.
 *
 * **Por qué a mano y no `androidx.security:security-crypto`**: esa librería
 * declara `minSdk 23` y el piso de hardware es 21. Meterla obligaría a subir el
 * minSdk de toda la app y dejar fuera Android 5, que es justo el aparato que
 * este producto quiere servir.
 *
 * Dos caminos, porque el Keystore cambió en Android 6:
 * - **API 23+**: llave AES-256 generada dentro del Keystore, AES/GCM directo.
 * - **API 21-22**: el Keystore de esa época solo guarda pares RSA. Se genera una
 *   llave AES al azar, se envuelve con el RSA del Keystore y se guarda envuelta;
 *   el token se cifra con esa AES.
 */
internal class TokenStoreAndroid(context: Context) : TokenStore {

    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(ARCHIVO_PREFS, Context.MODE_PRIVATE)

    private val appContext: Context = context.applicationContext

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

    private fun llaveAes(): SecretKey =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) llaveAesDelKeystore() else llaveAesEnvuelta()

    // --- API 23+ ------------------------------------------------------------

    /**
     * La guarda de versión vive en [llaveAes], una llamada más arriba, y lint no
     * la sigue a través del salto: reportaba cinco `NewApi` sobre
     * `KeyGenParameterSpec` con `./gradlew lint` en rojo. La anotación dice lo
     * que ya era cierto, y de yapa hace que lint verifique que todo el que
     * llame acá venga con su chequeo de `SDK_INT` puesto.
     */
    @RequiresApi(Build.VERSION_CODES.M)
    private fun llaveAesDelKeystore(): SecretKey {
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

    // --- API 21-22 ----------------------------------------------------------

    private fun llaveAesEnvuelta(): SecretKey {
        prefs.getString(CLAVE_AES_ENVUELTA, null)?.let { envuelta ->
            val cipher = Cipher.getInstance(TRANSFORMACION_RSA)
            cipher.init(Cipher.DECRYPT_MODE, parRsa().private)
            return SecretKeySpec(cipher.doFinal(desdeBase64(envuelta)), "AES")
        }

        val bytes = ByteArray(32).also { java.security.SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance(TRANSFORMACION_RSA)
        cipher.init(Cipher.ENCRYPT_MODE, parRsa().public)
        prefs.edit().putString(CLAVE_AES_ENVUELTA, base64(cipher.doFinal(bytes))).apply()
        return SecretKeySpec(bytes, "AES")
    }

    @Suppress("DEPRECATION")
    private fun parRsa(): java.security.KeyPair {
        val ks = KeyStore.getInstance(PROVEEDOR_KEYSTORE).apply { load(null) }
        val privada = ks.getKey(ALIAS_RSA, null) as? java.security.PrivateKey
        val certificado = ks.getCertificate(ALIAS_RSA)
        if (privada != null && certificado != null) {
            return java.security.KeyPair(certificado.publicKey, privada)
        }

        val desde = Calendar.getInstance()
        val hasta = Calendar.getInstance().apply { add(Calendar.YEAR, 30) }
        val generador = KeyPairGenerator.getInstance("RSA", PROVEEDOR_KEYSTORE)
        generador.initialize(
            KeyPairGeneratorSpec.Builder(appContext)
                .setAlias(ALIAS_RSA)
                .setSubject(X500Principal("CN=RutBusiness, O=RutBusiness"))
                .setSerialNumber(BigInteger.ONE)
                .setStartDate(desde.time)
                .setEndDate(hasta.time)
                .build(),
        )
        return generador.generateKeyPair()
    }

    private fun base64(bytes: ByteArray): String = Base64.encodeToString(bytes, Base64.NO_WRAP)

    private fun desdeBase64(texto: String): ByteArray = Base64.decode(texto, Base64.NO_WRAP)

    private companion object {
        const val ARCHIVO_PREFS = "rutbusiness_sesion"
        const val CLAVE_TOKEN = "jwt"
        const val CLAVE_AES_ENVUELTA = "aes_envuelta"
        const val PROVEEDOR_KEYSTORE = "AndroidKeyStore"
        const val ALIAS_AES = "rutbusiness_token_aes"
        const val ALIAS_RSA = "rutbusiness_token_rsa"
        const val TRANSFORMACION_AES = "AES/GCM/NoPadding"
        const val TRANSFORMACION_RSA = "RSA/ECB/PKCS1Padding"
        const val TAG_BITS = 128
    }
}
