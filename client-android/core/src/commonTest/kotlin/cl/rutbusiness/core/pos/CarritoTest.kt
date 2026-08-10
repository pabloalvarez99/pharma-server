package cl.rutbusiness.core.pos

import cl.rutbusiness.core.api.models.ProductDto
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

private fun producto(
    id: String,
    nombre: String,
    precio: String,
    stock: Long = 10,
) = ProductDto(
    active = true,
    createdAt = "2026-08-06T00:00:00Z",
    id = id,
    name = nombre,
    physicalStock = true,
    prescriptionType = "direct",
    price = precio,
    slug = nombre.lowercase().replace(' ', '-'),
    stock = stock,
    updatedAt = "2026-08-06T00:00:00Z",
)

class CarritoTest {

    @Test
    fun `agregar dos veces el mismo producto sube la cantidad y no duplica la fila`() {
        val arroz = producto("product:a", "Arroz", "1490")
        val carrito = Carrito().agregar(arroz).agregar(arroz)

        assertEquals(1, carrito.items.size)
        assertEquals(2, carrito.items.first().cantidad)
        assertEquals(2, carrito.unidades)
    }

    @Test
    fun `el total usa la misma formula que el server`() {
        // `invariants::order_subtotal` = suma de unit_price * quantity, sin
        // impuesto encima (los precios ya vienen con impuesto incluido).
        val carrito = Carrito()
            .agregar(producto("product:a", "Arroz", "1490"))
            .agregar(producto("product:a", "Arroz", "1490"))
            .agregar(producto("product:b", "Aceite", "2790"))

        assertEquals("5770", assertNotNull(carrito.total).aTextoDeServidor())
    }

    @Test
    fun `suma precios con decimales sin pasar por punto flotante`() {
        val carrito = Carrito()
            .agregar(producto("product:a", "Café", "4.99"))
            .agregar(producto("product:a", "Café", "4.99"))
            .agregar(producto("product:b", "Té", "0.01"))

        assertEquals("9.99", assertNotNull(carrito.total).aTextoDeServidor())
    }

    @Test
    fun `si un precio no se entiende el total es null y la UI no inventa un numero`() {
        val carrito = Carrito()
            .agregar(producto("product:a", "Arroz", "1490"))
            .agregar(producto("product:b", "Roto", "gratis"))

        assertNull(carrito.total)
    }

    @Test
    fun `bajar a cero saca la fila`() {
        val arroz = producto("product:a", "Arroz", "1490")
        val carrito = Carrito().agregar(arroz).cambiarCantidad("product:a", 0)

        assertTrue(carrito.vacio)
        assertEquals("0", assertNotNull(carrito.total).aTextoDeServidor())
    }

    // --- monto suelto ---------------------------------------------------------

    @Test
    fun `el monto suelto viaja como el precio de la linea y no como el del catalogo`() {
        // El centinela vale $0 en el catálogo a propósito: el precio de la venta
        // es el que dijo la dueña, y va en la línea.
        val centinela = producto("product:suelto", "Venta suelta", "0", stock = 100_000)
        val carrito = Carrito().conMontoSuelto(centinela, "2000")

        val lineas = carrito.aLineasDeVenta()
        assertEquals(1, lineas.size)
        assertEquals("product:suelto", lineas[0].product)
        assertEquals(1, lineas[0].quantity)
        assertEquals("2000", lineas[0].unitPrice)
        assertEquals("2000", assertNotNull(carrito.total).aTextoDeServidor())
    }

    /**
     * Un segundo monto **corrige** el primero.
     *
     * Dos líneas con el mismo producto el server las rechaza —carga los
     * productos del pedido y compara la cantidad—, así que si esto duplicara la
     * fila la venta entera fallaría con un 404 en la cara del cliente.
     */
    @Test
    fun `escribir otro monto corrige el anterior en vez de duplicar la linea`() {
        val centinela = producto("product:suelto", "Venta suelta", "0", stock = 100_000)
        val carrito = Carrito()
            .conMontoSuelto(centinela, "2000")
            .conMontoSuelto(centinela, "2500")

        assertEquals(1, carrito.items.size)
        assertEquals(1, carrito.items.first().cantidad)
        assertEquals("2500", carrito.items.first().precioUnitario)
        assertEquals("2500", assertNotNull(carrito.total).aTextoDeServidor())
    }

    @Test
    fun `el monto suelto convive con los productos del carrito`() {
        val centinela = producto("product:suelto", "Venta suelta", "0", stock = 100_000)
        val carrito = Carrito()
            .agregar(producto("product:a", "Arroz", "1490"))
            .conMontoSuelto(centinela, "2000")

        assertEquals(2, carrito.items.size)
        assertEquals("3490", assertNotNull(carrito.total).aTextoDeServidor())
    }

    /** Subir la cantidad multiplica igual que en cualquier otra línea. */
    @Test
    fun `dos bolsas al mismo monto se cobran al doble`() {
        val centinela = producto("product:suelto", "Venta suelta", "0", stock = 100_000)
        val carrito = Carrito()
            .conMontoSuelto(centinela, "2000")
            .cambiarCantidad("product:suelto", 2)

        assertEquals("4000", assertNotNull(carrito.total).aTextoDeServidor())
    }

    @Test
    fun `las lineas de venta llevan el precio tal como vino del server`() {
        val carrito = Carrito()
            .agregar(producto("product:a", "Arroz", "1490"))
            .cambiarCantidad("product:a", 3)

        val lineas = carrito.aLineasDeVenta()
        assertEquals(1, lineas.size)
        assertEquals("product:a", lineas[0].product)
        assertEquals("Arroz", lineas[0].productName)
        assertEquals(3, lineas[0].quantity)
        assertEquals("1490", lineas[0].unitPrice)
    }
}

class ClaveDeIdempotenciaTest {

    @Test
    fun `cada clave es distinta y tiene largo fijo`() {
        val claves = (1..500).map { PosRepository.nuevaClave() }
        assertEquals(500, claves.toSet().size)
        assertTrue(claves.all { it.length == 32 })
        assertTrue(claves.all { clave -> clave.all { it in "0123456789abcdef" } })
    }
}
