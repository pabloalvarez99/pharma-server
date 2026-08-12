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
 * Texto de una página para el cuaderno (copiar / notas).
 *
 * El feriante puede copiar a mano o pegar en una nota. Las 12 palabras van
 * numeradas; el payload QR va al final (bloques, no la frase).
 * Para PDF / sistema de impresión ver [htmlTarjetaImprimible].
 */
fun textoTarjetaImprimible(
    clave: ClaveDelNegocio,
    tenantSlug: String,
    titulo: String = "RutAgent - tarjeta de rescate",
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
        sb.appendLine("Código QR (payload de bloques):")
        sb.appendLine(qr)
    }
    sb.appendLine()
    sb.appendLine("Sin esta llave el respaldo no se puede abrir.")
    sb.appendLine("No mandes esto por WhatsApp.")
    return sb.toString().trimEnd() + "\n"
}

/**
 * HTML de **una página** para imprimir o "Guardar como PDF" (PrintManager).
 *
 * Sin dependencias nativas: tipografía monoespaciada, palabras numeradas,
 * bloques y opcionalmente un SVG del QR (matriz booleana). El SVG codifica
 * solo el payload de bloques - nunca las 12 palabras.
 *
 * [svgQr] se inserta tal cual dentro del body (debe ser markup confiable
 * generado por [svgMatrizCodigo]).
 */
fun htmlTarjetaImprimible(
    clave: ClaveDelNegocio,
    tenantSlug: String,
    svgQr: String? = null,
    titulo: String = "RutAgent - tarjeta de rescate",
): String {
    val slug = tenantSlug.trim().lowercase()
    val qrPayload = payloadQrRescate(tenantSlug, clave.bloques).orEmpty()
    val palabrasHtml = clave.palabras.mapIndexed { i, p ->
        "<li><strong>${i + 1}.</strong> ${escaparHtml(p)}</li>"
    }.joinToString("\n")
    val qrBlock = buildString {
        if (!svgQr.isNullOrBlank()) {
            appendLine("""<div class="qr">${svgQr.trim()}</div>""")
            appendLine("""<p class="hint">QR de bloques (escaneable). Las 12 palabras solo en el cuaderno.</p>""")
        }
        if (qrPayload.isNotEmpty()) {
            appendLine("""<p class="mono small">Payload: ${escaparHtml(qrPayload)}</p>""")
        }
    }
    return """
<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>${escaparHtml(titulo)}</title>
<style>
  @page { size: A4; margin: 16mm; }
  body {
    font-family: "Segoe UI", system-ui, sans-serif;
    color: #111;
    line-height: 1.35;
    max-width: 180mm;
    margin: 0 auto;
  }
  h1 { font-size: 20pt; margin: 0 0 4mm; }
  h2 { font-size: 12pt; margin: 6mm 0 2mm; }
  .warn {
    border: 2px solid #111;
    padding: 3mm 4mm;
    margin: 4mm 0;
    background: #fff8e6;
    font-size: 10pt;
  }
  ol.words {
    columns: 2;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12pt;
    padding-left: 6mm;
  }
  .blocks {
    font-family: ui-monospace, Consolas, monospace;
    font-size: 14pt;
    letter-spacing: 0.04em;
    text-align: center;
    border: 1px solid #333;
    padding: 3mm;
  }
  .qr { text-align: center; margin: 4mm 0; }
  .qr svg { width: 48mm; height: 48mm; }
  .mono { font-family: ui-monospace, Consolas, monospace; word-break: break-all; }
  .small { font-size: 8pt; color: #333; }
  .hint { font-size: 9pt; color: #333; }
  .foot { margin-top: 6mm; font-size: 9pt; }
</style>
</head>
<body>
  <h1>${escaparHtml(titulo)}</h1>
  <p>Negocio: <strong class="mono">${escaparHtml(slug)}</strong></p>
  <div class="warn">
    <strong>Pegá esta hoja en tu cuaderno.</strong>
    Sin esta llave el respaldo cifrado no se puede abrir.
    RutAgent no puede recuperarla. No la mandes por WhatsApp.
  </div>
  <h2>Palabras (12)</h2>
  <ol class="words">
$palabrasHtml
  </ol>
  <h2>Bloques (lápiz)</h2>
  <p class="blocks">${escaparHtml(clave.bloquesCompletos())}</p>
  <h2>Código QR de rescate</h2>
$qrBlock
  <p class="foot">RutAgent · tarjeta de rescate · una página · no compartir por chat</p>
</body>
</html>
""".trim() + "\n"
}

/**
 * SVG de una matriz booleana (QR o huella). Módulos negros = true.
 * Escala en viewBox 0..n para que el CSS fije el tamaño físico.
 */
fun svgMatrizCodigo(grilla: Array<BooleanArray>): String? {
    if (grilla.isEmpty()) return null
    val h = grilla.size
    val w = grilla.maxOfOrNull { it.size } ?: return null
    if (w == 0) return null
    val sb = StringBuilder()
    sb.append("""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 $w $h" shape-rendering="crispEdges">""")
    sb.append("""<rect width="$w" height="$h" fill="#fff"/>""")
    for (y in 0 until h) {
        val row = grilla[y]
        for (x in 0 until minOf(w, row.size)) {
            if (row[x]) {
                sb.append("""<rect x="$x" y="$y" width="1" height="1" fill="#000"/>""")
            }
        }
    }
    sb.append("</svg>")
    return sb.toString()
}

/** Escape mínimo para insertar texto en HTML. */
fun escaparHtml(raw: String): String = buildString(raw.length + 8) {
    for (c in raw) {
        when (c) {
            '&' -> append("&amp;")
            '<' -> append("&lt;")
            '>' -> append("&gt;")
            '"' -> append("&quot;")
            '\'' -> append("&#39;")
            else -> append(c)
        }
    }
}
