package cl.rutbusiness.app.ui.cobrar

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy del paso Escanear (leer código de barras).
 *
 * Puro JVM: si alguien reescribe las etiquetas inline en [PasoEscaner] y se
 * olvida de feria, o cuela una palabra técnica ("SKU", "buffer", "endpoint",
 * "reintentar la petición"), este test lo atrapa.
 */
class CopyEscanerTest {

    @Test
    fun `titulo cambia solo al dar de alta`() {
        assertEquals("Leer código", copyTituloEscaner(creandoProducto = false))
        assertEquals("Producto nuevo", copyTituloEscaner(creandoProducto = true))
    }

    @Test
    fun `subtitulo al dar de alta habla de lo que vende la duena en feria`() {
        val feria = copySubtituloEscaner(feria = true, creandoProducto = true, hayCamara = true)
        assertEquals("No estaba en lo que vendes", feria)
        assertFalse(feria!!.contains("catálogo", ignoreCase = true))

        val retail = copySubtituloEscaner(feria = false, creandoProducto = true, hayCamara = true)
        assertEquals("No estaba en tu catálogo", retail)
    }

    @Test
    fun `subtitulo con camara y sin dar de alta indica donde apoyar el codigo`() {
        assertEquals(
            "Acerca el código a la luz",
            copySubtituloEscaner(feria = true, creandoProducto = false, hayCamara = true),
        )
    }

    @Test
    fun `subtitulo es nulo sin camara y sin dar de alta`() {
        assertNull(copySubtituloEscaner(feria = true, creandoProducto = false, hayCamara = false))
    }

    @Test
    fun `cartel de espera dice que hacer sin jerga de camara`() {
        val titulo = copyTituloEsperando()
        val detalle = copyDetalleEsperando()
        assertEquals("Acerca el código de barras", titulo)
        assertTrue(detalle.contains("linterna", ignoreCase = true))
        listOf(titulo, detalle).forEach(::assertSinJergaTecnica)
    }

    @Test
    fun `cartel sin producto ofrece crear o escribir a mano y no asusta`() {
        val tituloFeria = copyTituloSinProducto(feria = true)
        assertEquals("Ese código no está en lo que vendes", tituloFeria)
        assertFalse(tituloFeria.contains("catálogo", ignoreCase = true))

        val tituloRetail = copyTituloSinProducto(feria = false)
        assertEquals("Ese código no está en tu catálogo", tituloRetail)

        val detalle = copyDetalleSinProducto("7801234567890")
        assertTrue(detalle.contains("7801234567890"))
        assertTrue(detalle.contains("escribirlo a mano", ignoreCase = true))
    }

    @Test
    fun `cartel de listo confirma el nombre`() {
        assertEquals("Listo: Fideos 400g", copyTituloListo("Fideos 400g"))
    }

    @Test
    fun `detalle de listo feria evita la palabra carrito`() {
        val unaUnidad = copyDetalleListo(feria = true, enCarrito = 1)
        assertEquals("1 unidad en lo que llevas", unaUnidad)
        assertFalse(unaUnidad.contains("carrito", ignoreCase = true))

        val variasUnidades = copyDetalleListo(feria = true, enCarrito = 3)
        assertEquals("3 unidades en lo que llevas", variasUnidades)
    }

    @Test
    fun `detalle de listo retail conserva la palabra carrito`() {
        assertEquals("1 unidad en el carrito", copyDetalleListo(feria = false, enCarrito = 1))
        assertEquals("2 unidades en el carrito", copyDetalleListo(feria = false, enCarrito = 2))
    }

    @Test
    fun `la salida a mano siempre dice lo mismo y esta siempre disponible`() {
        assertEquals("Escribir a mano", copyBotonEscribirAMano())
    }

    @Test
    fun `botones del panel de lectura son cortos y de mesa`() {
        assertEquals("Crear producto", copyBotonCrearProducto())
        assertEquals("Listo", copyBotonListo())
    }

    @Test
    fun `formulario nuevo pide papel en feria y boleta en retail`() {
        val feria = copyPlaceholderNombreNuevo(feria = true)
        assertTrue(feria.contains("papel", ignoreCase = true))
        assertFalse(feria.contains("boleta", ignoreCase = true))

        val retail = copyPlaceholderNombreNuevo(feria = false)
        assertTrue(retail.contains("boleta", ignoreCase = true))
    }

    @Test
    fun `ayuda de precio sin escribir habla de la clienta en feria`() {
        val feria = copyAyudaPrecioNuevo(feria = true, cobrado = null)
        assertEquals("Lo que le cobras a la clienta.", feria)
        assertFalse(feria.contains("público", ignoreCase = true))

        val retail = copyAyudaPrecioNuevo(feria = false, cobrado = null)
        assertEquals("El precio de venta al público.", retail)
    }

    @Test
    fun `ayuda de precio con algo escrito confirma cuanto se cobra en ambos packs`() {
        assertEquals("Se cobra $2.000", copyAyudaPrecioNuevo(feria = true, cobrado = "$2.000"))
        assertEquals("Se cobra $2.000", copyAyudaPrecioNuevo(feria = false, cobrado = "$2.000"))
    }

    @Test
    fun `nota final feria evita la palabra stock`() {
        val feria = copyNotaProductoNuevo(feria = true)
        assertFalse(feria.contains("stock", ignoreCase = true))
        assertTrue(feria.contains("una unidad", ignoreCase = true))
        assertTrue(feria.contains("agente", ignoreCase = true))

        val retail = copyNotaProductoNuevo(feria = false)
        assertTrue(retail.contains("stock", ignoreCase = true))
    }

    @Test
    fun `boton de crear y agregar cambia mientras guarda`() {
        assertEquals("Crear y agregar", copyBotonCrearYAgregar(guardando = false))
        assertEquals("Creando…", copyBotonCrearYAgregar(guardando = true))
    }

    @Test
    fun `ninguna frase del paso escaner usa jerga tecnica`() {
        val frases = listOf(
            copyTituloEscaner(creandoProducto = false),
            copyTituloEscaner(creandoProducto = true),
            copySubtituloEscaner(feria = true, creandoProducto = true, hayCamara = true),
            copySubtituloEscaner(feria = false, creandoProducto = false, hayCamara = true),
            copyTituloEsperando(),
            copyDetalleEsperando(),
            copyTituloSinProducto(feria = true),
            copyTituloSinProducto(feria = false),
            copyDetalleSinProducto("123"),
            copyTituloListo("Pan"),
            copyDetalleListo(feria = true, enCarrito = 1),
            copyDetalleListo(feria = false, enCarrito = 1),
            copyBotonCrearProducto(),
            copyBotonListo(),
            copyBotonEscribirAMano(),
            copyEtiquetaCodigoLeido(),
            copyEtiquetaNombreNuevo(),
            copyPlaceholderNombreNuevo(feria = true),
            copyPlaceholderNombreNuevo(feria = false),
            copyEtiquetaPrecioNuevo(),
            copyPlaceholderPrecioNuevo(),
            copyAyudaPrecioNuevo(feria = true, cobrado = null),
            copyAyudaPrecioNuevo(feria = false, cobrado = null),
            copyNotaProductoNuevo(feria = true),
            copyNotaProductoNuevo(feria = false),
            copyBotonCrearYAgregar(guardando = false),
            copyBotonCrearYAgregar(guardando = true),
        )
        frases.filterNotNull().forEach(::assertSinJergaTecnica)
    }

    /** Palabras de log/desarrollador que nunca le tocan la pantalla a la dueña. */
    private fun assertSinJergaTecnica(frase: String) {
        val jerga = listOf(
            "sku", "buffer", "driver", "endpoint", "socket", "petición",
            "request", "response", "null", "timeout", "backend", "api",
        )
        jerga.forEach { palabra ->
            assertFalse(
                "\"$frase\" no debería contener la palabra técnica \"$palabra\"",
                frase.contains(palabra, ignoreCase = true),
            )
        }
    }
}
