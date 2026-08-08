package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * Snapshot **plaintext** que va **dentro** del sobre AES-GCM (ADR-0022).
 *
 * Flujo: armar [SnapshotBackupV1] → [empaquetarSnapshot] (JSON UTF-8) →
 * Argon2id+AES-GCM (cliente, pendiente libs) → `POST /api/v1/user-backup`.
 *
 * El server **nunca** ve este JSON. Solo ciphertext + [MetaBackupCifrado].
 *
 * `snapshot_version` es el formato del **contenido** (secciones). Distinto de
 * `format_version` del sobre (KDF/AEAD en [SobreCifrado]).
 */
const val SNAPSHOT_VERSION: Int = 1

/** Claves de sección conocidas (extensibles sin romper parsers viejos). */
object SeccionSnapshot {
    const val PENDING_SALES = "pending_sales"
    const val RUBRO = "rubro"
    const val NOTE = "note"
}

@Serializable
data class SnapshotBackupV1(
    @SerialName("snapshot_version") val snapshotVersion: Int = SNAPSHOT_VERSION,
    @SerialName("created_at_unix") val createdAtUnix: Long,
    @SerialName("tenant_id") val tenantId: String,
    val rubro: String? = null,
    /** Etiqueta del aparato / build (no PII obligatorio). */
    @SerialName("device_label") val deviceLabel: String? = null,
    /**
     * Ventas cobradas offline que aún no llegaron al server.
     * Prioridad #1 del feriante: no perder el día si se rompe el teléfono.
     */
    @SerialName("pending_sales") val pendingSales: List<VentaEnCola> = emptyList(),
    /**
     * Secciones extra (JSON strings opacos) para fiado cache, catálogo, etc.
     * v1 puede ir vacío; el restore ignora claves desconocidas.
     */
    val sections: Map<String, String> = emptyMap(),
)

private val jsonSnapshot = Json {
    ignoreUnknownKeys = true
    encodeDefaults = true
}

/**
 * Valida forma mínima antes de cifrar o de rehidratar.
 * No toca crypto ni red.
 */
fun validarSnapshot(s: SnapshotBackupV1): String? {
    if (s.snapshotVersion != SNAPSHOT_VERSION) {
        return "snapshot_version=${s.snapshotVersion} (esperado $SNAPSHOT_VERSION)"
    }
    if (s.tenantId.isBlank()) return "tenant_id vacío"
    if (s.createdAtUnix <= 0L) return "created_at_unix inválido"
    return null
}

/** UTF-8 JSON listo para AEAD. Falla si la forma no calza. */
fun empaquetarSnapshot(s: SnapshotBackupV1): Result<ByteArray> {
    validarSnapshot(s)?.let { return Result.failure(IllegalArgumentException(it)) }
    return Result.success(jsonSnapshot.encodeToString(s).encodeToByteArray())
}

/** Inverse de [empaquetarSnapshot]. */
fun desempaquetarSnapshot(bytes: ByteArray): Result<SnapshotBackupV1> {
    if (bytes.isEmpty()) return Result.failure(IllegalArgumentException("snapshot vacío"))
    return try {
        val s = jsonSnapshot.decodeFromString<SnapshotBackupV1>(bytes.decodeToString())
        validarSnapshot(s)?.let { return Result.failure(IllegalArgumentException(it)) }
        Result.success(s)
    } catch (e: Exception) {
        Result.failure(e)
    }
}

/**
 * Arma un snapshot de day-1 a partir de la cola offline.
 *
 * No incluye catálogo ni fiado aún (secciones futuras). Suficiente para no
 * perder ventas del puesto al restaurar.
 */
fun armarSnapshotDesdeCola(
    tenantId: String,
    createdAtUnix: Long,
    pendingSales: List<VentaEnCola>,
    rubro: String? = null,
    deviceLabel: String? = "android",
): SnapshotBackupV1 = SnapshotBackupV1(
    createdAtUnix = createdAtUnix,
    tenantId = tenantId.trim(),
    rubro = rubro?.trim()?.takeIf { it.isNotEmpty() },
    deviceLabel = deviceLabel,
    pendingSales = pendingSales,
    sections = buildMap {
        if (!rubro.isNullOrBlank()) put(SeccionSnapshot.RUBRO, rubro.trim())
        put(SeccionSnapshot.PENDING_SALES, pendingSales.size.toString())
    },
)

/**
 * Texto de una página para el cuaderno (sin PDF ni QR dibujado).
 *
 * El feriante puede copiar a mano o pegar en una nota. Las 12 palabras van
 * numeradas; el payload QR va al final (bloques, no la frase).
 */
fun textoTarjetaImprimible(
    clave: ClaveDelNegocio,
    tenantSlug: String,
    titulo: String = "RutBusiness - tarjeta de rescate",
): String {
    val qr = payloadQrRescate(tenantSlug, clave.bloques).orEmpty()
    val sb = StringBuilder()
    sb.appendLine(titulo)
    sb.appendLine("Negocio: ${tenantSlug.trim().lowercase()}")
    sb.appendLine()
    sb.appendLine("Palabras (12) - escribí en el cuaderno:")
    clave.palabras.forEachIndexed { i, p ->
        sb.appendLine("  ${i + 1}. $p")
    }
    sb.appendLine()
    sb.appendLine("Bloques: ${clave.bloquesCompletos()}")
    if (qr.isNotEmpty()) {
        sb.appendLine()
        sb.appendLine("Código QR (cuando haya impresora):")
        sb.appendLine(qr)
    }
    sb.appendLine()
    sb.appendLine("Sin esta llave el respaldo no se puede abrir.")
    sb.appendLine("No mandes esto por WhatsApp.")
    return sb.toString().trimEnd() + "\n"
}
