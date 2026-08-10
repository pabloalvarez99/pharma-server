package cl.rutbusiness.core.pos

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.money.Dinero

data class ItemCarrito(
    val productoId: String,
    val nombre: String,
    /**
     * El precio tal como lo mandó el server.
     *
     * Con una excepción, y sólo una: la línea de **monto suelto**, donde el
     * precio es el que la dueña dijo en voz alta ("son $2.000") y no existe en
     * ningún catálogo. Se escribe una vez, al armar la línea, y desde ahí es
     * igual de inmutable que los otros. Ver [Carrito.conMontoSuelto].
     */
    val precioUnitario: String,
    val cantidad: Int,
    /** Stock que el server informó al momento de agregarlo. */
    val stockConocido: Long,
    /** `false` para servicios: no descuentan inventario y no tienen tope. */
    val descuentaStock: Boolean,
) {
    val subtotal: Dinero?
        get() = Dinero.deTextoDeServidor(precioUnitario)?.times(cantidad)
}

/**
 * El carrito del mostrador.
 *
 * Inmutable a propósito: cada operación devuelve uno nuevo. Así el estado de
 * Compose cambia de identidad y la lista se recompone sola, sin un observable
 * mutable que haya que acordarse de notificar.
 */
data class Carrito(val items: List<ItemCarrito> = emptyList()) {

    val vacio: Boolean get() = items.isEmpty()

    /** Cuántas unidades hay en total, para el contador del botón de cobrar. */
    val unidades: Int get() = items.sumOf { it.cantidad }

    /**
     * Total en pantalla mientras se arma la venta.
     *
     * **Esto es una vista previa, no el cobro.** Lo que se cobra es el `total`
     * que devuelve `POST /pos/sale`, y el comprobante muestra ese, no éste.
     * Existe porque el mostrador necesita ver cuánto va sumando y el server no
     * tiene endpoint de cotización — no hay un `POST /pos/quote` al que
     * preguntarle. Ver la nota de arriba de [Dinero].
     *
     * Aplica la misma fórmula que `invariants::order_subtotal` en el dominio
     * (suma de `unit_price * quantity`, sin impuesto agregado encima porque los
     * precios ya vienen con impuesto incluido), con aritmética entera sobre los
     * mismos decimales que mandó el server. Si algún precio no se pudo leer,
     * devuelve `null` y la UI dice "lo confirma el sistema al cobrar" en vez de
     * mostrar un número inventado.
     */
    val total: Dinero?
        get() {
            if (items.isEmpty()) return Dinero.CERO
            var acumulado = Dinero.CERO
            for (item in items) {
                val subtotal = item.subtotal ?: return null
                acumulado += subtotal
            }
            return acumulado
        }

    /** Suma uno. Si el producto ya está, sube la cantidad en vez de duplicar la fila. */
    fun agregar(producto: ProductDto): Carrito {
        val existente = items.indexOfFirst { it.productoId == producto.id }
        if (existente >= 0) return cambiarCantidad(producto.id, items[existente].cantidad + 1)

        return copy(
            items = items + ItemCarrito(
                productoId = producto.id,
                nombre = producto.name,
                precioUnitario = producto.price,
                cantidad = 1,
                stockConocido = producto.stock,
                descuentaStock = producto.physicalStock,
            ),
        )
    }

    /**
     * Deja **una** línea con un monto dicho a mano.
     *
     * Es la venta más común del puesto: "son $2.000, una bolsa". No hay producto
     * que buscar porque no hay nada que cargar — el monto es toda la venta.
     *
     * [producto] es el centinela con el que esa línea viaja al server, que exige
     * un producto por línea (`PosSaleItem.product` es obligatorio: "POS does not
     * allow free items"). El monto va en `unitPrice`, que es lo que el server
     * usa para el subtotal.
     *
     * **Reemplaza en vez de sumar, y eso es deliberado.** Dos líneas con el
     * mismo producto el server las rechaza -carga los productos del pedido y
     * compara `loaded.len()` contra la cantidad de líneas-, así que la única
     * alternativa a reemplazar sería sumar los dos montos en uno. Sumar plata
     * escrita a mano y mandar el resultado como si fuera un precio es la clase
     * de cuenta que en esta app hace el server, no el teléfono; y de paso
     * borraría de la venta que eran dos cosas. Quien escribe un segundo monto
     * está corrigiendo el primero, y eso es lo que pasa.
     *
     * La cantidad no se toca: subirla con el "+" del paso de pago es legítimo
     * -dos bolsas al mismo precio- y multiplica igual que cualquier otra línea.
     */
    fun conMontoSuelto(producto: ProductDto, monto: String): Carrito {
        val linea = ItemCarrito(
            productoId = producto.id,
            nombre = producto.name,
            precioUnitario = monto,
            cantidad = 1,
            stockConocido = producto.stock,
            descuentaStock = producto.physicalStock,
        )
        val donde = items.indexOfFirst { it.productoId == producto.id }
        if (donde < 0) return copy(items = items + linea)
        return copy(items = items.toMutableList().also { it[donde] = linea })
    }

    /** Cambia la cantidad. Cero o menos saca la fila, que es lo que espera quien toca "−". */
    fun cambiarCantidad(productoId: String, cantidad: Int): Carrito {
        if (cantidad <= 0) return quitar(productoId)
        return copy(
            items = items.map {
                if (it.productoId == productoId) it.copy(cantidad = cantidad) else it
            },
        )
    }

    fun quitar(productoId: String): Carrito =
        copy(items = items.filterNot { it.productoId == productoId })

    fun aLineasDeVenta(): List<LineaDeVenta> = items.map {
        LineaDeVenta(
            product = it.productoId,
            productName = it.nombre,
            quantity = it.cantidad,
            unitPrice = it.precioUnitario,
        )
    }
}
