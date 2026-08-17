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
 * Cliente del backup cifrado del feriante (ADR-0022 / ADR-0023).
 *
 * Distinto del admin tar.gz. Sube **solo** ciphertext + meta. Un nodo sin
 * bucket configurado contesta `accepted: false` con la razón, no inventa nube.
 */
class UserBackupApi(private val api: ApiFactory) {

    suspend fun subir(
        meta: MetaBackupWire,
        ciphertextBase64: String,
        retrievalHashHex: String? = null,
    ): Resultado<RespuestaSubida> = llamar(api) {
        api.http.post("${api.baseUrl}$USER_BACKUP_UPLOAD_PATH") {
            contentType(ContentType.Application.Json)
            setBody(
                PedidoSubida(
                    meta = meta,
                    ciphertextBase64 = ciphertextBase64,
                    retrievalHashHex = retrievalHashHex,
                ),
            )
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Sube un [SobreCifradoV1] ya armado en el cliente.
     *
     * [retrievalHashHex] es `SHA-256(prueba_retiro)` (ver [PruebaDeRetiro]).
     * Mandarlo es lo que después deja bajar este sobre desde un teléfono nuevo,
     * sin sesión. Es opcional en el cable para no romper apps ya instaladas,
     * pero la app que puede calcularlo **debería** mandarlo siempre: sin él el
     * respaldo sólo se baja desde el aparato que lo subió, que es justamente el
     * que se pierde.
     */
    suspend fun subirSobre(
        sobre: SobreCifradoV1,
        retrievalHashHex: String? = null,
    ): Resultado<RespuestaSubida> =
        subir(
            meta = metaWireDesdeSobre(sobre),
            ciphertextBase64 = envelopeToBase64(sobre.envelopeBytes),
            retrievalHashHex = retrievalHashHex,
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

    /**
     * Teléfono nuevo: baja el sobre más reciente **sin sesión**, presentando la
     * prueba derivada de la tarjeta del cuaderno.
     *
     * Esta es la única llamada del respaldo que se hace con un [ApiFactory] sin
     * token — y tiene que poder hacerse así, porque quien la necesita no tiene
     * cómo conseguir uno.
     *
     * El server contesta **404 para todo lo que falla**: slug que no existe,
     * prueba que no calza, negocio sin respaldos. No hay forma de distinguirlos
     * desde acá, y es a propósito. Lo que la app le dice a la dueña tiene que
     * cubrir los tres casos a la vez (ver [mensajeRescateFallido]).
     */
    suspend fun rescatar(
        tenantSlug: String,
        pruebaHex: String,
    ): Resultado<DescargaBackupWire> = llamar(api) {
        api.http.post("${api.baseUrl}$USER_BACKUP_RESCUE_PATH") {
            contentType(ContentType.Application.Json)
            setBody(
                PedidoRescate(
                    tenantSlug = PruebaDeRetiro.normalizarSlug(tenantSlug),
                    retrievalProofHex = pruebaHex.trim().lowercase(),
                ),
            )
        }.exigirExito(api.baseUrl).body()
    }
}

/**
 * Qué se le dice a alguien cuyo rescate volvió 404.
 *
 * El server no distingue las causas a propósito, así que el mensaje tampoco
 * puede fingir que sabe cuál fue. Lo que sí puede hacer es enumerar las tres
 * cosas revisables, en orden de probabilidad, en vez de dejar un "no
 * encontrado" que no le dice a nadie qué hacer.
 */
fun mensajeRescateFallido(tenantSlug: String): String =
    "No encontramos un respaldo con esos datos. Revisa tres cosas: " +
        "que el nombre del negocio sea exactamente el de la tarjeta " +
        "(\"${PruebaDeRetiro.normalizarSlug(tenantSlug)}\"), " +
        "que no falte ninguna palabra ni letra, y que este negocio haya " +
        "alcanzado a subir un respaldo alguna vez. " +
        "Nadie más que tú puede abrirlo, así que tampoco podemos buscarlo por ti."

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
    /** Presente al listar / tras accept; ausente en el body de subida. */
    @SerialName("backup_id") val backupId: String? = null,
)

@Serializable
private data class PedidoSubida(
    val meta: MetaBackupWire,
    @SerialName("ciphertext_base64") val ciphertextBase64: String,
    /** `SHA-256(prueba_retiro)`. Nunca la prueba, y nunca la semilla. */
    @SerialName("retrieval_hash_hex") val retrievalHashHex: String? = null,
)

@Serializable
private data class PedidoRescate(
    @SerialName("tenant_slug") val tenantSlug: String,
    /**
     * La prueba **sí** va entera acá — es su único uso, y el server la hashea
     * para compararla contra lo que guardó. De la prueba no se llega a la llave
     * del sobre: son ramas distintas de la misma derivación.
     */
    @SerialName("retrieval_proof_hex") val retrievalProofHex: String,
)

@Serializable
data class RespuestaSubida(
    val accepted: Boolean = false,
    val reason: String? = null,
    @SerialName("backup_id") val backupId: String? = null,
)
