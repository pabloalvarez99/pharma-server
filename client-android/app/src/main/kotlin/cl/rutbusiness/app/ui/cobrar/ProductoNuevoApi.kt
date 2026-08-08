package cl.rutbusiness.app.ui.cobrar

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.patch
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.Serializable

/**
 * Crear un producto desde la caja, con el código que se acaba de escanear.
 *
 * Va a mano y no por el cliente generado, por dos razones distintas:
 *
 * 1. **El spec no tipa estos bodies.** `POST /products` y `PATCH /products/{id}`
 *    salen del generador con `JsonElement` de entrada y de salida, así que el
 *    tipo real habría que ponerlo acá igual (mismo caso que `AssistApi`).
 * 2. **Y con `JsonElement` no anda.** Ktor elige el serializador por la clase
 *    en tiempo de ejecución, y los valores de un `JsonObject` armado a mano son
 *    `JsonLiteral`, una clase interna de kotlinx que esa búsqueda no encuentra:
 *    la llamada muere con `Serializer for class 'JsonLiteral' is not found`
 *    **antes de salir el request**, y como `llamar()` traduce cualquier
 *    excepción a "no pudimos conectar", en pantalla se ve como un problema de
 *    red que no existe. Con un `@Serializable` propio el serializador se
 *    resuelve en compilación y el problema desaparece.
 */

/** Lo mínimo que el server pide para dar de alta un producto (`NewProduct`). */
@Serializable
private data class NuevoProducto(
    val name: String,
    /** Decimal como texto: es como viaja la plata en toda esta API. */
    val price: String,
    val stock: Int,
)

/** `UpdateProduct` con un solo campo: reasignar el EAN. */
@Serializable
private data class CambioDeCodigo(val barcode: String)

/**
 * Da de alta el producto.
 *
 * Nace con [stock] unidades -una, la que la cajera tiene en la mano- porque con
 * cero el server rechaza la venta por stock insuficiente y crear el producto no
 * habría servido de nada.
 */
suspend fun crearProducto(
    api: ApiFactory,
    nombre: String,
    precio: String,
    stock: Int = 1,
): Resultado<ProductDto> = llamar(api) {
    api.http.post("${api.baseUrl}/api/v1/products") {
        contentType(ContentType.Application.Json)
        setBody(NuevoProducto(name = nombre, price = precio, stock = stock))
    }.exigirExito(api.baseUrl).body()
}

/**
 * Le pega el código de barras a un producto que ya existe.
 *
 * Va aparte porque `NewProduct` **no** tiene `barcode`: el server lo asigna por
 * `PATCH`, que además es tenant-único y contesta 409 si otro producto ya lo
 * tiene.
 */
suspend fun pegarCodigo(
    api: ApiFactory,
    productoId: String,
    codigo: String,
): Resultado<ProductDto> = llamar(api) {
    api.http.patch("${api.baseUrl}/api/v1/products/$productoId") {
        contentType(ContentType.Application.Json)
        setBody(CambioDeCodigo(codigo))
    }.exigirExito(api.baseUrl).body()
}
