package cl.rutbusiness.app.ui.cobrar

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy + escáner por pack (ADR-0022).
 *
 * Puro JVM: no monta Compose ni cámara. Si alguien reescribe las etiquetas
 * inline en [PasoBuscar] y se olvida de feria, este test lo atrapa.
 */
class CopyCobrarFeriaTest {

    @Test
    fun `feria sin barcode habla de nombre no de EAN`() {
        val c = copyBuscarCobrar(barcode = false)
        assertEquals("¿Qué vendiste?", c.etiqueta)
        assertTrue(
            c.placeholder.lowercase().contains("tomate") ||
                c.placeholder.lowercase().contains("cilantro"),
        )
        assertFalse(c.ayudaOnline.contains("código de barras", ignoreCase = true))
        assertTrue(c.ayudaOnline.contains("nombre", ignoreCase = true))
    }

    @Test
    fun `retail con barcode menciona codigo de barras`() {
        val c = copyBuscarCobrar(barcode = true)
        assertEquals("Buscar producto", c.etiqueta)
        assertTrue(c.placeholder.contains("código de barras", ignoreCase = true))
        assertTrue(c.ayudaOnline.contains("código de barras", ignoreCase = true))
    }

    @Test
    fun `escaner solo si pack y hardware`() {
        assertFalse(escanerVisible(barcode = false, hayCamara = true))
        assertFalse(escanerVisible(barcode = true, hayCamara = false))
        assertFalse(escanerVisible(barcode = false, hayCamara = false))
        assertTrue(escanerVisible(barcode = true, hayCamara = true))
    }

    @Test
    fun `feria miss enseña venderle al agente con precio`() {
        val pista = pistaBusquedaVacia(
            feria = true,
            consulta = " tomates ",
            puedeCargar = true,
        )
        assertTrue(
            "debe enseñar la frase con precio: era \"$pista\"",
            pista.contains("a 2000"),
        )
        assertTrue(pista.contains("vendí tomates a 2000"))
        assertTrue(pista.contains("Agregar una cosa"))
        assertFalse(
            "feria no habla de computador: era \"$pista\"",
            pista.contains("computador", ignoreCase = true),
        )
    }

    @Test
    fun `feria miss sin cargar solo enseña al agente`() {
        val pista = pistaBusquedaVacia(
            feria = true,
            consulta = "cilantro",
            puedeCargar = false,
        )
        assertTrue(pista.contains("a 2000"))
        assertTrue(pista.contains("vendí cilantro a 2000"))
        assertFalse(pista.contains("Agregar una cosa"))
        assertFalse(pista.contains("computador", ignoreCase = true))
    }

    @Test
    fun `pharmacy miss conserva el copy retail exacto`() {
        assertEquals(
            "Revisa cómo se escribe, prueba con una palabra más corta, " +
                "o agrégalo si todavía no está cargado.",
            pistaBusquedaVacia(
                feria = false,
                consulta = "ibuprofeno",
                puedeCargar = true,
            ),
        )
        assertEquals(
            "Revisa cómo se escribe, o prueba con una palabra más corta.",
            pistaBusquedaVacia(
                feria = false,
                consulta = "ibuprofeno",
                puedeCargar = false,
            ),
        )
    }

    @Test
    fun `barra feria habla de cosas no de productos`() {
        assertEquals("Nada en la venta todavía", copyBarraCarrito(0, null, feria = true))
        assertEquals(
            "2 cosas · $3.000",
            copyBarraCarrito(2, "\$3.000", feria = true),
        )
        assertFalse(
            copyBarraCarrito(3, "\$1", feria = true).contains("producto", ignoreCase = true),
        )
    }

    @Test
    fun `barra retail conserva productos`() {
        assertEquals("Sin productos todavía", copyBarraCarrito(0, null, feria = false))
        assertEquals(
            "3 productos · $4.470",
            copyBarraCarrito(3, "\$4.470", feria = false),
        )
    }

    @Test
    fun `offline feria no nombra computador ni sistema del negocio`() {
        val s = copyOfflinePago(feria = true)
        assertFalse(s.contains("computador", ignoreCase = true))
        assertFalse(s.contains("sistema del negocio", ignoreCase = true))
        assertTrue(s.contains("señal", ignoreCase = true) || s.contains("red", ignoreCase = true))
    }

    @Test
    fun `offline retail puede nombrar el sistema del negocio`() {
        val s = copyOfflinePago(feria = false)
        assertTrue(s.contains("sistema del negocio", ignoreCase = true))
    }

    @Test
    fun `subtitulo feria es de mesa`() {
        assertEquals("Anotá lo que se lleva", copySubtituloBuscar(feria = true))
        assertEquals("Busca el producto y agrégalo", copySubtituloBuscar(feria = false))
    }
}
