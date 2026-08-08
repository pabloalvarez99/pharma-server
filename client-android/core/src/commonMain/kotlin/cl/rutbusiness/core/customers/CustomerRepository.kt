package cl.rutbusiness.core.customers

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import kotlinx.serialization.Serializable

/** Un cliente del negocio. Espejo de `CustomerDto` en el dominio. */
@Serializable
data class ClienteDto(
    val id: String,
    val name: String,
    val rut: String? = null,
    val phone: String? = null,
    val active: Boolean = true,
)

/**
 * Clientes, para el fiado.
 *
 * `GET /api/v1/clientes` está anotado, pero con `body = serde_json::Value`: el
 * generador devuelve `JsonElement` y no hay modelo tipado que pedirle. El
 * filtrado por nombre se hace en el teléfono a propósito — la lista de clientes
 * de un almacén de barrio son decenas, no miles, y buscar local es
 * instantáneo y funciona con la red caída.
 */
class CustomerRepository(private val api: ApiFactory) {

    suspend fun listar(): Resultado<List<ClienteDto>> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/clientes")
            .exigirExito(api.baseUrl)
            .body<List<ClienteDto>>()
            .filter { it.active }
    }
}
