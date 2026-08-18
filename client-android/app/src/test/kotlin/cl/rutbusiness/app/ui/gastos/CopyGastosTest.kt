package cl.rutbusiness.app.ui.gastos

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Gate de copy de "Lo que gastaste" (ola 30, ADR-0022). Puro JVM, mismo
 * criterio que `CopyVentasTest.kt` y `CopyMenuTest.kt`.
 */
class CopyGastosTest {

    private val palabrasProhibidasFeria = listOf(
        "stock",
        "catálogo",
        "catalogo",
        "boleta",
        "cajón",
        "cajon",
        "arqueo",
        "sku",
        "ítem",
        "item",
        "transacción",
        "transaccion",
        "proveedor",
        "egreso",
        "comprobante",
    )

    private val palabrasDeVoseoArgentino = listOf(
        "tenés",
        "escribí",
        "probá",
        "querés",
        "podés",
        "fijate",
        "andá",
        "hacé",
        "mirá",
        "decí",
        "anotá",
    )

    @Test
    fun `titulo y subtitulo cambian por rubro`() {
        assertEquals("Lo que gastaste", CopyGastos.titulo(feria = true))
        assertEquals("Gastos", CopyGastos.titulo(feria = false))
    }

    @Test
    fun `boton de anotar dice anotar algo que gastaste en feria`() {
        assertEquals("Anotar algo que gastaste", CopyGastos.ctaAnotar(feria = true))
        assertEquals("Anotar gasto", CopyGastos.ctaAnotar(feria = false))
    }

    @Test
    fun `categorias sugeridas separan etiqueta de valor de protocolo`() {
        val categorias = CopyGastos.CATEGORIAS_SUGERIDAS
        assertEquals("arriendo", categorias.first { it.first == "Arriendo del puesto" }.second)
        assertEquals("flete", categorias.first { it.first == "Flete" }.second)
        assertEquals("bolsas", categorias.first { it.first == "Bolsas" }.second)
        assertEquals("otros", categorias.first { it.first == "Otros" }.second)
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
        CopyGastos.titulo(true),
        CopyGastos.subtitulo(true),
        CopyGastos.chipHoy(),
        CopyGastos.chipEsteMes(),
        CopyGastos.tituloVacio(true),
        CopyGastos.beneficioVacio(true),
        CopyGastos.pistaVacio(true),
        CopyGastos.tituloError(true),
        CopyGastos.mensajeSinPermiso(true),
        CopyGastos.ctaAnotar(true),
        CopyGastos.ctaAnotando(true),
        CopyGastos.tituloFormulario(true),
        CopyGastos.labelMonto(),
        CopyGastos.labelCategoria(true),
        CopyGastos.placeholderCategoria(),
        CopyGastos.labelDescripcion(),
        CopyGastos.placeholderDescripcion(true),
        CopyGastos.tituloComoPagaste(),
        CopyGastos.chipEfectivo(),
        CopyGastos.chipTransferencia(),
        CopyGastos.chipTarjeta(),
        CopyGastos.botonCancelar(),
        CopyGastos.avisoGuardado(true),
        CopyGastos.tituloErrorAnotar(true),
        CopyGastos.mensajeErrorGenerico(true),
    ) + CopyGastos.CATEGORIAS_SUGERIDAS.map { it.first }

    private fun todoCopyUsuario(): List<String> = todoCopyFeriaUsuario() + listOf(
        CopyGastos.titulo(false),
        CopyGastos.subtitulo(false),
        CopyGastos.tituloVacio(false),
        CopyGastos.beneficioVacio(false),
        CopyGastos.pistaVacio(false),
        CopyGastos.tituloError(false),
        CopyGastos.mensajeSinPermiso(false),
        CopyGastos.ctaAnotar(false),
        CopyGastos.ctaAnotando(false),
        CopyGastos.tituloFormulario(false),
        CopyGastos.labelCategoria(false),
        CopyGastos.placeholderDescripcion(false),
        CopyGastos.avisoGuardado(false),
        CopyGastos.tituloErrorAnotar(false),
        CopyGastos.mensajeErrorGenerico(false),
    )
}
