package cl.rutbusiness.core.backup

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/**
 * Cifrado del snapshot → sobre v1 (ADR-0022).
 *
 * Flujo:
 * 1. [empaquetarSnapshot] → plaintext UTF-8
 * 2. [cifrarSobreV1] con llave de 32 B (sale de Argon2id cuando exista)
 * 3. Base64 del envelope → [UserBackupApi.subir]
 *
 * Formato de [SobreCifradoV1.envelopeBytes]:
 * ```
 * RB1\n
 * <json CabeceraSobreWire>\n
 * <ciphertext||tag>
 * ```
 *
 * El server solo ve base64 opaco + meta (sha256/size). **Nunca** la frase.
 */

const val KDF_SALT_LEN: Int = 16
const val AEAD_NONCE_LEN: Int = 12
const val AES_KEY_LEN: Int = 32

private const val MAGIC = "RB1"

@Serializable
data class CabeceraSobreWire(
    @SerialName("format_version") val formatVersion: Int = FORMAT_VERSION,
    val kdf: String = KDF_ALG,
    @SerialName("kdf_memory_kib") val kdfMemoryKib: Int = KDF_MEMORY_KIB,
    @SerialName("kdf_iterations") val kdfIterations: Int = KDF_ITERATIONS,
    @SerialName("kdf_parallelism") val kdfParallelism: Int = KDF_PARALLELISM,
    val aead: String = AEAD_ALG,
    @SerialName("salt_hex") val saltHex: String,
    @SerialName("nonce_hex") val nonceHex: String,
)

data class SobreCifradoV1(
    val header: CabeceraSobreWire,
    /** Ciphertext + tag GCM. */
    val ciphertext: ByteArray,
    /** Bytes del sobre completo (magic + header + ct). */
    val envelopeBytes: ByteArray,
    val meta: MetaBackupCifrado,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SobreCifradoV1) return false
        return header == other.header &&
            ciphertext.contentEquals(other.ciphertext) &&
            envelopeBytes.contentEquals(other.envelopeBytes) &&
            meta == other.meta
    }

    override fun hashCode(): Int {
        var r = header.hashCode()
        r = 31 * r + ciphertext.contentHashCode()
        r = 31 * r + envelopeBytes.contentHashCode()
        r = 31 * r + meta.hashCode()
        return r
    }
}

private val jsonHeader = Json {
    encodeDefaults = true
    ignoreUnknownKeys = true
}

/**
 * Cifra [plaintext] con AES-256-GCM.
 *
 * [key] debe ser 32 bytes (salida Argon2id). [salt] se guarda en el header
 * para que el restore pueda re-derivar la llave; si es null se genera.
 *
 * [kdfLabel] por defecto `argon2id` (contrato v1). Solo cambiar si el KDF
 * real difiere — no mentir en el header.
 */
fun cifrarSobreV1(
    key: ByteArray,
    plaintext: ByteArray,
    tenantId: String,
    uploadedAtUnix: Long,
    salt: ByteArray? = null,
    nonce: ByteArray? = null,
    label: String? = null,
    kdfLabel: String = KDF_ALG,
): Result<SobreCifradoV1> {
    if (key.size != AES_KEY_LEN) {
        return Result.failure(IllegalArgumentException("key AES-256: 32 bytes"))
    }
    if (tenantId.isBlank()) {
        return Result.failure(IllegalArgumentException("tenant_id vacío"))
    }
    if (plaintext.isEmpty()) {
        return Result.failure(IllegalArgumentException("plaintext vacío"))
    }
    return try {
        val saltBytes = salt ?: CryptoPlataforma.randomBytes(KDF_SALT_LEN)
        val nonceBytes = nonce ?: CryptoPlataforma.randomBytes(AEAD_NONCE_LEN)
        require(saltBytes.size == KDF_SALT_LEN)
        require(nonceBytes.size == AEAD_NONCE_LEN)
        val ct = CryptoPlataforma.aesGcmEncrypt(key, nonceBytes, plaintext)
        val header = CabeceraSobreWire(
            kdf = kdfLabel,
            saltHex = bytesToHex(saltBytes),
            nonceHex = bytesToHex(nonceBytes),
        )
        val envelope = empaquetarEnvelope(header, ct)
        val sha = bytesToHex(CryptoPlataforma.sha256(envelope))
        val meta = MetaBackupCifrado(
            tenantId = tenantId.trim(),
            formatVersion = FORMAT_VERSION,
            ciphertextSha256Hex = sha,
            sizeBytes = envelope.size.toLong(),
            uploadedAtUnix = uploadedAtUnix,
            label = label,
        )
        val err = validarMeta(meta, envelope.size.toLong(), sha)
        if (err != null) {
            return Result.failure(IllegalStateException("meta inválida: $err"))
        }
        Result.success(
            SobreCifradoV1(
                header = header,
                ciphertext = ct,
                envelopeBytes = envelope,
                meta = meta,
            ),
        )
    } catch (e: Exception) {
        Result.failure(e)
    }
}

/** Descifra un sobre empaquetado con la misma [key] de 32 B. */
fun descifrarSobreV1(key: ByteArray, envelopeBytes: ByteArray): Result<ByteArray> {
    if (key.size != AES_KEY_LEN) {
        return Result.failure(IllegalArgumentException("key AES-256: 32 bytes"))
    }
    return try {
        val (header, ct) = desempaquetarEnvelope(envelopeBytes)
            .getOrElse { return Result.failure(it) }
        val nonce = hexToBytes(header.nonceHex)
            ?: return Result.failure(IllegalArgumentException("nonce_hex inválido"))
        if (nonce.size != AEAD_NONCE_LEN) {
            return Result.failure(IllegalArgumentException("nonce len"))
        }
        Result.success(CryptoPlataforma.aesGcmDecrypt(key, nonce, ct))
    } catch (e: Exception) {
        Result.failure(e)
    }
}

internal fun empaquetarEnvelope(header: CabeceraSobreWire, ciphertext: ByteArray): ByteArray {
    val headerJson = jsonHeader.encodeToString(header)
    val prefix = "$MAGIC\n$headerJson\n".encodeToByteArray()
    return prefix + ciphertext
}

internal fun desempaquetarEnvelope(envelope: ByteArray): Result<Pair<CabeceraSobreWire, ByteArray>> {
    // Buscar fin de la 2ª línea (header JSON) tras "RB1\n".
    val textStart = envelope
    val magic = "$MAGIC\n".encodeToByteArray()
    if (textStart.size < magic.size + 2) {
        return Result.failure(IllegalArgumentException("sobre corto"))
    }
    for (i in magic.indices) {
        if (textStart[i] != magic[i]) {
            return Result.failure(IllegalArgumentException("magic RB1 ausente"))
        }
    }
    // Encontrar \n después del JSON.
    var nl = -1
    for (i in magic.size until textStart.size) {
        if (textStart[i] == '\n'.code.toByte()) {
            nl = i
            break
        }
    }
    if (nl < 0) return Result.failure(IllegalArgumentException("header sin fin"))
    val headerJson = textStart.copyOfRange(magic.size, nl).decodeToString()
    val ct = textStart.copyOfRange(nl + 1, textStart.size)
    if (ct.isEmpty()) return Result.failure(IllegalArgumentException("sin ciphertext"))
    return try {
        Result.success(jsonHeader.decodeFromString<CabeceraSobreWire>(headerJson) to ct)
    } catch (e: Exception) {
        Result.failure(e)
    }
}

@OptIn(ExperimentalEncodingApi::class)
fun envelopeToBase64(envelope: ByteArray): String = Base64.encode(envelope)

@OptIn(ExperimentalEncodingApi::class)
fun base64ToEnvelope(b64: String): ByteArray = Base64.decode(b64)

/**
 * Material de frase/bloques → 32 bytes **provisional** (NO es Argon2id).
 *
 * Solo para tests de integración del AEAD y demos locales. El header debe
 * etiquetar `kdf` distinto de [KDF_ALG] si se usa esto en un sobre real.
 *
 * `SHA-256(salt || UTF-8(material))` — barato y determinista.
 */
fun derivarClaveProvisional(material: String, salt: ByteArray): ByteArray {
    require(salt.size == KDF_SALT_LEN)
    val raw = salt + material.trim().encodeToByteArray()
    return CryptoPlataforma.sha256(raw)
}

/** `true` cuando exista impl real de Argon2id con params v1. Hoy: false. */
const val ARGON2ID_LISTO: Boolean = false
