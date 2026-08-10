package cl.rutbusiness.app.ui.catalogo

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.catalog.ProductRepository
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull

/**
 * Cobrar un monto sin tener nada cargado.
 *
 * **El caso.** "Son $2.000, una bolsa." Es la venta más común del puesto y no
 * tiene producto detrás: el monto es toda la venta. Hasta hoy la app la hacía
 * imposible — para cobrar había que buscar un producto, y para tener un producto
 * había que cargarlo, y el único camino para cargarlo pasaba por el escáner.
 *
 * **Por qué hay un producto igual.** El server no acepta líneas libres:
 * `PosSaleItem.product` es obligatorio y su propio comentario lo dice ("POS does
 * not allow free items"). Así que la venta suelta viaja como una línea contra un
 * producto centinela, uno solo por negocio, con el monto escrito en `unit_price`
 * — que es de donde el server saca el subtotal (`sales::service`, suma de
 * `unit_price * quantity`).
 *
 * **El centinela no tiene inventario.** Nace `physical_stock = false` y
 * `stock = 0` (`NewProduct.physical_stock`, migración 0031). Eso no es un
 * detalle de ahorro: un ítem sin inventario no choca nunca contra el chequeo de
 * stock, así que **su stock no se mueve jamás** y no hay nada que reponer. Antes
 * nacía con un colchón de 100.000 unidades que cada venta bajaba en uno, y había
 * que rellenarlo cada tanto con una llamada que además podía dar 403 si quien
 * estaba en la caja no era admin.
 *
 * Y sobre todo: en cero **sin** el flag, el centinela aparecería como "sin
 * stock" en las alertas y en el tablero — un producto que la dueña no cargó,
 * gritando un problema que no existe. El colchón lo tapaba por accidente, no por
 * diseño.
 */

/** Cómo se llama el centinela. Se ve en el comprobante, así que se lee. */
const val NOMBRE_VENTA_SUELTA: String = "Venta suelta"

/**
 * La marca en `attrs` que lo identifica.
 *
 * Se busca por esto y no sólo por el nombre porque el nombre lo puede cambiar
 * cualquiera desde "Lo que vendo" o desde el ERP de escritorio, y el día que
 * alguien lo renombre la app crearía un segundo centinela sin darse cuenta.
 */
const val CLAVE_VENTA_SUELTA: String = "rb_venta_suelta"

/** Cuántos candidatos se miran al buscarlo por nombre. */
private const val CANDIDATOS = 10

/** `true` si este producto es el centinela de los montos sueltos. */
fun esVentaSuelta(producto: ProductDto): Boolean {
    val marca = (producto.attrs as? JsonObject)?.get(CLAVE_VENTA_SUELTA)
    if (marca is JsonPrimitive && marca.booleanOrNull == true) return true
    // El nombre es el plan B para un centinela creado antes de que existiera la
    // marca, o por otro cliente.
    return producto.name.trim().equals(NOMBRE_VENTA_SUELTA, ignoreCase = true)
}

/** El centinela dentro de una lista ya traída, si está. */
fun ventaSueltaEn(productos: List<ProductDto>): ProductDto? =
    productos.firstOrNull(::esVentaSuelta)

/**
 * El centinela, buscándolo primero y creándolo si no está.
 *
 * [enElTelefono] es el catálogo que ya está en memoria —el que Cobrar guarda para
 * vender sin señal—. Se mira **antes** que la red: si el centinela ya existe,
 * está ahí, y entonces cobrar un monto suelto funciona sin conexión como
 * cualquier otra venta.
 *
 * Crear pide señal, pero eso pasa **una sola vez en la vida del negocio**, así
 * que conviene que pase cuando hay señal y no cuando hace falta cobrar. De eso
 * se encarga quien llame: `CobrarViewModel` lo pide junto con el refresco del
 * catálogo, que corre al abrir la pantalla y con conexión. Quien llegue acá sin
 * señal y sin nada en el teléfono tiene que decirlo con sus palabras antes:
 * desde acá saldría un error de red genérico, y el problema no es la red sino
 * que todavía no se usó nunca.
 */
suspend fun asegurarVentaSuelta(
    api: ApiFactory,
    enElTelefono: List<ProductDto> = emptyList(),
): Resultado<ProductDto> {
    ventaSueltaEn(enElTelefono)?.let { return Resultado.Ok(it) }

    return when (val encontrado = ProductRepository(api).buscar(NOMBRE_VENTA_SUELTA, CANDIDATOS)) {
        is Resultado.Falla -> encontrado

        is Resultado.Ok -> encontrado.valor.firstOrNull(::esVentaSuelta)?.let { Resultado.Ok(it) }
            ?: crearProducto(
                api = api,
                nombre = NOMBRE_VENTA_SUELTA,
                // El precio del catálogo no se usa nunca: cada línea lleva el
                // monto que se dijo. Cero es lo honesto para "no tiene precio".
                precio = "0",
                stock = 0,
                sinInventario = true,
                extras = JsonObject(mapOf(CLAVE_VENTA_SUELTA to JsonPrimitive(true))),
            )
    }
}

/**
 * Saca el centinela de una lista para mostrarla.
 *
 * No es un producto: es plomería. En "Lo que vendo" sería una fila que la dueña
 * no cargó y no puede explicar, y en el buscador de Cobrar sería una fila a
 * $0 que se puede agregar por error en medio de una venta.
 */
fun sinVentaSuelta(productos: List<ProductDto>): List<ProductDto> =
    productos.filterNot(::esVentaSuelta)
