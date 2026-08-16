package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.error.AppError
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Caja (ADR-0022).
 *
 * Puro JVM: si alguien reescribe «cajón» / «arqueo» / «sesión» en el CTA o el
 * flujo de cerrar el día feria, este test lo atrapa.
 */
class CopyCajaTest {

    /** Palabras de cajero formal que el feriante no debe ver. */
    private fun assertSinJergaCajaFormal(donde: String, texto: String) {
        val lower = texto.lowercase()
        assertFalse(
            "$donde no debe decir «cajón»: $texto",
            lower.contains("cajón") || lower.contains("cajon"),
        )
        assertFalse(
            "$donde no debe decir «arqueo»: $texto",
            lower.contains("arqueo"),
        )
        assertFalse(
            "$donde no debe decir «sesión»: $texto",
            lower.contains("sesión") || lower.contains("sesion"),
        )
        assertFalse(
            "$donde no debe usar inglés de caja: $texto",
            lower.contains("session") || lower.contains("cash register") || lower.contains("drawer"),
        )
    }


    @Test
    fun `feria abre el puesto con Empezar el dia y sin cajon como instruccion`() {
        val c = copyAbrirCaja(feria = true)
        assertEquals("Abrir el puesto", c.tituloCard)
        assertEquals("Empezar el día", c.cta)
        assertTrue(c.ayudaMonto.contains("0"))
        assertFalse(
            "la instrucción principal de feria no debe hablar de cajón",
            c.ayuda.lowercase().contains("cajón") || c.tituloCard.lowercase().contains("cajón"),
        )
        assertTrue(c.ayuda.lowercase().contains("puesto") || c.ayudaMonto.lowercase().contains("puesto"))
    }

    @Test
    fun `farmacia pide el cajon y Abrir la caja`() {
        val c = copyAbrirCaja(feria = false)
        assertEquals("Abrir la caja", c.cta)
        assertTrue(c.etiquetaMonto.lowercase().contains("cajón"))
        assertTrue(c.ayuda.lowercase().contains("cajón"))
    }

    @Test
    fun `feria cierra el dia contando la plata sin jerga de arqueo`() {
        val abierta = copyCajaAbierta(feria = true)
        assertEquals("Cerrar el día", abierta.ctaCerrar)
        assertEquals("Puesto abierto", abierta.chipEstado)
        assertFalse(abierta.tituloEsperado.lowercase().contains("cajón"))
        assertFalse(abierta.vacioMovimientos.lowercase().contains("cajón"))
        assertTrue(abierta.vacioMovimientos.lowercase().contains("billete") ||
            abierta.tituloMovimientos.lowercase().contains("plata"))

        val arqueo = copyArqueoCaja(feria = true)
        assertEquals("Contar la plata", arqueo.tituloCard)
        assertEquals("Cerrar el día", arqueo.cta)
        assertEquals("¿Cerramos el día?", arqueo.confirmarTitulo)
        assertTrue(
            "feria invita a contar el puesto o el día",
            arqueo.ayuda.lowercase().contains("puesto") ||
                arqueo.ayuda.lowercase().contains("día"),
        )
        assertTrue(
            "nota feria invita vuelto mal o cambio en chileno",
            arqueo.placeholderNota.lowercase().contains("vuelto") ||
                arqueo.placeholderNota.lowercase().contains("cambio"),
        )
        assertTrue(arqueo.ayudaNota.lowercase().contains("vuelto"))
        // Gate de regresión: el feriante no ve ritual de cajero formal.
        for (s in listOf(
            arqueo.tituloCard,
            arqueo.ayuda,
            arqueo.etiquetaMonto,
            arqueo.ayudaMonto,
            arqueo.tituloNota,
            arqueo.etiquetaNota,
            arqueo.placeholderNota,
            arqueo.ayudaNota,
            arqueo.cta,
            arqueo.ctaGuardando,
            arqueo.ctaVolver,
            arqueo.confirmarTitulo,
            arqueo.confirmarLabel,
            arqueo.cancelarLabel,
        )) {
            assertSinJergaCajaFormal("copy arqueo feria", s)
        }
        assertSinJergaCajaFormal(
            "confirm cierre feria",
            copyConfirmarCierre(feria = true, montoFormateado = "$15.000"),
        )
        val confirm = copyConfirmarCierre(feria = true, montoFormateado = "$15.000")
        assertTrue(confirm.lowercase().contains("día") || confirm.lowercase().contains("puesto"))
        assertTrue(confirm.lowercase().contains("no se puede deshacer") ||
            confirm.lowercase().contains("mañana"))
    }

    @Test
    fun `retail cierra la caja con jerga de cajon`() {
        val abierta = copyCajaAbierta(feria = false)
        assertEquals("Cerrar la caja", abierta.ctaCerrar)
        assertEquals("Caja abierta", abierta.chipEstado)
        assertTrue(abierta.tituloEsperado.lowercase().contains("cajón"))

        val arqueo = copyArqueoCaja(feria = false)
        assertTrue(arqueo.tituloCard.lowercase().contains("cajón"))
        assertEquals("Cerrar la caja", arqueo.cta)
        assertTrue(arqueo.ayudaMonto.lowercase().contains("cajón"))
        val confirm = copyConfirmarCierre(feria = false, montoFormateado = "$15.000")
        assertTrue(confirm.lowercase().contains("caja"))
        assertFalse(
            "retail confirm no usa inglés",
            confirm.contains("session", ignoreCase = true) ||
                confirm.contains("cash", ignoreCase = true),
        )
    }

    @Test
    fun `feria saca y mete plata del puesto sin cajon`() {
        val sacar = copyMovimientoCaja(feria = true, esRetiro = true)
        assertTrue(sacar.ayuda.lowercase().contains("puesto"))
        assertFalse(sacar.ayuda.lowercase().contains("cajón"))
        assertEquals("Volver al puesto", sacar.ctaVolver)

        val meter = copyMovimientoCaja(feria = true, esRetiro = false)
        assertTrue(meter.ayuda.lowercase().contains("puesto"))
        assertFalse(meter.ayuda.lowercase().contains("cajón"))
    }

    @Test
    fun `retail saca y mete del cajon`() {
        val sacar = copyMovimientoCaja(feria = false, esRetiro = true)
        assertTrue(sacar.ayuda.lowercase().contains("cajón"))
        assertEquals("Volver a la caja", sacar.ctaVolver)
    }

    @Test
    fun `titulo del paso feria dice puesto`() {
        assertEquals("Abrir el puesto", tituloPasoCaja(PasoDeCaja.Abrir, feria = true, "retiro"))
        assertEquals("El puesto de hoy", tituloPasoCaja(PasoDeCaja.Abierta, feria = true, "retiro"))
        assertEquals("Abrir la caja", tituloPasoCaja(PasoDeCaja.Abrir, feria = false, "retiro"))
    }

    @Test
    fun `409 es caja ya abierta`() {
        assertTrue(
            esCajaYaAbierta(
                AppError.ErrorDelServidor(
                    status = 409,
                    code = "CONFLICT",
                    serverMessage = "el usuario ya tiene una caja abierta",
                ),
            ),
        )
    }

    @Test
    fun `mensaje de caja abierta sin 409 tambien cuenta`() {
        assertTrue(
            esCajaYaAbierta(
                AppError.ErrorDelServidor(
                    status = 400,
                    code = "INVALID",
                    serverMessage = "El usuario ya tiene una caja abierta en otro dispositivo",
                ),
            ),
        )
    }

    @Test
    fun `500 no es caja ya abierta`() {
        assertFalse(
            esCajaYaAbierta(
                AppError.ErrorDelServidor(
                    status = 500,
                    code = "INTERNAL",
                    serverMessage = "falló el server",
                ),
            ),
        )
    }

    @Test
    fun `error de red feria no nombra computador ni IP`() {
        for (accion in AccionErrorRed.entries) {
            val msg = copyErrorRed(feria = true, accion = accion)
            assertFalse(
                "feria $accion no debe hablar de computador: $msg",
                msg.lowercase().contains("computador"),
            )
            assertFalse(
                "feria $accion no debe mostrar IP local: $msg",
                msg.contains("192.168"),
            )
            assertTrue("feria $accion debe hablar de señal: $msg", msg.lowercase().contains("señal"))
        }
    }

    @Test
    fun `error de red retail sigue con computador del negocio`() {
        for (accion in AccionErrorRed.entries) {
            val msg = copyErrorRed(feria = false, accion = accion)
            assertTrue(
                "retail $accion nombra computador: $msg",
                msg.lowercase().contains("computador"),
            )
        }
    }

    @Test
    fun `cobro feria recuerda que no se cobra dos veces`() {
        val msg = copyErrorRed(feria = true, accion = AccionErrorRed.Cobro)
        assertTrue(msg.lowercase().contains("no se cobra dos veces"))
        assertTrue(msg.lowercase().contains("reintentar") || msg.contains("Reintentar"))
    }

    @Test
    fun `esperado en puesto feria sin computador`() {
        val feria = copyEsperadoEnPuesto(feria = true)
        assertTrue(feria.lowercase().contains("puesto"))
        assertFalse(feria.lowercase().contains("computador"))
        assertFalse(feria.contains("192.168"))

        val retail = copyEsperadoEnPuesto(feria = false)
        assertTrue(retail.lowercase().contains("computador"))
    }
}
