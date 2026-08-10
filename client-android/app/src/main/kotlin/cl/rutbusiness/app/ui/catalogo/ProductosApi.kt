package cl.rutbusiness.app.ui.catalogo

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
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Escribir el catálogo: crear un producto, editarlo, pegarle un código.
 *
 * **Vive acá y no en `ui/cobrar/` a propósito.** Hasta hoy estas dos llamadas
 * eran privadas del escáner (`ProductoNuevoApi.kt`), y eso hacía que el único
 * camino para cargar el primer producto pasara por la cámara — que es
 * exactamente lo que el pack `feria` apaga (`barcode: false`). Cargar lo que se
 * vende es una operación del catálogo; que el escáner también la use es un
 * detalle del escáner.
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
 *
 * El campo [NuevoProducto.attrs] **sí** es un `JsonObject` y no rompe esa regla:
 * lo que fallaba era pasar un `JsonElement` como *body*, donde Ktor busca el
 * serializador por la clase real del objeto. Acá el tipo está declarado en un
 * `@Serializable`, así que kotlinx usa `JsonObject.serializer()` —elegido en
 * compilación— y ése sí sabe escribir cada primitivo. Es lo que deja conservar
 * los atributos que esta pantalla no edita sin convertir sus números en texto.
 */

/** Lo mínimo que el server pide para dar de alta un producto (`NewProduct`). */
@Serializable
private data class NuevoProducto(
    val name: String,
    /** Decimal como texto: es como viaja la plata en toda esta API. */
    val price: String,
    val stock: Int,
    /** Atributos del pack de rubro (`product.attrs`). Ausente = sin atributos. */
    val attrs: JsonObject? = null,
)

/** `UpdateProduct` parcial: sólo lo que la pantalla de catálogo edita. */
@Serializable
private data class CambioDeProducto(
    val name: String? = null,
    val price: String? = null,
    val attrs: JsonObject? = null,
)

/** `UpdateProduct` con un solo campo: reasignar el EAN. */
@Serializable
private data class CambioDeCodigo(val barcode: String)

/** `StockAdjust` con `set`: deja el stock **en** ese número, no le suma. */
@Serializable
private data class AjusteDeStock(val set: Int, val reason: String? = null)

/**
 * Da de alta el producto.
 *
 * Nace con [stock] unidades -una, la que la cajera tiene en la mano- porque con
 * cero el server rechaza la venta por stock insuficiente y crear el producto no
 * habría servido de nada.
 *
 * [unidad] es el valor del `AttrField` «unidad» que declara el pack ("Se vende
 * por": kg, atado, bolsa…). Va adentro de `attrs` con la **clave del pack**, no
 * con una constante de la app: el server ya dice cómo se llama el campo.
 */
suspend fun crearProducto(
    api: ApiFactory,
    nombre: String,
    precio: String,
    stock: Int = 1,
    unidad: String? = null,
    claveDeUnidad: String = CLAVE_UNIDAD,
    /** Atributos que no son del pack —una marca interna, por ejemplo—. */
    extras: JsonObject? = null,
): Resultado<ProductDto> = llamar(api) {
    api.http.post("${api.baseUrl}/api/v1/products") {
        contentType(ContentType.Application.Json)
        setBody(
            NuevoProducto(
                name = nombre,
                price = precio,
                stock = stock,
                attrs = atributosCon(extras, claveDeUnidad, unidad),
            ),
        )
    }.exigirExito(api.baseUrl).body()
}

/**
 * Deja el stock de un producto **en** [unidades].
 *
 * `set` y no `delta` a propósito: es una corrección hacia un número conocido, y
 * repetirla dos veces por un reintento tiene que dar lo mismo. Un `delta` sumaría
 * dos veces.
 *
 * Pide rol admin+ (`crates/api/src/v1/catalog.rs`), así que una cajera puede
 * recibir 403. Quien llame tiene que poder seguir sin esto.
 */
suspend fun fijarStock(
    api: ApiFactory,
    productoId: String,
    unidades: Int,
    motivo: String,
): Resultado<ProductDto> = llamar(api) {
    api.http.post("${api.baseUrl}/api/v1/products/$productoId/stock") {
        contentType(ContentType.Application.Json)
        setBody(AjusteDeStock(set = unidades, reason = motivo))
    }.exigirExito(api.baseUrl).body()
}

/**
 * Edita nombre, precio y unidad de un producto que ya existe.
 *
 * `UpdateProduct.attrs` **reemplaza el objeto entero**, no hace merge, así que
 * se manda lo que ya tenía el producto con la unidad encima. [previos] es el
 * `attrs` que vino en la lectura; sin él se perderían los atributos que esta
 * pantalla no muestra (`precio_ref` en feria, `talla`/`color` en tienda).
 */
suspend fun editarProducto(
    api: ApiFactory,
    productoId: String,
    nombre: String,
    precio: String,
    unidad: String? = null,
    previos: JsonObject? = null,
    claveDeUnidad: String = CLAVE_UNIDAD,
): Resultado<ProductDto> = llamar(api) {
    api.http.patch("${api.baseUrl}/api/v1/products/$productoId") {
        contentType(ContentType.Application.Json)
        setBody(
            CambioDeProducto(
                name = nombre,
                price = precio,
                attrs = atributosCon(previos, claveDeUnidad, unidad),
            ),
        )
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

/**
 * La clave por defecto del atributo de unidad.
 *
 * Es la que declara `domain::rubro` para el pack `feria`. Se puede pisar por
 * parámetro porque **el que manda es el pack**: si un rubro llama distinto a su
 * campo, la pantalla lee la clave de ahí y esta constante nunca se usa.
 */
const val CLAVE_UNIDAD: String = "unidad"

/**
 * Los atributos a mandar: los que ya tenía, con la unidad encima.
 *
 * Devuelve `null` -o sea, `attrs` ausente en el JSON- cuando no hay nada que
 * escribir, para no pisar con un objeto vacío lo que el producto ya tenía.
 * Una unidad en blanco **borra** esa clave: es cómo se saca un "se vende por"
 * que se puso por error.
 */
private fun atributosCon(
    previos: JsonObject?,
    clave: String,
    unidad: String?,
): JsonObject? {
    if (unidad == null && previos == null) return null
    val mapa = previos.orEmpty().toMutableMap()
    when {
        unidad == null -> Unit
        unidad.isBlank() -> mapa.remove(clave)
        else -> mapa[clave] = JsonPrimitive(unidad.trim())
    }
    return if (mapa.isEmpty()) null else JsonObject(mapa)
}
