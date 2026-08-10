package cl.rutbusiness.core.money

import cl.rutbusiness.core.net.ApiFactory
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.http.isSuccess
import kotlinx.serialization.Serializable

@Serializable
private data class AdminSettingDto(val key: String, val value: String)

/**
 * De dónde sale la moneda del negocio.
 *
 * El country pack guarda el código ISO en el `admin_setting` `money.currency`,
 * por tenant. Si el server todavía no lo tiene (contesta 404), se usa
 * [Moneda.POR_DEFECTO] y la app funciona igual: el día que el setting exista,
 * empieza a respetarse sin tocar una línea de UI.
 *
 * No lanza nunca. Que el server no sepa su moneda no puede impedir cobrar.
 */
class MonedaRepository(private val api: ApiFactory) {

    suspend fun resolver(): Moneda {
        val respuesta = runCatching {
            api.http.get("${api.baseUrl}/api/v1/settings/$CLAVE_MONEDA")
        }.getOrNull() ?: return Moneda.POR_DEFECTO

        if (!respuesta.status.isSuccess()) return Moneda.POR_DEFECTO

        val codigo = runCatching { respuesta.body<AdminSettingDto>().value }.getOrNull()
        return if (codigo.isNullOrBlank()) Moneda.POR_DEFECTO else Moneda.de(codigo)
    }

    private companion object {
        const val CLAVE_MONEDA = "money.currency"
    }
}
