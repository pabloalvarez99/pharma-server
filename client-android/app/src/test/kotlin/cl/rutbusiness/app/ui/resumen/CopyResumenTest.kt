package cl.rutbusiness.app.ui.resumen

import cl.rutbusiness.app.ui.agente.ASI_SE_ANOTA_UNA_VENTA
import cl.rutbusiness.app.ui.agente.ASI_SE_FIA
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Resumen / «Hoy» (ADR-0022).
 *
 * Puro JVM: el feriante lee el cuaderno del día (venta, puesto), no un
 * tablero con boletas, cajón ni computador.
 */
class CopyResumenTest {

    private val palabrasProhibidasFeria = listOf(
        "sistema",
        "cajón",
        "cajon",
        "arqueo",
        "computador",
        "tablero",
        "kpi",
        "dashboard",
        "boleta",
    )

    @Test
    fun `barra feria dice Hoy y mirar de nuevo`() {
        assertEquals("Hoy", tituloHoy(feria = true))
        assertEquals("Tu día", tituloHoy(feria = false))
        assertEquals("Cómo va el puesto hoy", subtituloHoy(feria = true))
        assertEquals("Cómo va el negocio hoy", subtituloHoy(feria = false))
        assertEquals("Mirar de nuevo", labelActualizarHoy(feria = true))
        assertEquals("Actualizar", labelActualizarHoy(feria = false))
        assertEquals("Más", labelAbrirMenuHoy())
    }

    @Test
    fun `carga feria no habla de cuenta de ledger`() {
        val feria = cargandoHoy(feria = true)
        assertEquals("Viendo cómo va el día...", feria)
        assertFalse(
            "feria no dice «cuenta» de ledger al cargar",
            feria.lowercase().contains("cuenta"),
        )
        assertEquals("Sacando la cuenta del día...", cargandoHoy(feria = false))
    }

    @Test
    fun `feria cuenta ventas en la tarjeta`() {
        assertEquals("1 venta.", copyTarjetaConteo(feria = true, conteo = 1L))
        assertEquals("42 ventas.", copyTarjetaConteo(feria = true, conteo = 42L))
        assertFalse(
            "feria no dice boleta",
            copyTarjetaConteo(feria = true, conteo = 3L).lowercase().contains("boleta"),
        )
    }

    @Test
    fun `farmacia sigue contando boletas`() {
        assertEquals("1 boleta.", copyTarjetaConteo(feria = false, conteo = 1L))
        assertEquals("42 boletas.", copyTarjetaConteo(feria = false, conteo = 42L))
        assertEquals(
            "Todavía no hay ninguna venta.",
            copyTarjetaConteo(feria = false, conteo = 0L),
        )
    }

    @Test
    fun `feria resume el dia en ventas`() {
        assertEquals(
            "$1.200 en 1 venta",
            copyResumenDelDia(feria = true, monto = "$1.200", conteo = 1L),
        )
        assertEquals(
            "$5.000 en 7 ventas",
            copyResumenDelDia(feria = true, monto = "$5.000", conteo = 7L),
        )
        assertFalse(
            copyResumenDelDia(feria = true, monto = "$1", conteo = 2L)
                .lowercase()
                .contains("boleta"),
        )
    }

    @Test
    fun `farmacia resume el dia en boletas`() {
        assertEquals(
            "$1.200 en 1 boleta",
            copyResumenDelDia(feria = false, monto = "$1.200", conteo = 1L),
        )
        assertEquals(
            "$5.000 en 7 boletas",
            copyResumenDelDia(feria = false, monto = "$5.000", conteo = 7L),
        )
    }

    @Test
    fun `comparacion sin ayer usa el label de refresco del rubro`() {
        val feria = pistaComparacion(feria = true, vendidoAyerFormateado = null)
        assertTrue(feria.contains("«Mirar de nuevo»"))
        assertFalse(feria.contains("Actualizar"))

        val formal = pistaComparacion(feria = false, vendidoAyerFormateado = null)
        assertTrue(formal.contains("«Actualizar»"))
        assertFalse(formal.contains("Mirar de nuevo"))
    }

    @Test
    fun `vacio del dia ensena la frase del agente`() {
        assertEquals("Todavía no vendiste nada hoy", tituloHoySinVentas())
        val pista = pistaHoySinVentas()
        assertTrue(pista.contains(ASI_SE_ANOTA_UNA_VENTA))
        assertTrue(pista.contains("«") && pista.contains("»"))
        assertTrue(pista.lowercase().contains("agente"))
        assertEquals("Hablarle al agente", ctaHablarleAlAgenteHoy())
    }

    @Test
    fun `fiado en Hoy habla de personas no de cajon`() {
        assertEquals("Te deben", tituloTeDebenHoy())
        assertEquals("1 persona te debe.", cuantosTeDebenHoy(1))
        assertEquals("3 personas te deben.", cuantosTeDebenHoy(3))
        assertEquals("Quién me debe", ctaFiadoHoy(hayDeuda = false))
        assertEquals("Ver quién me debe", ctaFiadoHoy(hayDeuda = true))

        val vacio = vacioFiadoHoy(feria = true)
        assertTrue(vacio.contains(ASI_SE_FIA))
        assertFalse(vacio.lowercase().contains("cajón") || vacio.lowercase().contains("cajon"))

        val error = errorFiadoHoy(feria = true)
        assertTrue(error.contains("«Mirar de nuevo»"))
        assertFalse(error.lowercase().contains("computador"))
    }

    @Test
    fun `feria explica la caja sin computador`() {
        val t = copyEnCajaExplicacion(feria = true, nombreDeCaja = null)
        assertTrue(t.lowercase().contains("puesto"))
        assertFalse(
            "feria no menciona computador",
            t.lowercase().contains("computador"),
        )
        assertTrue(t.contains("anotaste hoy") || t.contains("anotó"))
    }

    @Test
    fun `farmacia puede seguir hablando del computador del negocio`() {
        val t = copyEnCajaExplicacion(feria = false, nombreDeCaja = "Caja 1")
        assertTrue(t.contains("«Caja 1»"))
        assertTrue(t.lowercase().contains("computador"))
    }

    @Test
    fun `el orden de las tarjetas pone plata entrada y deuda antes que el cierre`() {
        assertEquals(listOf("ventas", "semana", "fiado"), ordenDeBloquesHoy(feria = true))
        assertEquals(
            listOf("ventas", "semana", "fiado", "caja", "faltantes", "vencimientos"),
            ordenDeBloquesHoy(feria = false),
        )
    }

    @Test
    fun `la semana nunca dice un total sumado, solo compara`() {
        assertEquals(
            "Gráfico de la semana. El mejor día fue el sábado, con $12.000.",
            descripcionSemana(mejorDiaFrase = "el sábado", mejorDiaMontoFormateado = "$12.000"),
        )
        val sinDatos = descripcionSemana(mejorDiaFrase = null, mejorDiaMontoFormateado = null)
        assertTrue(sinDatos.contains("Todavía no hay ventas"))
        assertFalse(
            "la descripcion de la semana no puede sumar los 7 dias",
            descripcionSemana("el sábado", "$12.000").lowercase().contains("total"),
        )
    }

    @Test
    fun `hoy nunca lleva el articulo el`() {
        assertEquals("hoy", mejorDiaFrase("jueves", esHoy = true))
        assertEquals("el jueves", mejorDiaFrase("jueves", esHoy = false))
    }

    @Test
    fun `los dias de la semana estan en orden lunes a domingo`() {
        assertEquals("lu", letraDiaSemana(0))
        assertEquals("do", letraDiaSemana(6))
        assertEquals("lunes", nombreDiaSemana(0))
        assertEquals("domingo", nombreDiaSemana(6))
        assertEquals("Hoy", etiquetaHoySemana())
    }

    @Test
    fun `se esta por acabar cuenta en singular y plural sin fecha de maquina`() {
        assertEquals("1 producto se está acabando.", tituloSePorAcabar(1))
        assertEquals("5 productos se están acabando.", tituloSePorAcabar(5))
        assertEquals(
            "· Tomate — quedan 3",
            filaSePorAcabar(nombre = "Tomate", stock = 3L),
        )
        assertEquals(
            "· Tomate — sin stock",
            filaSePorAcabar(nombre = "Tomate", stock = 0L),
        )
        assertTrue(vacioSePorAcabar(umbral = 5).contains("5"))
        assertEquals("Y 2 más. Pídeselos al agente: «¿qué se está por acabar?».", masSePorAcabar(2))
    }

    @Test
    fun `se esta por vencer dice el plazo como lo diria una persona`() {
        assertEquals("1 lote se vence pronto.", tituloPorVencer(1))
        assertEquals("4 lotes se vencen pronto.", tituloPorVencer(4))
        assertEquals(
            "· Yogurt — vence mañana",
            filaPorVencer(producto = "Yogurt", plazo = plazoDeVencimiento(diasParaVencer = 1L, expired = false)),
        )
        assertEquals("vence hoy", plazoDeVencimiento(diasParaVencer = 0L, expired = false))
        assertEquals("ya vencido", plazoDeVencimiento(diasParaVencer = -1L, expired = true))
        assertEquals("le quedan 10 días", plazoDeVencimiento(diasParaVencer = 10L, expired = false))
        assertTrue(vacioPorVencer(dias = 30).contains("30"))
        assertEquals("Y 3 más.", masPorVencer(3))
    }

    @Test
    fun `feria nunca usa jerga de tablero ni de cajon`() {
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

    /** Strings de usuario en el camino feria de «Hoy». */
    private fun todoCopyFeriaUsuario(): List<String> = listOf(
        tituloHoy(true),
        subtituloHoy(true),
        labelActualizarHoy(true),
        cargandoHoy(true),
        tituloVendidoHoy(),
        copyTarjetaConteo(true, 0L),
        copyTarjetaConteo(true, 1L),
        copyTarjetaConteo(true, 7L),
        etiquetaComparacion(Comparacion.Mejor),
        etiquetaComparacion(Comparacion.Igual),
        etiquetaComparacion(Comparacion.Peor),
        etiquetaComparacion(Comparacion.SinDatoDeAyer),
        pistaComparacion(true, null),
        pistaComparacion(true, "$10.000"),
        copyResumenDelDia(true, "$5.000", 1L),
        copyResumenDelDia(true, "$5.000", 3L),
        tituloHoySinVentas(),
        pistaHoySinVentas(),
        ctaHablarleAlAgenteHoy(),
        tituloTeDebenHoy(),
        errorFiadoHoy(true),
        vacioFiadoHoy(true),
        cuantosTeDebenHoy(1),
        cuantosTeDebenHoy(4),
        ctaFiadoHoy(false),
        ctaFiadoHoy(true),
        copyEnCajaExplicacion(true, null),
        copyEnCajaExplicacion(true, "Puesto"),
        tituloSemana(),
        errorSemanaHoy(true),
        etiquetaHoySemana(),
        descripcionSemana(null, null),
        descripcionSemana("el sábado", "$12.000"),
        mejorDiaFrase("sábado", esHoy = false),
        mejorDiaFrase("jueves", esHoy = true),
    ) + (0..6).map { letraDiaSemana(it) } + (0..6).map { nombreDiaSemana(it) }
}
