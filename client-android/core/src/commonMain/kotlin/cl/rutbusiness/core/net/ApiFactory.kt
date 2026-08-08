package cl.rutbusiness.core.net

import cl.rutbusiness.core.api.apis.CatalogApi
import cl.rutbusiness.core.api.apis.InventoryApi
import cl.rutbusiness.core.api.apis.SalesApi
import io.ktor.client.HttpClient
import io.ktor.client.engine.HttpClientEngine
import io.ktor.client.plugins.DefaultRequest
import io.ktor.client.plugins.HttpTimeout
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.client.request.header
import io.ktor.http.HttpHeaders
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json

/**
 * Dueño del único [HttpClient] de la app y fábrica de los clientes generados.
 *
 * Un solo cliente para todo: abrir un pool de conexiones por pantalla es
 * justamente lo que un teléfono de 1 GB de RAM no puede pagar. Las clases
 * `*Api` son envoltorios baratos sobre este cliente, así que crearlas por
 * llamada no cuesta nada.
 */
class ApiFactory(
    baseUrl: String,
    /**
     * Quién se entera de si el server contestó. Por defecto nadie: los tests y
     * las herramientas no necesitan el cartel de "sin conexión".
     *
     * Va **antes** que [tokenProvider] para que el proveedor de token siga
     * siendo el último parámetro y las llamadas `ApiFactory(url) { token }` no
     * cambien.
     */
    val red: ReporteDeRed = ReporteDeRed.Nulo,
    /**
     * El motor HTTP. En producción siempre el de la plataforma; las pruebas le
     * pasan uno de mentira para poder ejercitar el camino completo -incluida la
     * cabecera `Idempotency-Key`- sin levantar un server.
     */
    motor: HttpClientEngine = defaultHttpClientEngine(),
    private val tokenProvider: () -> String?,
) {
    /** URL canónica contra la que habla esta instancia. */
    val baseUrl: String = baseUrl.trimEnd('/')

    val http: HttpClient = HttpClient(motor) {
        expectSuccess = false
        install(ContentNegotiation) { json(JSON) }
        // El bearer se pone en el cliente y no en cada `*Api` generado: así lo
        // llevan **todas** las llamadas, incluidas las escritas a mano que el
        // generador no cubre (`/api/v1/me`). Ponerlo por API deja agujeros.
        install(DefaultRequest) {
            tokenProvider()?.let { header(HttpHeaders.Authorization, "Bearer $it") }
        }
        install(HttpTimeout) {
            // Datos móviles lentos son el caso normal, no el borde: los tiempos
            // son generosos a propósito. Lo que no se tolera es quedarse colgado
            // para siempre sin decirle nada al usuario.
            connectTimeoutMillis = 10_000
            requestTimeoutMillis = 30_000
            socketTimeoutMillis = 30_000
        }
    }

    fun catalog(): CatalogApi = CatalogApi(baseUrl, http)

    fun inventory(): InventoryApi = InventoryApi(baseUrl, http)

    fun sales(): SalesApi = SalesApi(baseUrl, http)

    fun close() {
        http.close()
    }

    companion object {
        /**
         * `ignoreUnknownKeys` no es pereza: el server evoluciona más rápido que
         * la app instalada en el teléfono del cliente, y un campo nuevo en la
         * respuesta no puede tumbar una versión vieja.
         */
        val JSON: Json = Json {
            ignoreUnknownKeys = true
            isLenient = true
            explicitNulls = false
        }
    }
}
