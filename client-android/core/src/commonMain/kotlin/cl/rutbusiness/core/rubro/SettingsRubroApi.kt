package cl.rutbusiness.core.rubro

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.put
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.Serializable

/**
 * Lectura/escritura de `business.vertical` (setting del tenant).
 *
 * No es el pack completo: el pack se sirve en [RubroPackRepository].
 * Este API sirve para el alta / preferencias cuando la dueña elige Feria.
 */
class SettingsRubroApi(private val api: ApiFactory) {

    /**
     * Vertical guardado, o `null` si la clave no existe (404) o falla la red.
     * El alta no se muere por un hiccup de settings.
     */
    suspend fun leerVertical(): Resultado<String?> = llamar(api) {
        val resp = api.http.get("${api.baseUrl}/api/v1/settings/business.vertical")
        if (resp.status.value == 404) return@llamar null
        resp.exigirExito(api.baseUrl)
        resp.body<SettingDto>().value
    }

    suspend fun guardarVertical(rubro: String): Resultado<Unit> = llamar(api) {
        api.http.put("${api.baseUrl}/api/v1/settings/business.vertical") {
            contentType(ContentType.Application.Json)
            setBody(SettingBody(value = rubro.trim().lowercase()))
        }.exigirExito(api.baseUrl)
        Unit
    }
}

@Serializable
private data class SettingDto(val key: String? = null, val value: String? = null)

@Serializable
private data class SettingBody(val value: String)
