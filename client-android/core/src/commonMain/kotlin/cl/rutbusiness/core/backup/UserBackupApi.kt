package cl.rutbusiness.core.backup

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Cliente del backup cifrado del feriante (ADR-0022).
 *
 * Distinto del admin tar.gz. Sube **solo** ciphertext + meta. Hasta que el
 * bucket exista, el server contesta `accepted: false` con razón clara.
 */
class UserBackupApi(private val api: ApiFactory) {

    suspend fun subir(
        meta: MetaBackupWire,
        ciphertextBase64: String,
    ): Resultado<RespuestaSubida> = llamar(api) {
        api.http.post("${api.baseUrl}$USER_BACKUP_UPLOAD_PATH") {
            contentType(ContentType.Application.Json)
            setBody(PedidoSubida(meta = meta, ciphertextBase64 = ciphertextBase64))
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Sube un [SobreCifradoV1] ya armado en el cliente.
     * El server puede contestar `accepted: false` sin bucket - mensaje honesto.
     */
    suspend fun subirSobre(sobre: SobreCifradoV1): Resultado<RespuestaSubida> =
        subir(
            meta = metaWireDesdeSobre(sobre),
            ciphertextBase64 = envelopeToBase64(sobre.envelopeBytes),
        )

    suspend fun listar(): Resultado<List<MetaBackupWire>> = llamar(api) {
        api.http.get("${api.baseUrl}$USER_BACKUP_UPLOAD_PATH")
            .exigirExito(api.baseUrl)
            .body()
    }

    /**
     * Baja un sobre cifrado por id (lab memory o bucket futuro).
     * El cliente descifra local con la llave del cuaderno.
     */
    suspend fun descargar(backupId: String): Resultado<DescargaBackupWire> = llamar(api) {
        val id = backupId.trim()
        require(id.isNotEmpty()) { "backup_id vacío" }
        api.http.get("${api.baseUrl}$USER_BACKUP_UPLOAD_PATH/$id")
            .exigirExito(api.baseUrl)
            .body()
    }
}

@Serializable
data class DescargaBackupWire(
    val meta: MetaBackupWire,
    @SerialName("ciphertext_base64") val ciphertextBase64: String,
    @SerialName("backup_id") val backupId: String,
)

/** Meta de wire desde el sobre local (sin tocar la frase). */
fun metaWireDesdeSobre(sobre: SobreCifradoV1): MetaBackupWire {
    val m = sobre.meta
    return MetaBackupWire(
        tenantId = m.tenantId,
        formatVersion = m.formatVersion,
        ciphertextSha256Hex = m.ciphertextSha256Hex,
        sizeBytes = m.sizeBytes,
        uploadedAtUnix = m.uploadedAtUnix,
        label = m.label,
    )
}

/**
 * Copy honesta de la respuesta de subida (nunca promete nube si accepted=false).
 */
fun mensajeTrasSubida(prep: PreparacionRespaldo, resp: RespuestaSubida): String {
    val base = prep.mensaje
    return when {
        resp.accepted ->
            "$base · Guardado en la nube (id ${resp.backupId ?: "ok"}). " +
                "La llave del cuaderno sigue siendo tuya."
        else ->
            "$base · El server recibió el sobre cifrado pero " +
                "aún no tiene bucket: ${resp.reason ?: "accepted=false"}. " +
                "En este teléfono el cifrado ya está listo."
    }
}

@Serializable
data class MetaBackupWire(
    @SerialName("tenant_id") val tenantId: String,
    @SerialName("format_version") val formatVersion: Int,
    @SerialName("ciphertext_sha256_hex") val ciphertextSha256Hex: String,
    @SerialName("size_bytes") val sizeBytes: Long,
    @SerialName("uploaded_at_unix") val uploadedAtUnix: Long,
    val label: String? = null,
)

@Serializable
private data class PedidoSubida(
    val meta: MetaBackupWire,
    @SerialName("ciphertext_base64") val ciphertextBase64: String,
)

@Serializable
data class RespuestaSubida(
    val accepted: Boolean = false,
    val reason: String? = null,
    @SerialName("backup_id") val backupId: String? = null,
)
