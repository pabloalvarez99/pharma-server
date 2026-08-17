package cl.rutbusiness.app.ui.fiado

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Fiado (ADR-0022).
 *
 * Puro JVM: si alguien reintroduce «computador», «cajón», «saldo» o jerga de
 * ledger en el camino feria, este test lo atrapa. Formal queda intacto.
 */
class CopyFiadoTest {

    private val palabrasProhibidasFeria = listOf(
        "sistema",
        "cajón",
        "cajon",
        "computador",
        "saldo",
        "export",
        "account",
        "session",
    )

    @Test
    fun `feria nunca usa jerga de ledger ni de farmacia`() {
        for (copy in todoCopyFeriaUsuario()) {
            val bajo = copy.lowercase()
            for (palabra in palabrasProhibidasFeria) {
                assertFalse(
                    "copy feria no puede decir «$palabra»: «$copy»",
                    bajo.contains(palabra),
                )
            }
        }
    }

    @Test
    fun `lista y vacio se leen como nombres y plata`() {
        assertEquals("Quién me debe", tituloListaDeuda())
        assertEquals("Nadie te debe ahora", tituloVacioDeuda(feria = true))
        assertEquals("Nadie te debe plata", tituloVacioDeuda(feria = false))

        val pista = pistaVacioDeuda(feria = true)
        assertTrue(pista.contains(EJEMPLO_FIADO_FERIA))
        assertTrue(pista.contains("«") && pista.contains("»"))
        assertTrue(pista.lowercase().contains("agente"))
    }

    @Test
    fun `actualizar en feria es mirar de nuevo como Hoy y Caja`() {
        assertEquals("Mirar de nuevo", labelActualizarFiado(feria = true))
        assertEquals("Actualizar", labelActualizarFiado(feria = false))
    }

    @Test
    fun `detalle feria dice te debe no saldo de cuenta`() {
        assertEquals("Te debe", tituloDebeAhora(feria = true))
        assertEquals("Debe ahora", tituloDebeAhora(feria = false))
        assertEquals("Quién te debe", tituloDetalleFallback(feria = true))
        assertEquals("La cuenta", tituloDetalleFallback(feria = false))

        val vacio = vacioMovimientos(feria = true)
        assertTrue(vacio.contains(EJEMPLO_FIADO_FERIA))
        assertFalse(vacio.lowercase().contains("saldo"))
    }

    @Test
    fun `movimientos feria se hablan se llevo y dejo plata`() {
        assertEquals("Dejó plata", etiquetaMovimiento(esAbono = true, feria = true))
        assertEquals("Se llevó", etiquetaMovimiento(esAbono = false, feria = true))
        assertEquals("Te pagó", etiquetaMovimiento(esAbono = true, feria = false))
        assertEquals("Se llevó fiado", etiquetaMovimiento(esAbono = false, feria = false))
    }

    @Test
    fun `feria sin detalle pide senal no computador`() {
        val copy = errorSinCuenta(feria = true)
        assertTrue(copy.contains("señal") || copy.contains("senal") || copy.contains("revisá la señal"))
        assertFalse(
            "modoFeria no exige computador",
            copy.lowercase().contains("computador"),
        )
        assertTrue(copy.contains("intentá de nuevo") || copy.contains("intenta de nuevo"))
        assertFalse(copy.lowercase().contains("saldo"))
    }

    @Test
    fun `farmacia sin cuenta sigue nombrando el computador del negocio`() {
        val copy = errorSinCuenta(feria = false)
        assertTrue(copy.lowercase().contains("computador"))
        assertFalse(copy.contains("señal"))
    }

    @Test
    fun `feria efectivo dice que la plata cuenta en el dia del puesto y no cajon`() {
        val copy = ayudaComoPaga(feria = true, entraALaCaja = true)
        assertEquals("Esta plata cuenta en el día del puesto.", copy)
        assertFalse(copy.lowercase().contains("cajón"))
        assertFalse(copy.lowercase().contains("caja"))
        assertFalse(copy.lowercase().contains("computador"))
        assertFalse(copy.lowercase().contains("arqueo"))
        assertFalse(copy.lowercase().contains("cierre"))
    }

    @Test
    fun `feria transferencia habla de puesto o plata del dia sin cajon`() {
        val copy = ayudaComoPaga(feria = true, entraALaCaja = false)
        assertFalse(copy.lowercase().contains("cajón"))
        assertFalse(copy.lowercase().contains("caja"))
        assertFalse(copy.lowercase().contains("arqueo"))
        assertFalse(copy.lowercase().contains("cierre"))
        assertTrue(
            copy.lowercase().contains("día") || copy.lowercase().contains("puesto"),
        )
    }

    @Test
    fun `retail efectivo y transferencia mantienen cajon o caja`() {
        val efectivo = ayudaComoPaga(feria = false, entraALaCaja = true)
        assertTrue(efectivo.lowercase().contains("caja") || efectivo.lowercase().contains("cierre"))

        val transferencia = ayudaComoPaga(feria = false, entraALaCaja = false)
        assertTrue(transferencia.lowercase().contains("cajón"))
    }

    @Test
    fun `remate de abono feria no habla de cajon ni computador`() {
        val feria = remateAbonoEfectivo(feria = true)
        assertTrue(feria.lowercase().contains("día") || feria.lowercase().contains("dia"))
        assertFalse(feria.lowercase().contains("cajón"))
        assertFalse(feria.lowercase().contains("computador"))

        val formal = remateAbonoEfectivo(feria = false)
        assertTrue(formal.lowercase().contains("caja") || formal.lowercase().contains("cierre"))
    }

    @Test
    fun `abono feria suena a mesa y no a cajon ni arqueo`() {
        val abonoFeria = listOf(
            tituloPasoAbono(),
            tituloCuantoPaga(feria = true),
            referenciaDeuda(feria = true, montoFormateado = "$2.000"),
            labelMontoPago(feria = true),
            tituloComoPaga(),
            chipEfectivo(),
            chipTransferencia(),
            ayudaComoPaga(feria = true, entraALaCaja = true),
            ayudaComoPaga(feria = true, entraALaCaja = false),
            tituloNotaPago(),
            labelNotaPago(feria = true),
            placeholderNotaPago(feria = true),
            previaAnotarPago(feria = true),
            ctaAnotarPago(guardando = false, feria = true),
            ctaAnotarPago(guardando = true, feria = true),
            ctaVolverDelAbono(feria = true),
            remateAbonoEfectivo(feria = true),
        )
        val prohibidas = listOf(
            "caja",
            "cajón",
            "cajon",
            "arqueo",
            "computador",
            "sistema",
            "cierre",
        )
        for (copy in abonoFeria) {
            val bajo = copy.lowercase()
            for (palabra in prohibidas) {
                assertFalse(
                    "abono feria no puede decir «$palabra»: «$copy»",
                    bajo.contains(palabra),
                )
            }
        }

        assertEquals("¿Cuánto me das?", tituloCuantoPaga(feria = true))
        assertEquals("Lo que te da", labelMontoPago(feria = true))
        assertEquals("Algo que anotar (opcional)", labelNotaPago(feria = true))
        assertEquals("Vas a anotar lo que te dio:", previaAnotarPago(feria = true))
        assertEquals("Anotar lo que me dio", ctaAnotarPago(guardando = false, feria = true))
        assertEquals("Anotando...", ctaAnotarPago(guardando = true, feria = true))
        assertEquals(
            "Te debe $2.000. Puede ser todo o una parte.",
            referenciaDeuda(feria = true, montoFormateado = "$2.000"),
        )
        assertEquals("Volver", ctaVolverDelAbono(feria = true))
    }

    @Test
    fun `abono retail conserva cajon y el CTA formal`() {
        assertEquals("¿Cuánto te está pagando?", tituloCuantoPaga(feria = false))
        assertEquals("Plata que te da", labelMontoPago(feria = false))
        assertEquals("Nota del pago (opcional)", labelNotaPago(feria = false))
        assertEquals("Vas a anotar que te pagó:", previaAnotarPago(feria = false))
        assertEquals("Anotar el pago", ctaAnotarPago(guardando = false, feria = false))
        assertEquals("Volver a la cuenta", ctaVolverDelAbono(feria = false))

        val efectivo = ayudaComoPaga(feria = false, entraALaCaja = true)
        assertTrue(efectivo.lowercase().contains("caja") || efectivo.lowercase().contains("cierre"))
        val transferencia = ayudaComoPaga(feria = false, entraALaCaja = false)
        assertTrue(transferencia.lowercase().contains("cajón"))
    }
}
