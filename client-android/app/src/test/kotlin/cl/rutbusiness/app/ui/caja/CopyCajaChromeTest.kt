package cl.rutbusiness.app.ui.caja

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de chrome de Caja feria: ritual del día, no admin de cajón ni «sistema».
 */
class CopyCajaChromeTest {

    private fun assertSinJergaFeria(donde: String, texto: String) {
        val lower = texto.lowercase()
        for (palabra in listOf(
            "sistema",
            "cajón",
            "cajon",
            "arqueo",
            "computador",
            "tablero",
            "kpi",
            "dashboard",
            "sesión",
            "sesion",
        )) {
            assertFalse(
                "$donde no debe sonar a admin de caja («$palabra»): $texto",
                lower.contains(palabra),
            )
        }
    }

    @Test
    fun `feria sin senal no pide sistema prendido`() {
        val c = copyCajaSinConexion(feria = true)
        assertEquals("Sin señal no se cierra el día", c.titulo)
        assertTrue(
            "hint honesto: cobro confirma plata, no el teléfono: ${c.hint}",
            c.hint.contains("confirma el cobro", ignoreCase = true),
        )
        assertTrue(
            "hint invita a seguir vendiendo efectivo: ${c.hint}",
            c.hint.contains("efectivo", ignoreCase = true),
        )
        assertTrue(
            "hint cierra el día cuando vuelva la red: ${c.hint}",
            c.hint.contains("red", ignoreCase = true) &&
                c.hint.contains("cierras el día", ignoreCase = true),
        )
        assertEquals("Volver", c.accion)
        assertSinJergaFeria("sin conexión feria", "${c.titulo} | ${c.hint}")
    }

    @Test
    fun `retail sin conexion conserva sistema y cajon`() {
        val c = copyCajaSinConexion(feria = false)
        assertEquals("La caja necesita el sistema prendido", c.titulo)
        assertTrue(c.hint.lowercase().contains("cajón"))
        assertTrue(c.hint.lowercase().contains("arqueo"))
        assertTrue(c.hint.lowercase().contains("sistema"))
        assertEquals("Volver", c.accion)
    }

    @Test
    fun `actualizar en feria es mirar de nuevo como Hoy`() {
        assertEquals("Mirar de nuevo", labelActualizarCaja(feria = true))
        assertEquals("Actualizar", labelActualizarCaja(feria = false))
    }

    @Test
    fun `cargando feria habla de puesto`() {
        assertEquals("Abriendo el puesto...", cargandoCaja(feria = true))
        assertEquals("Viendo cómo está la caja...", cargandoCaja(feria = false))
        assertSinJergaFeria("cargando feria", cargandoCaja(true))
    }

    @Test
    fun `subtitulos feria son ritual del dia`() {
        assertEquals("Sin contar monedas", subtituloPasoCaja(PasoDeCaja.Abrir, feria = true))
        assertEquals("Día en marcha", subtituloPasoCaja(PasoDeCaja.Abierta, feria = true))
        assertEquals(
            "Queda en la cuenta del día",
            subtituloPasoCaja(PasoDeCaja.Movimiento, feria = true),
        )
        assertEquals(
            "Cuenta primero, después cierras",
            subtituloPasoCaja(PasoDeCaja.Arqueo, feria = true),
        )
        assertEquals("Listo por hoy", subtituloPasoCaja(PasoDeCaja.Cerrada, feria = true))
        val juntos = listOf(
            subtituloPasoCaja(PasoDeCaja.Abrir, true),
            subtituloPasoCaja(PasoDeCaja.Abierta, true),
            subtituloPasoCaja(PasoDeCaja.Movimiento, true),
            subtituloPasoCaja(PasoDeCaja.Arqueo, true),
            subtituloPasoCaja(PasoDeCaja.Cerrada, true),
        ).joinToString(" | ")
        assertSinJergaFeria("subtítulos feria", juntos)
    }

    @Test
    fun `subtitulos retail conservan ritual de cajon`() {
        assertEquals("Lo primero del día", subtituloPasoCaja(PasoDeCaja.Abrir, feria = false))
        assertEquals(
            "Caja principal",
            subtituloPasoCaja(PasoDeCaja.Abierta, feria = false, nombreCaja = "Caja principal"),
        )
        assertEquals(
            "Queda anotado hasta el cierre",
            subtituloPasoCaja(PasoDeCaja.Movimiento, feria = false),
        )
        assertEquals(
            "Cuenta primero, después cierra",
            subtituloPasoCaja(PasoDeCaja.Arqueo, feria = false),
        )
        assertEquals(null, subtituloPasoCaja(PasoDeCaja.Cerrada, feria = false))
    }
}
