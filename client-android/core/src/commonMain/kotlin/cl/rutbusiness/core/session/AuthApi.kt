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
        llamar(api) {
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
    suspend fun me(): Resultado<Me> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/me").exigirExito(api.baseUrl).body()
    }

    /**
     * Exchange Google `id_token` → JWT de sesión (ADR-0022).
     *
     * Un server sin client id contesta 501 y el botón ni siquiera aparece (ver
     * `IdentidadGoogle.disponible()`). El cliente **nunca** manda client
     * secret. No loguear [idToken].
     *
     * Un `sub` que no pertenece a ningún negocio recibe 403: esta ruta entra,
     * no da de alta. Para eso está [crearNegocioConGoogle].
     */
    suspend fun loginConGoogle(
        idToken: String,
        tenant: String? = null,
    ): Resultado<LoginResponse> = llamar(api) {
        api.http.post("${api.baseUrl}$GOOGLE_SIGN_IN_PATH") {
            contentType(ContentType.Application.Json)
            setBody(
                GoogleLoginRequest(
                    idToken = idToken,
                    tenant = tenant?.trim()?.takeIf { it.isNotEmpty() },
                ),
            )
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Crear un negocio nuevo con una cuenta de Google (ADR-0022).
     *
     * Ruta aparte de [loginConGoogle] a propósito: entrar toca datos de otro,
     * crear arranca un negocio vacío. Si fuera una bandera del login, un
     * [nombreDelNegocio] mal tipeado dejaría de ser "ese negocio no existe" y
     * pasaría a fabricar uno, con la persona adentro y sin enterarse.
     *
     * [nombreCorto] es opcional: si no va, el server lo deriva del nombre y lo
     * devuelve en la respuesta. Nunca conviene adivinarlo del lado del
     * teléfono — el server puede haberlo ajustado.
     */
    suspend fun crearNegocioConGoogle(
        idToken: String,
        nombreDelNegocio: String,
        nombreCorto: String? = null,
        rubro: String? = null,
    ): Resultado<GoogleAltaResponse> = llamar(api) {
        api.http.post("${api.baseUrl}$GOOGLE_SIGN_UP_PATH") {
            contentType(ContentType.Application.Json)
            setBody(
                GoogleAltaRequest(
                    idToken = idToken,
                    businessName = nombreDelNegocio.trim(),
                    tenantSlug = nombreCorto?.trim()?.takeIf { it.isNotEmpty() },
                    vertical = rubro?.trim()?.takeIf { it.isNotEmpty() },
                ),
            )
        }.exigirExito(api.baseUrl).body()
    }
}

/** Path estable con `domain::google_identity::GOOGLE_SIGN_IN_PATH`. */
const val GOOGLE_SIGN_IN_PATH: String = "/api/v1/auth/google"

/** Path estable con `domain::google_identity::GOOGLE_SIGN_UP_PATH`. */
const val GOOGLE_SIGN_UP_PATH: String = "/api/v1/auth/google/negocio"

@Serializable
private data class GoogleLoginRequest(
    @SerialName("id_token") val idToken: String,
    val tenant: String? = null,
)

@Serializable
private data class GoogleAltaRequest(
    @SerialName("id_token") val idToken: String,
    @SerialName("business_name") val businessName: String,
    @SerialName("tenant_slug") val tenantSlug: String? = null,
    val vertical: String? = null,
)

/**
 * Igual que [LoginResponse] más el nombre corto que decidió el server.
 *
 * Vuelve para que la próxima entrada no dependa de que la persona recuerde
 * qué eligió el servidor por ella.
 */
@Serializable
data class GoogleAltaResponse(
    val token: String,
    @SerialName("token_type") val tokenType: String = "Bearer",
    @SerialName("expires_in") val expiresIn: Long = 0,
    val email: String? = null,
    @SerialName("tenant_slug") val tenantSlug: String,
)
