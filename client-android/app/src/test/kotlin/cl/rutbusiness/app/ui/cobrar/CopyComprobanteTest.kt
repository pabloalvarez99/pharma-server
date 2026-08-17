package cl.rutbusiness.app.ui.cobrar

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy del paso Comprobante: la pantalla más repetida del día.
 *
 * Puro JVM: si alguien reescribe las etiquetas inline en [PasoComprobante] y
 * se olvida de feria, o cuela jerga técnica ("voucher", "DTE", "transacción",
 * "documento tributario"), este test lo atrapa.
 */
class CopyComprobanteTest {

    @Test
    fun `etiqueta hero distingue vuelto de cobrado`() {
        assertEquals("Vuelto", copyEtiquetaVuelto())
        assertEquals("Cobrado", copyEtiquetaCobrado())
    }

    @Test
    fun `total de la venta recuerda el monto cuando la cifra grande es el vuelto`() {
        assertEquals("Total de la venta: $6.200", copyTotalDeLaVenta("$6.200"))
    }

    @Test
    fun `titulo sin detalle no asusta ni habla de error`() {
        val titulo = copyTituloSinDetalle()
        assertEquals("La venta quedó registrada", titulo)
        assertFalse(titulo.contains("error", ignoreCase = true))
    }

    @Test
    fun `boton para seguir vendiendo es igual en ambos packs`() {
        assertEquals("Cobrar otra venta", copyBotonCobrarOtraVenta())
    }

    @Test
    fun `puntos ganados usa singular solo con exactamente uno`() {
        assertEquals("Sumó 1 punto de fidelidad.", copyPuntosGanados(1L))
        assertEquals("Sumó 2 puntos de fidelidad.", copyPuntosGanados(2L))
        assertEquals("Sumó 0 puntos de fidelidad.", copyPuntosGanados(0L))
    }

    @Test
    fun `venta guardada no promete plata ni pide reintentar cobrar`() {
        assertEquals("Venta guardada", copyTituloVentaGuardada())
        val detalle = copyDetalleVentaGuardada()
        assertTrue(detalle.contains("no se cobra dos veces", ignoreCase = true))
        assertFalse(detalle.contains("$"))
    }

    @Test
    fun `etiquetas de linea son las de un papel de puesto`() {
        assertEquals("Subtotal", copyEtiquetaSubtotal())
        assertEquals("Descuento", copyEtiquetaDescuento())
        assertEquals("Total", copyEtiquetaTotalLinea())
        assertEquals("Pagó con", copyEtiquetaPagoCon())
        assertEquals("Vuelto", copyEtiquetaVueltoLinea())
    }

    @Test
    fun `boton de mandar por chat no dice compartir`() {
        val boton = copyBotonMandarPorChat()
        assertEquals("Mandar por chat", boton)
        assertFalse(boton.contains("compartir", ignoreCase = true))
    }

    @Test
    fun `mensaje para compartir incluye titulo referencia lineas y total sin vuelto`() {
        val mensaje = copyMensajeParaCompartir(
            titulo = "Puesto de la Marta",
            referencia = "Nº 12345678",
            lineas = listOf("2 × Empanada  $2.000", "1 × Bebida  $1.200"),
            total = "$3.200",
            vuelto = null,
        )
        assertEquals(
            "Puesto de la Marta\nNº 12345678\n2 × Empanada  $2.000\n1 × Bebida  $1.200\nTotal $3.200",
            mensaje,
        )
    }

    @Test
    fun `mensaje para compartir suma el vuelto cuando lo hay`() {
        val mensaje = copyMensajeParaCompartir(
            titulo = "Puesto de la Marta",
            referencia = "Nº 12345678",
            lineas = listOf("1 × Pan  $1.000"),
            total = "$1.000",
            vuelto = "$500",
        )
        assertTrue(mensaje.endsWith("Total $1.000 · Vuelto $500"))
    }

    @Test
    fun `ninguna frase del paso comprobante usa jerga tecnica`() {
        val frases = listOf(
            copyEtiquetaVuelto(),
            copyEtiquetaCobrado(),
            copyTotalDeLaVenta("$1.000"),
            copyTituloSinDetalle(),
            copyBotonCobrarOtraVenta(),
            copyPuntosGanados(1L),
            copyPuntosGanados(3L),
            copyTituloVentaGuardada(),
            copyDetalleVentaGuardada(),
            copyEtiquetaSubtotal(),
            copyEtiquetaDescuento(),
            copyEtiquetaTotalLinea(),
            copyEtiquetaPagoCon(),
            copyEtiquetaVueltoLinea(),
            copyBotonMandarPorChat(),
            copyMensajeParaCompartir(
                titulo = "Puesto de la Marta",
                referencia = "Nº 12345678",
                lineas = listOf("1 × Pan  $1.000"),
                total = "$1.000",
                vuelto = "$500",
            ),
        )
        frases.forEach(::assertSinJergaTecnica)
    }

    /** Palabras de log/desarrollador o de boleta fiscal que nunca le tocan la pantalla a la dueña. */
    private fun assertSinJergaTecnica(frase: String) {
        val jerga = listOf(
            "voucher", "dte", "documento tributario", "transacción",
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
