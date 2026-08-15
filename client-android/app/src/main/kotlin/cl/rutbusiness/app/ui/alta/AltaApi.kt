package cl.rutbusiness.app.ui.alta

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
 * Dar de alta un negocio desde el teléfono, sin que nadie tenga que dictar nada.
 *
 * ## Cuál de los dos endpoints, y por qué NO el otro
 *
 * El server tiene dos caminos para crear un negocio y sólo uno se puede usar
 * desde una app pública:
 *
 * | Endpoint | Qué lo protege | ¿Sirve acá? |
 * |---|---|---|
 * | `POST /api/v1/setup` | base **sin** usuarios (nodo de un puesto) | lab / teléfono |
 * | `POST /api/v1/alta` | slug y correo libres (nube compartida) | **feria** |
 * | `POST /admin/v1/tenants` | secreto `X-Provisioning-Key` | **no** |
 *
 * `/admin/v1/tenants` está hecho para máquina a máquina: lo llama el sitio web
 * de registro, que corre en un servidor y guarda la clave en su propio entorno
 * (ver `docs/product/saas-web-prompts/sp4-signup-landing.md`). Un APK no tiene
 * dónde guardar esa clave: cualquiera lo descarga de la tienda, lo descompila
 * en dos minutos y se lleva la llave que crea negocios en el nodo compartido.
 * Por eso esta app **no manda `X-Provisioning-Key` ni lo tiene compilado**, y
 * por eso el alta apunta a un servidor que todavía no tiene cuenta.
 *
 * `/api/v1/setup` no necesita ningún secreto porque su permiso es un hecho, no
 * una llave: sólo contesta mientras la base entera esté sin usuarios. Apenas
 * existe una cuenta, contesta 409 para siempre. No se puede abusar de él para
 * crear cuentas de más — la primera que se crea lo cierra.
 *
 * La consecuencia de diseño es que **un negocio = un servidor**, que es
 * exactamente el modelo que hace que esto pueda ser gratis: cada negocio corre
 * el suyo (en el local, arrendado, o algún día en el propio teléfono) y el
 * costo por negocio nuevo no lo paga nadie más.
 */

/** Respuesta de `GET /api/v1/setup/status`. */
@Serializable
data class EstadoDelServidor(
    /** `true` cuando la base no tiene ni un usuario: se puede crear el negocio. */
    @SerialName("needs_setup") val necesitaAlta: Boolean,
)

/** Cuerpo de `POST /api/v1/setup`. */
@Serializable
private data class AltaDeNegocio(
    @SerialName("business_name") val nombreDelNegocio: String,
    val email: String,
    val password: String,
    /** La clave del rubro. Ver [RUBROS]. */
    val vertical: String,
)

/**
 * Respuesta de `POST /api/v1/setup`.
 *
 * Trae un token listo para usar, pero esta pantalla **no lo usa**: ver
 * [crearNegocio].
 */
@Serializable
data class NegocioCreado(
    val token: String,
    /**
     * El nombre corto que el server le puso al negocio, derivado del nombre
     * largo. Es el que hay que escribir en "Sucursal" para entrar, y por eso
     * viene de vuelta: adivinarlo en el cliente sería repetir el `slugify` del
     * server y desincronizarse la primera vez que alguien lo toque.
     */
    @SerialName("tenant_slug") val nombreCorto: String,
)

/**
 * ¿Este servidor está libre para crear un negocio?
 *
 * Se pregunta **antes** de mostrar el formulario, no después de llenarlo. Que
 * te digan "acá ya hay un negocio" recién cuando tocaste "Crear", con el nombre
 * y la clave ya escritos, es la clase de cosa que hace cerrar la app.
 */
suspend fun estadoDelServidor(api: ApiFactory): Resultado<EstadoDelServidor> = llamar(api) {
    api.http.get("${api.baseUrl}/api/v1/setup/status")
        .exigirExito(api.baseUrl)
        .body()
}

/**
 * Crea el negocio y su dueña.
 *
 * A mano y no por el cliente generado por la misma razón que
 * [cl.rutbusiness.app.ui.cobrar.crearProducto]: `/setup` no está en el OpenAPI
 * —no tiene `#[utoipa::path]`—, así que el generador no lo conoce. Los DTOs son
 * `@Serializable` propios y no `JsonObject` armado a mano, porque Ktor elige el
 * serializador por la clase en tiempo de ejecución y un `JsonLiteral` muere
 * antes de salir el request, disfrazado de error de red.
 *
 * **El token que devuelve el server se descarta y se hace un login normal.** Se
 * ve como un viaje de más y lo es —unos 200 ms—, pero compra que la sesión se
 * abra por el único camino que ya está probado ([SessionRepository.entrar]):
 * ese camino guarda el token cifrado, la dirección, el nombre corto y el correo
 * en el mismo orden que siempre. La alternativa era abrirle un segundo camino a
 * la sesión sólo para el alta, y una sesión con dos formas de nacer es una
 * sesión con el doble de maneras de quedar a medias.
 */
suspend fun crearNegocio(
    api: ApiFactory,
    nombreDelNegocio: String,
    rubro: String,
    email: String,
    clave: String,
    enLaNube: Boolean = false,
): Resultado<NegocioCreado> = llamar(api) {
    val camino = if (enLaNube) "/api/v1/alta" else "/api/v1/setup"
    api.http.post("${api.baseUrl}$camino") {
        contentType(ContentType.Application.Json)
        setBody(
            AltaDeNegocio(
                nombreDelNegocio = nombreDelNegocio,
                email = email,
                password = clave,
                vertical = rubro,
            ),
        )
    }.exigirExito(api.baseUrl).body()
}
