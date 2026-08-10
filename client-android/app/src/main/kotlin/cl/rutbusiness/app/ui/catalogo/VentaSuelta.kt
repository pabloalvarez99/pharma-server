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
 * **Lo que esto es: un rodeo, no la solución.** La solución de verdad es que
 * `NewProduct` acepte `physical_stock: false`, como ya existe en la base
 * (migración 0031) y usa el seed de servicios: un producto que no descuenta
 * inventario nunca choca contra el chequeo de stock y el centinela deja de
 * necesitar mantenimiento. Hoy no hay forma de pedirlo por la API pública —
 * `catalog::repo::set_physical_stock` sólo lo llama el seed— y ese archivo está
 * en el carril de otro agente. Queda anotado en el digest.
 *
 * Mientras tanto: el centinela nace con [STOCK_INICIAL] unidades y se repone
 * cuando baja de [STOCK_MINIMO]. A cien ventas sueltas por día eso da años, y la
 * reposición es una llamada más que además puede fallar sin romper la venta.
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

/** Con cuánto stock nace. */
private const val STOCK_INICIAL = 100_000

/** Cuándo se repone. */
private const val STOCK_MINIMO = 1_000

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
 * vender sin señal—. Se mira **antes** que la red: si el centinela ya se usó
 * alguna vez, está ahí, y entonces cobrar un monto suelto funciona sin conexión
 * como cualquier otra venta. Quien llame sin señal y sin nada en el teléfono
 * tiene que decirlo con sus palabras antes de llegar acá: desde acá saldría un
 * error de red genérico, y el problema no es la red sino que todavía no se usó
 * nunca.
 */
suspend fun asegurarVentaSuelta(
    api: ApiFactory,
    enElTelefono: List<ProductDto> = emptyList(),
): Resultado<ProductDto> {
    ventaSueltaEn(enElTelefono)?.let { return Resultado.Ok(it) }

    return when (val encontrado = ProductRepository(api).buscar(NOMBRE_VENTA_SUELTA, CANDIDATOS)) {
        is Resultado.Falla -> encontrado

        is Resultado.Ok -> {
            val yaEsta = encontrado.valor.firstOrNull(::esVentaSuelta)
            if (yaEsta != null) {
                // Reponer es opcional: si la cajera no es admin el server
                // contesta 403 y la venta tiene que salir igual.
                if (yaEsta.stock < STOCK_MINIMO) {
                    fijarStock(api, yaEsta.id, STOCK_INICIAL, "venta suelta: reposición")
                }
                Resultado.Ok(yaEsta)
            } else {
                crearProducto(
                    api = api,
                    nombre = NOMBRE_VENTA_SUELTA,
                    // El precio del catálogo no se usa nunca: cada línea lleva el
                    // monto que se dijo. Cero es lo honesto para "no tiene precio".
                    precio = "0",
                    stock = STOCK_INICIAL,
                    extras = JsonObject(mapOf(CLAVE_VENTA_SUELTA to JsonPrimitive(true))),
                )
            }
        }
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
