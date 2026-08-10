package cl.rutbusiness.core.backup

/**
 * Contrato del sobre de backup cifrado v1 (ADR-0022).
 *
 * Espejo de `domain::user_backup` (format_version, KDF, AEAD). El cifrado real
 * (Argon2id + AES-GCM) se cablea cuando el bucket exista; acá viven:
 * - constantes de parámetros
 * - validación de meta
 * - payload del QR de rescate
 *
 * **Nunca** se manda la frase al server.
 */

const val FORMAT_VERSION: Int = 1
/**
 * KDF real en el cliente hoy. Argon2id sigue siendo el objetivo de producto
 * (domain docs); el header del sobre dice **siempre** el algoritmo usado.
 */
const val KDF_ALG: String = "pbkdf2-hmac-sha256"
const val AEAD_ALG: String = "aes-256-gcm"
/** Solo aplica a Argon2id; en PBKDF2 se ignora (se serializa por compat). */
const val KDF_MEMORY_KIB: Int = 65_536
/** PBKDF2 iterations (OWASP ≥ 210_000 para SHA-256). */
const val KDF_ITERATIONS: Int = 210_000
const val KDF_PARALLELISM: Int = 1

/** Path documentado: `POST /api/v1/user-backup`. */
const val USER_BACKUP_UPLOAD_PATH: String = "/api/v1/user-backup"

/**
 * Cabecera del sobre (salt/nonce en hex). Va junto al ciphertext; el server
 * no la interpreta más allá de guardar bytes opacos.
 */
data class CabeceraSobreV1(
    val formatVersion: Int = FORMAT_VERSION,
    val kdf: String = KDF_ALG,
    val kdfMemoryKib: Int = KDF_MEMORY_KIB,
    val kdfIterations: Int = KDF_ITERATIONS,
    val kdfParallelism: Int = KDF_PARALLELISM,
    val aead: String = AEAD_ALG,
    val saltHex: String,
    val nonceHex: String,
)

/** Metadatos que viajan con el upload (sin plaintext). */
data class MetaBackupCifrado(
    val tenantId: String,
    val formatVersion: Int,
    val ciphertextSha256Hex: String,
    val sizeBytes: Long,
    val uploadedAtUnix: Long,
    val label: String? = null,
)

sealed class ErrorValidacionBackup {
    data object Formato : ErrorValidacionBackup()
    data object TenantVacio : ErrorValidacionBackup()
    data object Sha256Hex : ErrorValidacionBackup()
    data object CiphertextVacio : ErrorValidacionBackup()
    data class Tamanio(val meta: Long, val actual: Long) : ErrorValidacionBackup()
    data object Sha256NoCalza : ErrorValidacionBackup()
}

fun validarMeta(
    meta: MetaBackupCifrado,
    ciphertextSize: Long,
    sha256HexCalculado: String,
): ErrorValidacionBackup? {
    if (meta.formatVersion != FORMAT_VERSION) return ErrorValidacionBackup.Formato
    if (meta.tenantId.isBlank()) return ErrorValidacionBackup.TenantVacio
    if (!esSha256Hex(meta.ciphertextSha256Hex)) return ErrorValidacionBackup.Sha256Hex
    if (ciphertextSize <= 0L) return ErrorValidacionBackup.CiphertextVacio
    if (meta.sizeBytes != ciphertextSize) {
        return ErrorValidacionBackup.Tamanio(meta.sizeBytes, ciphertextSize)
    }
    if (!meta.ciphertextSha256Hex.equals(sha256HexCalculado, ignoreCase = true)) {
        return ErrorValidacionBackup.Sha256NoCalza
    }
    return null
}

private fun esSha256Hex(s: String): Boolean =
    s.length == 64 && s.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }

/**
 * Payload del QR / línea imprimible de la tarjeta de rescate.
 *
 * `rutbusiness-rescue:v1:<tenant>:<BLOQUE>-…` (5 bloques de 4).
 * Las 12 palabras **no** van en el QR (fácil de fotografiar y reenviar).
 */
fun payloadQrRescate(tenantSlug: String, bloques: List<String>): String? {
    if (bloques.size != BLOQUES_TARJETA || bloques.any { it.length != LARGO_BLOQUE }) return null
    val slug = tenantSlug.trim().lowercase()
    if (slug.isEmpty()) return null
    return "rutbusiness-rescue:v1:$slug:${bloques.joinToString("-")}"
}

/** Parsea el payload del QR. Devuelve slug + bloques unidos con `-`. */
fun parsearPayloadQrRescate(raw: String): Pair<String, String>? {
    val s = raw.trim()
    val rest = s.removePrefix("rutbusiness-rescue:v1:")
    if (rest === s || !s.startsWith("rutbusiness-rescue:v1:")) return null
    val sep = rest.indexOf(':')
    if (sep <= 0) return null
    val slug = rest.substring(0, sep).lowercase()
    val blocks = rest.substring(sep + 1)
    val parts = blocks.split('-', ' ').filter { it.isNotEmpty() }
    if (parts.size != BLOQUES_TARJETA || parts.any { it.length != LARGO_BLOQUE }) return null
    return slug to blocks
}
