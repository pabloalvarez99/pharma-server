package cl.rutbusiness.app.ui.ventas

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Gate de copy de "Lo que vendiste" (ola 29, ADR-0022). Puro JVM, mismo
 * criterio que `CopyMenuTest.kt`.
 */
class CopyVentasTest {

    private val palabrasProhibidasFeria = listOf(
        "boleta",
        "catálogo",
        "catalogo",
        "stock",
        "cajón",
        "cajon",
        "arqueo",
        "transacción",
        "transaccion",
        "orden",
        "pedido",
        "sku",
        "ítem",
        "item",
    )

    private val palabrasDeVoseoArgentino = listOf(
        "tenés",
        "escribí",
        "probá",
        "querés",
        "podés",
        "fijate",
    )

    @Test
    fun `titulo y subtitulo cambian por rubro`() {
        assertEquals("Lo que vendiste hoy", tituloHistorial(feria = true))
        assertEquals("Ventas de hoy", tituloHistorial(feria = false))
    }

    @Test
    fun `boton de deshacer dice me equivoque en feria`() {
        assertEquals("Me equivoqué", ctaDeshacerVenta(feria = true))
        assertEquals("Devolver esta venta", ctaDeshacerVenta(feria = false))
    }

    @Test
    fun `confirmacion dice el monto y que las cosas vuelven`() {
        val mensaje = mensajeConfirmarDeshacer(feria = true, montoFormateado = "$1.200")
        assertEquals(
            "Se deshace la venta completa por $1.200 y las cosas vuelven a lo que tienes.",
            mensaje,
        )
        assertFalse(mensaje.contains("¿estás seguro?"))
    }

    @Test
    fun `etiqueta de venta devuelta nunca depende solo del color`() {
        assertEquals("Deshecha", etiquetaVentaDevuelta(feria = true))
        assertEquals("Devuelta", etiquetaVentaDevuelta(feria = false))
    }

    @Test
    fun `feria nunca usa vocabulario de retail`() {
        for (copy in todoCopyFeriaUsuario()) {
            val bajo = copy.lowercase()
            for (palabra in palabrasProhibidasFeria) {
                assertFalse(
                    "copy feria no puede decir «$palabra»: «$copy»",
                    Regex("""\b$palabra\b""").containsMatchIn(bajo),
                )
            }
        }
    }

    @Test
    fun `ningun copy usa voseo argentino`() {
        for (copy in todoCopyUsuario()) {
            val bajo = copy.lowercase()
            for (palabra in palabrasDeVoseoArgentino) {
                assertFalse(
                    "el copy no puede usar voseo «$palabra»: «$copy»",
                    bajo.contains(palabra),
                )
            }
        }
    }

    private fun todoCopyFeriaUsuario(): List<String> = listOf(
        tituloHistorial(true),
        subtituloHistorial(true),
        tituloEntradaHistorial(true),
        subtituloEntradaHistorial(true),
        etiquetaComoPago("cash", true),
        etiquetaComoPago("card", true),
        etiquetaComoPago("transfer", true),
        etiquetaComoPago("credit", true),
        etiquetaVentaDevuelta(true),
        tituloVacioHistorial(true),
        beneficioVacioHistorial(true),
        pistaVacioHistorial(true),
        tituloErrorHistorial(true),
        tituloDetalleVenta(true),
        subtituloDetalleVenta(),
        tituloQueLlevaba(true),
        tituloComoPago(),
        tituloCuandoFue(),
        ctaDeshacerVenta(true),
        ctaDeshaciendo(true),
        tituloConfirmarDeshacer(true),
        mensajeConfirmarDeshacer(true, "$1.200"),
        botonConfirmarDeshacer(true),
        botonCancelarDeshacer(),
        motivoDeDevolucion(true),
        avisoDeshecha(true),
        tituloErrorDeshacer(true),
        mensajeSinPermisoDeshacer(true),
    )

    private fun todoCopyUsuario(): List<String> = todoCopyFeriaUsuario() + listOf(
        tituloHistorial(false),
        subtituloHistorial(false),
        tituloEntradaHistorial(false),
        subtituloEntradaHistorial(false),
        etiquetaVentaDevuelta(false),
        tituloVacioHistorial(false),
        beneficioVacioHistorial(false),
        pistaVacioHistorial(false),
        tituloErrorHistorial(false),
        tituloDetalleVenta(false),
        tituloQueLlevaba(false),
        ctaDeshacerVenta(false),
        ctaDeshaciendo(false),
        tituloConfirmarDeshacer(false),
        mensajeConfirmarDeshacer(false, "$1.200"),
        botonConfirmarDeshacer(false),
        motivoDeDevolucion(false),
        avisoDeshecha(false),
        tituloErrorDeshacer(false),
        mensajeSinPermisoDeshacer(false),
    )
}
