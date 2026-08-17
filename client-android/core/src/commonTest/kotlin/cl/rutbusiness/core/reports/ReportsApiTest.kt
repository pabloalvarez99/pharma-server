package cl.rutbusiness.core.reports

import cl.rutbusiness.core.net.ApiFactory
import kotlinx.serialization.decodeFromString
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

/**
 * Deserialización de los cuatro reportes con la forma REAL del server
 * (relevada de `crates/domain/src/expenses/model.rs`), no de memoria.
 *
 * Usa [ApiFactory.JSON] a propósito: es el mismo `Json` con el que la app
 * decodifica de verdad (`ignoreUnknownKeys`, `explicitNulls = false`), así
 * que un test que pasa acá y falla en el teléfono sería una configuración
 * de test mentirosa.
 */
class ReportsApiTest {

    @Test
    fun `top-products deserializa la forma real del server`() {
        val json = """
            [
                {
                    "rank": 1,
                    "product_id": "prod-1",
                    "product_name": "Paracetamol 500mg",
                    "qty_sold": 42,
                    "revenue": "12.50",
                    "revenue_pct": "18.30",
                    "abc_class": "A"
                }
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<TopProductoDto>>(json)

        assertEquals(1, filas.size)
        val fila = filas[0]
        assertEquals(1L, fila.rank)
        assertEquals("prod-1", fila.productId)
        assertEquals("Paracetamol 500mg", fila.productName)
        assertEquals(42L, fila.qtySold)
        assertEquals("12.50", fila.revenue)
        assertEquals("18.30", fila.revenuePct)
        assertEquals("A", fila.abcClass)
    }

    @Test
    fun `top-products con product_id null no revienta`() {
        val json = """
            [
                {
                    "rank": 3,
                    "product_id": null,
                    "product_name": "Ítem libre",
                    "qty_sold": 1,
                    "revenue": "1490",
                    "revenue_pct": "0.50",
                    "abc_class": "C"
                }
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<TopProductoDto>>(json)

        assertNull(filas[0].productId)
        assertEquals("Ítem libre", filas[0].productName)
    }

    @Test
    fun `sales-by-method deserializa method y label por separado`() {
        val json = """
            [
                {"method": "efectivo", "label": "Efectivo", "orders": 5, "amount": "50000"},
                {"method": "fiado", "label": "Fiado", "orders": 2, "amount": "12.50"}
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<VentaPorMetodoDto>>(json)

        assertEquals(2, filas.size)
        assertEquals("efectivo", filas[0].method)
        assertEquals("Efectivo", filas[0].label)
        assertEquals("50000", filas[0].amount)
        assertEquals("fiado", filas[1].method)
        assertEquals("12.50", filas[1].amount)
    }

    @Test
    fun `margins-daily deserializa items_without_cost`() {
        val json = """
            [
                {
                    "date": "2026-08-17",
                    "revenue": "100000",
                    "cost": "40000",
                    "margin": "60000",
                    "margin_pct": "60.00",
                    "items_without_cost": 3
                }
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<MargenDiarioDto>>(json)

        assertEquals("2026-08-17", filas[0].date)
        assertEquals("60.00", filas[0].marginPct)
        assertEquals(3L, filas[0].itemsWithoutCost)
    }

    @Test
    fun `stock-rotation con turnover y days_of_inventory null no revienta`() {
        val json = """
            [
                {
                    "product_id": "prod-2",
                    "product_name": "Jarabe X",
                    "qty_sold": 10,
                    "current_stock": 0,
                    "turnover": null,
                    "days_of_inventory": null
                }
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<RotacionStockDto>>(json)

        assertEquals(10L, filas[0].qtySold)
        assertNull(filas[0].turnover)
        assertNull(filas[0].daysOfInventory)
    }

    @Test
    fun `stock-rotation con turnover presente lo trae como texto`() {
        val json = """
            [
                {
                    "product_id": "prod-3",
                    "product_name": "Jarabe Y",
                    "qty_sold": 20,
                    "current_stock": 5,
                    "turnover": "4.00",
                    "days_of_inventory": "7.50"
                }
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<RotacionStockDto>>(json)

        assertEquals("4.00", filas[0].turnover)
        assertEquals("7.50", filas[0].daysOfInventory)
    }

    @Test
    fun `campo desconocido en la respuesta no revienta`() {
        val json = """
            [
                {
                    "method": "tarjeta",
                    "label": "Tarjeta",
                    "orders": 1,
                    "amount": "1000",
                    "installments": 3
                }
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<VentaPorMetodoDto>>(json)

        assertEquals("tarjeta", filas[0].method)
    }

    @Test
    fun `campo ausente cae en el default y no lanza`() {
        val json = """[{}]"""

        val filas = ApiFactory.JSON.decodeFromString<List<TopProductoDto>>(json)

        assertEquals(0L, filas[0].rank)
        assertNull(filas[0].productId)
        assertEquals("", filas[0].productName)
        assertEquals("0", filas[0].revenue)
        assertEquals("", filas[0].abcClass)
    }

    @Test
    fun `monto con decimales y monto entero sobreviven el round-trip como texto identico`() {
        val json = """
            [
                {"date": "2026-08-17", "revenue": "12.50", "cost": "0", "margin": "12.50", "margin_pct": "100.00", "items_without_cost": 0},
                {"date": "2026-08-16", "revenue": "1490", "cost": "0", "margin": "1490", "margin_pct": "100.00", "items_without_cost": 0}
            ]
        """.trimIndent()

        val filas = ApiFactory.JSON.decodeFromString<List<MargenDiarioDto>>(json)

        assertEquals("12.50", filas[0].revenue)
        assertEquals("1490", filas[1].revenue)
    }
}
