package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.error.AppError
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Caja (ADR-0022).
 *
 * Puro JVM: si alguien reescribe «cajón» en el CTA de feria, este test lo atrapa.
 */
class CopyCajaTest {

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
    fun `feria cierra el dia y pregunta cuanta plata hay`() {
        val abierta = copyCajaAbierta(feria = true)
        assertEquals("Cerrar el día", abierta.ctaCerrar)
        assertFalse(abierta.tituloEsperado.lowercase().contains("cajón"))

        val arqueo = copyArqueoCaja(feria = true)
        assertEquals("¿Cuánta plata hay?", arqueo.tituloCard)
        assertEquals("Cerrar el día", arqueo.cta)
        assertFalse(arqueo.tituloCard.lowercase().contains("cajón"))
    }

    @Test
    fun `retail cierra la caja con jerga de cajon`() {
        val abierta = copyCajaAbierta(feria = false)
        assertEquals("Cerrar la caja", abierta.ctaCerrar)
        assertTrue(abierta.tituloEsperado.lowercase().contains("cajón"))

        val arqueo = copyArqueoCaja(feria = false)
        assertTrue(arqueo.tituloCard.lowercase().contains("cajón"))
        assertEquals("Cerrar la caja", arqueo.cta)
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
}
