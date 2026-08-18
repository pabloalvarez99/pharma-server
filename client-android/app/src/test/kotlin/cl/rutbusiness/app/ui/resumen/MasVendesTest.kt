package cl.rutbusiness.app.ui.resumen

import cl.rutbusiness.core.reports.TopProductoDto
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Geometría y orden de "Lo que más vendes", sin Compose ni server detrás.
 *
 * Regla de plata: `fraccionDeBarra` es la única conversión de un texto de
 * servidor a `Float` que este bloque hace, y es para píxeles — nunca se
 * muestra ese número como cifra. Ver KDoc de [MasVendes.kt].
 */
class MasVendesTest {

    @Test
    fun `cero por ciento no dibuja barra`() {
        assertEquals(0f, fraccionDeBarra("0"))
        assertEquals(0f, fraccionDeBarra("0.0"))
    }

    @Test
    fun `cien por ciento llena la barra entera`() {
        assertEquals(1f, fraccionDeBarra("100"))
    }

    @Test
    fun `valores fuera de rango se coercen entre 0 y 1`() {
        assertEquals(1f, fraccionDeBarra("150"))
        assertEquals(0f, fraccionDeBarra("-30"))
    }

    @Test
    fun `texto basura no revienta, cae en cero`() {
        assertEquals(0f, fraccionDeBarra(""))
        assertEquals(0f, fraccionDeBarra("no es un numero"))
    }

    @Test
    fun `un porcentaje chico pero mayor que cero tiene un piso visible`() {
        val fraccion = fraccionDeBarra("0.1")
        assertTrue("una barra visible no puede ser menor a 0.004f", fraccion >= 0.004f)
    }

    @Test
    fun `los nombres quedan en el orden del ranking del server, nunca reordenados`() {
        val productos = listOf(
            TopProductoDto(rank = 1, productName = "Tomates", revenuePct = "40"),
            TopProductoDto(rank = 2, productName = "Cilantro", revenuePct = "35"),
            TopProductoDto(rank = 3, productName = "Perejil", revenuePct = "25"),
        )
        assertEquals(listOf("Tomates", "Cilantro", "Perejil"), nombresOrdenados(productos))
    }

    @Test
    fun `lista vacia no produce descripcion, el bloque no se dibuja`() {
        assertTrue(descripcionMasVendes(emptyList()).isEmpty())
        assertTrue(nombresOrdenados(emptyList()).isEmpty())
    }

    @Test
    fun `la descripcion no contiene ningun monto sumado`() {
        val descripcion = descripcionMasVendes(listOf("Tomates", "Cilantro"))
        assertFalse(descripcion.contains("$"))
        assertFalse(descripcion.any { it.isDigit() })
    }

    /** `product_id` puede venir `null` (ítem libre); el nombre igual está. */
    @Test
    fun `un producto sin product_id igual aparece por su nombre`() {
        val productos = listOf(TopProductoDto(rank = 1, productId = null, productName = "Palta suelta"))
        assertEquals(listOf("Palta suelta"), nombresOrdenados(productos))
    }

    /**
     * El snapshot offline guarda `masVendes` como parte de [ResumenGuardado]:
     * guardar y volver a leer tiene que dejar la misma lista, en el mismo
     * orden, para que el día sin señal muestre exactamente lo que ya se
     * mostró con conexión.
     */
    @Test
    fun `guardar y restaurar el snapshot deja la misma lista de mas vendes`() {
        val guardado = ResumenGuardado(
            ventasHoy = null,
            ventasDeAyer = null,
            comparacion = Comparacion.SinDatoDeAyer.name,
            masVendes = listOf(
                TopProductoDto(rank = 1, productId = "p1", productName = "Tomates", revenue = "10000", revenuePct = "60", abcClass = "A"),
                TopProductoDto(rank = 2, productId = null, productName = "Cilantro suelto", revenue = "5000", revenuePct = "40", abcClass = "B"),
            ),
            enCaja = null,
            nombreDeCaja = null,
            sinCajaAbierta = false,
            porCobrar = null,
            stockBajo = emptyList(),
            cuantosConStockBajo = 0,
            porVencer = emptyList(),
            cuantosPorVencer = 0,
        )

        val json = Json.encodeToString(ResumenGuardado.serializer(), guardado)
        val restaurado = Json.decodeFromString(ResumenGuardado.serializer(), json)

        assertEquals(guardado.masVendes, restaurado.masVendes)
        assertEquals(
            listOf("Tomates", "Cilantro suelto"),
            nombresOrdenados(restaurado.masVendes),
        )
    }
}
