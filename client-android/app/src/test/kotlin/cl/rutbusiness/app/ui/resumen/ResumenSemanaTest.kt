package cl.rutbusiness.app.ui.resumen

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Arma los 7 días del bloque semanal y compara sin sumar — puro JVM.
 *
 * `sales-daily` (ver [DiaUtc.rangoDeSemana]) nunca manda fila para hoy ni
 * para un día sin ventas: acá se prueba que ninguno de los dos casos deja un
 * hueco o un `Float` corrupto en el gráfico.
 */
class ResumenSemanaTest {

    private val esqueleto = listOf(
        DiaEnLaSemana("2026-07-31", diaDeLaSemana = 4, esHoy = false), // viernes
        DiaEnLaSemana("2026-08-01", diaDeLaSemana = 5, esHoy = false), // sábado
        DiaEnLaSemana("2026-08-02", diaDeLaSemana = 6, esHoy = false), // domingo
        DiaEnLaSemana("2026-08-03", diaDeLaSemana = 0, esHoy = false), // lunes
        DiaEnLaSemana("2026-08-04", diaDeLaSemana = 1, esHoy = false), // martes
        DiaEnLaSemana("2026-08-05", diaDeLaSemana = 2, esHoy = false), // miércoles
        DiaEnLaSemana("2026-08-06", diaDeLaSemana = 3, esHoy = true), // jueves, hoy
    )

    // --- armarSemana ----------------------------------------------------------

    @Test
    fun `un dia sin fila del server se completa en cero, no se salta`() {
        val filas = listOf(
            VentaDiariaDto(date = "2026-08-01", orders = 3, revenue = "10000"),
            // 2026-08-02 sin fila: domingo sin ventas.
            VentaDiariaDto(date = "2026-08-03", orders = 1, revenue = "500"),
        )

        val semana = armarSemana(esqueleto, filas, revenueHoy = "7000")

        assertEquals(7, semana.size)
        assertEquals("0", semana.first { it.fecha == "2026-08-02" }.revenue)
        assertEquals("10000", semana.first { it.fecha == "2026-08-01" }.revenue)
    }

    @Test
    fun `hoy nunca sale de las filas de sales-daily, siempre del agregado`() {
        // Si por lo que sea sales-daily mandara una fila para hoy, igual se
        // ignora: el agregado es la única fuente de "hoy" en toda la pantalla.
        val filas = listOf(VentaDiariaDto(date = "2026-08-06", orders = 99, revenue = "999999"))

        val semana = armarSemana(esqueleto, filas, revenueHoy = "7000")

        val hoy = semana.first { it.esHoy }
        assertEquals("7000", hoy.revenue)
        assertEquals("2026-08-06", hoy.fecha)
    }

    @Test
    fun `emparejar por fecha no por posicion no se descuadra con filas desordenadas`() {
        val filas = listOf(
            VentaDiariaDto(date = "2026-08-05", orders = 2, revenue = "200"),
            VentaDiariaDto(date = "2026-07-31", orders = 1, revenue = "100"),
        )

        val semana = armarSemana(esqueleto, filas, revenueHoy = "7000")

        assertEquals("100", semana.first { it.fecha == "2026-07-31" }.revenue)
        assertEquals("200", semana.first { it.fecha == "2026-08-05" }.revenue)
        assertEquals("0", semana.first { it.fecha == "2026-08-03" }.revenue)
    }

    // --- mejorDiaDeLaSemana: compara, nunca suma -------------------------------

    @Test
    fun `el mejor dia es el de mayor monto, no una suma`() {
        val semana = armarSemana(
            esqueleto,
            filas = listOf(
                VentaDiariaDto(date = "2026-08-01", revenue = "5000"),
                VentaDiariaDto(date = "2026-08-03", revenue = "50000"),
            ),
            revenueHoy = "1000",
        )

        val mejor = mejorDiaDeLaSemana(semana)
        assertEquals("2026-08-03", mejor?.fecha)
    }

    @Test
    fun `una semana entera en cero no tiene mejor dia`() {
        val semana = armarSemana(esqueleto, filas = emptyList(), revenueHoy = "0")
        assertNull(mejorDiaDeLaSemana(semana))
    }

    @Test
    fun `un monto que no parsea no revienta la busqueda del mejor dia`() {
        val semana = armarSemana(
            esqueleto,
            filas = listOf(VentaDiariaDto(date = "2026-08-01", revenue = "no-es-un-numero")),
            revenueHoy = "3000",
        )

        val mejor = mejorDiaDeLaSemana(semana)
        assertEquals("2026-08-06", mejor?.fecha) // hoy, el único monto que sí parsea
    }

    // --- alturaDeBarra: geometria, nunca la cifra que se muestra --------------

    @Test
    fun `un monto que no parsea dibuja un dia en cero, no tumba el grafico`() {
        assertEquals(0f, alturaDeBarra(""), 0.001f)
        assertEquals(0f, alturaDeBarra("no-es-un-numero"), 0.001f)
    }

    @Test
    fun `la altura respeta la escala decimal del monto`() {
        assertEquals(1490f, alturaDeBarra("1490"), 0.001f)
        assertEquals(12.5f, alturaDeBarra("12.50"), 0.001f)
    }

    @Test
    fun `un monto en cero da altura cero`() {
        assertTrue(alturaDeBarra("0") == 0f)
        assertTrue(alturaDeBarra("0.00") == 0f)
    }
}
