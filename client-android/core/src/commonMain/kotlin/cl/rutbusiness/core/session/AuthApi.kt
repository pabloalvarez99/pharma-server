package cl.rutbusiness.core.session

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

@Serializable
internal data class LoginRequest(
    val tenant: String,
    val email: String,
    val password: String,
)

@Serializable
data class LoginResponse(
    val token: String,
    @SerialName("token_type") val tokenType: String = "Bearer",
    @SerialName("expires_in") val expiresIn: Long = 0,
)

@Serializable
data class Me(
    val sub: String,
    @SerialName("tenant_id") val tenantId: String,
    val roles: List<String> = emptyList(),
    val exp: Long = 0,
)

/**
 * Login y verificación de sesión.
 *
 * Estos dos endpoints están escritos a mano y **no** salen del generador, a
 * diferencia de los otros 139: `POST /api/v1/login` y `GET /api/v1/me` viven en
 * `crates/api/src/routes.rs` sin anotación `#[utoipa::path]`, así que no
 * aparecen en el spec de OpenAPI. El día que se anoten, este archivo se borra.
 */
class AuthApi(private val api: ApiFactory) {

    suspend fun login(tenant: String, email: String, password: String): Resultado<LoginResponse> =
        llamar(api.baseUrl) {
            api.http.post("${api.baseUrl}/api/v1/login") {
                contentType(ContentType.Application.Json)
                setBody(
                    LoginRequest(
                        tenant = tenant.trim(),
                        email = email.trim().lowercase(),
                        password = password,
                    ),
                )
            }.exigirExito(api.baseUrl).body()
        }

    /** Sirve para saber si el token guardado sigue vivo sin pedir credenciales. */
    suspend fun me(): Resultado<Me> = llamar(api.baseUrl) {
        api.http.get("${api.baseUrl}/api/v1/me").exigirExito(api.baseUrl).body()
    }
}
