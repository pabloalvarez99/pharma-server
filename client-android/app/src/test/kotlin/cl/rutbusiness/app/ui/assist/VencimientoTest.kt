package cl.rutbusiness.app.ui.assist

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class VencimientoTest {

    /** 2026-08-06T12:00:00Z. */
    private val mediodia = 1_786_017_600L

    @Test
    fun `lee la fecha que manda el server`() {
        assertEquals(
            mediodia,
            Vencimiento.epochSegundosDesdeRfc3339("2026-08-06T12:00:00Z"),
        )
    }

    @Test
    fun `lee la fecha con fraccion de segundo`() {
        // `chrono` serializa con nanosegundos cuando los hay.
        assertEquals(
            mediodia,
            Vencimiento.epochSegundosDesdeRfc3339("2026-08-06T12:00:00.123456789Z"),
        )
    }

    @Test
    fun `lee la fecha con desplazamiento de zona`() {
        // Chile continental en invierno. Si algún día el server deja de mandar
        // UTC, esto evita un error de tres horas en silencio.
        assertEquals(
            mediodia,
            Vencimiento.epochSegundosDesdeRfc3339("2026-08-06T09:00:00-03:00"),
        )
    }

    @Test
    fun `una fecha ilegible no rompe nada`() {
        assertNull(Vencimiento.epochSegundosDesdeRfc3339(""))
        assertNull(Vencimiento.epochSegundosDesdeRfc3339("mañana"))
        assertNull(Vencimiento.epochSegundosDesdeRfc3339("2026-13-45T99:99:99Z"))
        assertNull(Vencimiento.segundosRestantes("no es una fecha", mediodia))
    }

    @Test
    fun `descuenta lo que falta`() {
        // El TTL real del server es 180 s (`TOKEN_TTL_SECS`).
        val restantes = Vencimiento.segundosRestantes("2026-08-06T12:03:00Z", mediodia)
        assertEquals(180L, restantes)
    }

    /**
     * El caso que motiva todo el recorte: los aparatos del piso de hardware son
     * viejos y muchos andan con la hora corrida. Un reloj adelantado mostraría
     * "vencida" una propuesta recién nacida y dejaría a la dueña sin poder
     * confirmar nada.
     */
    @Test
    fun `un reloj adelantado no mata una propuesta recien nacida`() {
        val telefonoAdelantadoMediaHora = mediodia + 30 * 60
        val restantes = Vencimiento.segundosRestantes(
            "2026-08-06T12:03:00Z",
            telefonoAdelantadoMediaHora,
        )
        assertEquals(Vencimiento.MINIMO_SEGUNDOS, restantes)
        assertTrue("todavía se puede confirmar", restantes!! > 0)
    }

    @Test
    fun `un reloj atrasado no promete horas que no existen`() {
        val telefonoAtrasadoUnDia = mediodia - 86_400
        val restantes = Vencimiento.segundosRestantes(
            "2026-08-06T12:03:00Z",
            telefonoAtrasadoUnDia,
        )
        assertEquals(Vencimiento.MAXIMO_SEGUNDOS, restantes)
    }

    @Test
    fun `el vencimiento se dice en palabras, no en numeros`() {
        assertEquals("Tienes unos minutos para confirmarla.", Vencimiento.enPalabras(180))
        assertEquals("Queda poco tiempo para confirmarla.", Vencimiento.enPalabras(30))
        assertEquals("Ya pasó el tiempo para confirmarla.", Vencimiento.enPalabras(0))
    }

    @Test
    fun `el texto del vencimiento no tiene jerga`() {
        val jerga = listOf("token", "timestamp", "expira", "TTL", "UTC")
        listOf(200L, 30L, 0L).forEach { segundos ->
            val texto = Vencimiento.enPalabras(segundos)
            jerga.forEach { palabra ->
                assertTrue(
                    "«$texto» le habla a la dueña, no al que escribió el server",
                    !texto.contains(palabra, ignoreCase = true),
                )
            }
        }
    }
}

/**
 * Lo que se le dice a la dueña cuando el server rechaza la confirmación.
 *
 * Prueba de un bug real encontrado corriendo el flujo contra el server: el
 * primer texto decía «no guardé nada. Pídemelo de nuevo». Pero el server no
 * distingue «venció» de «ya se usó» —a propósito, para no ser un oráculo de
 * tokens— y en el segundo caso el gasto **sí** quedó guardado. Con datos
 * móviles intermitentes la respuesta se puede perder después de que el server
 * escribió, así que ese texto podía llevar a anotar el mismo gasto dos veces.
 */
class RechazoTest {

    private val rechazoUsado = "El token de confirmación es inválido o ya fue usado."
    private val rechazoVencido = "El token de confirmación expiró. Vuelve a pedir la acción."

    @Test
    fun `no promete que no se guardo nada`() {
        listOf(rechazoUsado, rechazoVencido).forEach { delServidor ->
            val texto = mensajeDeRechazo(delServidor)
            assertTrue(
                "«$texto» afirma algo que el teléfono no puede saber, y puede " +
                    "terminar en un gasto duplicado",
                !texto.contains("no guardé nada"),
            )
            assertTrue(
                "«$texto» tiene que mandar a revisar antes de repetir",
                texto.contains("revísalo", ignoreCase = true),
            )
        }
    }

    @Test
    fun `no le habla al que escribio el server`() {
        listOf(rechazoUsado, rechazoVencido).forEach { delServidor ->
            val texto = mensajeDeRechazo(delServidor)
            assertTrue("«$texto» todavía dice token", !texto.contains("token", ignoreCase = true))
        }
    }

    /** Un error que no es de token se deja pasar: `AppError` ya está escrito. */
    @Test
    fun `otros errores no se tapan`() {
        val red = "No pudimos conectar con http://192.168.1.10:8080. Revisa que tu teléfono tenga internet."
        assertEquals(red, mensajeDeRechazo(red))
    }
}
