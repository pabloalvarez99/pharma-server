package cl.rutbusiness.app.ui.resumen

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Los bordes del día que la pantalla le pide al server.
 *
 * Vale la pena probarlo aunque sean cuatro operaciones: un error de un día acá
 * no rompe nada visible — la pantalla muestra un número perfectamente creíble
 * que resulta ser el de anteayer, y nadie se entera hasta que la dueña compara
 * contra su cuaderno.
 */
class DiaUtcTest {

    /** `2026-08-06T14:32:10Z` en milisegundos de época. */
    private val jueves = 1_786_026_730_000L

    @Test
    fun `el rango de ayer cubre el dia entero y no toca hoy`() {
        val (desde, hasta) = DiaUtc.rangoDeAyer(jueves)
        assertEquals("2026-08-05T00:00:00Z", desde)
        // El último milisegundo de ayer, no la medianoche de hoy: el filtro del
        // server es inclusivo en los dos extremos.
        assertEquals("2026-08-05T23:59:59Z", hasta)
    }

    @Test
    fun `el comienzo del dia es medianoche UTC`() {
        assertEquals("2026-08-06T00:00:00Z", DiaUtc.rfc3339(DiaUtc.comienzoDelDia(jueves)))
    }

    /**
     * Justo antes de medianoche UTC sigue siendo el mismo día.
     *
     * Es la hora en que el negocio chileno está cerrando (20:59 en Chile), o
     * sea el momento exacto en que un error de borde se vería en producción.
     */
    @Test
    fun `el ultimo milisegundo del dia todavia es ese dia`() {
        val finDelDia = DiaUtc.comienzoDelDia(jueves) + DiaUtc.MILIS_POR_DIA - 1L
        val (desde, _) = DiaUtc.rangoDeAyer(finDelDia)
        assertEquals("2026-08-05T00:00:00Z", desde)
    }

    /** Primero de mes: ayer es el último día del mes anterior. */
    @Test
    fun `cruzar el comienzo de mes retrocede al mes anterior`() {
        // 2026-09-01T00:00:00Z
        val primeroDeSeptiembre = 1_788_220_800_000L
        val (desde, hasta) = DiaUtc.rangoDeAyer(primeroDeSeptiembre)
        assertEquals("2026-08-31T00:00:00Z", desde)
        assertEquals("2026-08-31T23:59:59Z", hasta)
    }

    /** Y el año, con el 29 de febrero de un bisiesto de por medio. */
    @Test
    fun `el primero de marzo de un bisiesto retrocede al 29 de febrero`() {
        // 2028-03-01T00:00:00Z. 2028 es bisiesto.
        val primeroDeMarzo = 1_835_481_600_000L
        val (desde, _) = DiaUtc.rangoDeAyer(primeroDeMarzo)
        assertEquals("2028-02-29T00:00:00Z", desde)
    }

    @Test
    fun `el primero de enero retrocede al 31 de diciembre anterior`() {
        // 2027-01-01T00:00:00Z
        val anioNuevo = 1_798_761_600_000L
        val (desde, hasta) = DiaUtc.rangoDeAyer(anioNuevo)
        assertEquals("2026-12-31T00:00:00Z", desde)
        assertEquals("2026-12-31T23:59:59Z", hasta)
    }

    /** La época misma, como control de que el origen del cálculo está bien. */
    @Test
    fun `el cero de la epoca es el primero de enero de 1970`() {
        assertEquals("1970-01-01T00:00:00Z", DiaUtc.rfc3339(0L))
    }

    /** Y antes de la época, que es donde la división entera se equivoca sola. */
    @Test
    fun `una fecha anterior a la epoca no se corre un dia`() {
        assertEquals("1969-12-31T23:59:59Z", DiaUtc.rfc3339(-1_000L))
    }
}
