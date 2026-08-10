package cl.rutbusiness.app.ui.catalogo

import cl.rutbusiness.core.api.models.ProductDto
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Reconocer el centinela de los montos sueltos.
 *
 * Si falla, no falla poco: un centinela que no se reconoce se duplica en cada
 * cobro rápido, y uno que no se filtra aparece como una fila a $0 en "Lo que
 * vendo" y en el buscador de Cobrar, donde se agrega sin querer a una venta.
 */
class VentaSueltaTest {

    @Test
    fun `lo reconoce por la marca de attrs`() {
        val marcado = producto("p1", "Cualquier nombre", marcado = true)
        assertTrue(esVentaSuelta(marcado))
    }

    /**
     * La marca gana, y por eso existe: el nombre lo puede cambiar cualquiera
     * desde "Lo que vendo" o desde el ERP de escritorio, y sin la marca el
     * siguiente cobro rápido crearía un segundo centinela.
     */
    @Test
    fun `lo sigue reconociendo despues de que le cambien el nombre`() {
        val renombrado = producto("p1", "Varios", marcado = true)
        assertEquals(renombrado, ventaSueltaEn(listOf(producto("p0", "Tomate"), renombrado)))
    }

    /** Y por nombre, para uno creado antes de que existiera la marca. */
    @Test
    fun `lo reconoce por el nombre cuando no tiene marca`() {
        assertTrue(esVentaSuelta(producto("p1", NOMBRE_VENTA_SUELTA)))
        assertTrue(esVentaSuelta(producto("p1", "  venta suelta  ")))
    }

    @Test
    fun `un producto normal no es venta suelta`() {
        assertFalse(esVentaSuelta(producto("p1", "Tomate")))
        // Una marca puesta en false tampoco: `attrs` es JSON libre del server.
        assertFalse(esVentaSuelta(producto("p1", "Tomate", marcado = false)))
        assertNull(ventaSueltaEn(listOf(producto("p1", "Tomate"))))
    }

    /**
     * Y no se cae con basura en `attrs`.
     *
     * `attrs` lo escribe cualquier cliente del ERP. Una marca que llegó como
     * texto u objeto no puede tirar la pantalla de catálogo entera.
     */
    @Test
    fun `no se cae si la marca no es un booleano`() {
        val raro = producto("p1", "Tomate").copy(
            attrs = JsonObject(mapOf(CLAVE_VENTA_SUELTA to JsonPrimitive("si"))),
        )
        assertFalse(esVentaSuelta(raro))

        val anidado = producto("p1", "Tomate").copy(
            attrs = JsonObject(
                mapOf(CLAVE_VENTA_SUELTA to JsonObject(mapOf("x" to JsonPrimitive(1)))),
            ),
        )
        assertFalse(esVentaSuelta(anidado))
    }

    @Test
    fun `se saca de las listas que se muestran`() {
        val lista = listOf(
            producto("p0", "Tomate"),
            producto("p1", NOMBRE_VENTA_SUELTA, marcado = true),
            producto("p2", "Cilantro"),
        )

        val visible = sinVentaSuelta(lista)
        assertEquals(listOf("Tomate", "Cilantro"), visible.map { it.name })
    }

    private fun producto(id: String, nombre: String, marcado: Boolean? = null) = ProductDto(
        active = true,
        createdAt = "2026-08-09T00:00:00Z",
        id = id,
        name = nombre,
        physicalStock = true,
        prescriptionType = "none",
        price = "0",
        slug = id,
        stock = 100L,
        updatedAt = "2026-08-09T00:00:00Z",
        attrs = marcado?.let {
            JsonObject(mapOf(CLAVE_VENTA_SUELTA to JsonPrimitive(it)))
        },
    )
}
