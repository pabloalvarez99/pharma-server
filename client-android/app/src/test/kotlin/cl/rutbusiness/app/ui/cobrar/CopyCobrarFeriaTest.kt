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
    fun `barra feria sin total no dice sistema ni inventa un monto`() {
        val s = copyBarraCarrito(unidades = 2, total = null, feria = true)
        assertEquals("2 cosas · se confirma al cobrar", s)
        assertFalse("feria no dice sistema: era \"$s\"", s.contains("sistema", ignoreCase = true))
        // Unidades "2" es conteo real; no hay cifra inventada de total ($…).
        assertFalse("no inventa total con \$: era \"$s\"", s.contains('$'))
        assertFalse(s.contains("192.168"))
        assertFalse(s.contains("export", ignoreCase = true))
        assertFalse(s.contains("checkout", ignoreCase = true))
    }

    @Test
    fun `barra retail sin total puede nombrar el sistema`() {
        val s = copyBarraCarrito(unidades = 1, total = null, feria = false)
        assertTrue(s.contains("sistema", ignoreCase = true))
        assertTrue(s.contains("1 producto"))
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
    fun `feria bar offline empty sin jerga de sistema ip export checkout`() {
        val bar = copyBarraCarrito(unidades = 3, total = null, feria = true)
        val offline = copyOfflinePago(feria = true)
        val vacio = copyBarraCarrito(unidades = 0, total = null, feria = true)
        val ayuda = copyAyudaCatalogoGuardado(feria = true, antiguedad = "hace un rato")
        for (s in listOf(bar, offline, vacio, ayuda)) {
            assertFalse("no sistema: era \"$s\"", s.contains("sistema", ignoreCase = true))
            assertFalse("no IP: era \"$s\"", s.contains("192.168"))
            assertFalse("no export: era \"$s\"", s.contains("export", ignoreCase = true))
            assertFalse("no checkout: era \"$s\"", s.contains("checkout", ignoreCase = true))
            assertFalse("no computador: era \"$s\"", s.contains("computador", ignoreCase = true))
        }
    }

    @Test
    fun `ayuda catalogo guardado feria no habla de stock de gondola`() {
        val feria = copyAyudaCatalogoGuardado(feria = true, antiguedad = "hace 5 min")
        assertTrue(feria.contains("Guardado hace 5 min"))
        assertFalse(feria.contains("stock", ignoreCase = true))
        val retail = copyAyudaCatalogoGuardado(feria = false, antiguedad = "hace 5 min")
        assertTrue(retail.contains("stock", ignoreCase = true))
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

    @Test
    fun `nota de monto suelto feria habla de puesto no de caja`() {
        val feria = copyNotaMontoSuelto(feria = true)
        assertTrue(feria.contains("puesto", ignoreCase = true))
        assertFalse(feria.contains("caja", ignoreCase = true))
        assertTrue(feria.contains("Venta suelta"))

        val retail = copyNotaMontoSuelto(feria = false)
        assertTrue(retail.contains("caja", ignoreCase = true))
        assertFalse(retail.contains("puesto", ignoreCase = true))
    }

    @Test
    fun `papel sin detalle feria no suena a ticket fiscal del sistema`() {
        val feria = copySinDetalleComprobante(feria = true)
        assertTrue(feria.contains("papel", ignoreCase = true) || feria.contains("Tu día"))
        assertFalse(feria.contains("computador", ignoreCase = true))
        assertFalse(feria.contains("sistema del negocio", ignoreCase = true))
    }

    @Test
    fun `ref del papel es quieta no grita Comprobante`() {
        assertEquals("Nº AB12CD34", copyRefPapel("AB12CD34-resto-largo"))
        assertEquals("Sin número", copyRefPapel("   "))
        assertFalse(copyRefPapel("folio-1").contains("Comprobante", ignoreCase = true))
    }

    @Test
    fun `pie de venta encolada feria no nombra el sistema`() {
        val feria = copyPieVentaEncolada(feria = true, unidades = 2)
        assertTrue(feria.contains("2 unidades"))
        assertTrue(feria.contains("señal", ignoreCase = true))
        assertFalse(feria.contains("sistema", ignoreCase = true))
    }
}
