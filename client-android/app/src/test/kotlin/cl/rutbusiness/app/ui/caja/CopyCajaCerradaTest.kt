package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.money.Moneda
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * El aviso de cierre viejo y el resumen para compartir de [PasoCajaCerrada].
 *
 * Lógica pura, sin Compose: corre en milisegundos y en cada commit, igual que
 * `DiferenciaTest`.
 */
class CopyCajaCerradaTest {

    private val pesos = Moneda.de("CLP")

    // Un instante fijo cualquiera de 2026-08-16, para no depender del reloj real.
    private val hoy16Ago = 1_786_872_000_000L // 2026-08-16T09:20:00Z

    // --- cerroOtroDia ----------------------------------------------------------

    @Test
    fun `un cierre del mismo dia UTC no es de otro dia`() {
        assertFalse(cerroOtroDia("2026-08-16T08:00:00Z", hoy16Ago))
        assertFalse(cerroOtroDia("2026-08-16T23:59:59Z", hoy16Ago))
        assertFalse(cerroOtroDia("2026-08-16T00:00:00Z", hoy16Ago))
    }

    @Test
    fun `un cierre de un dia UTC distinto si es de otro dia`() {
        assertTrue(cerroOtroDia("2026-08-15T23:59:59Z", hoy16Ago))
        assertTrue(cerroOtroDia("2026-08-17T00:00:01Z", hoy16Ago))
    }

    /** Sin dato no se acusa: mejor no avisar que avisar mal. */
    @Test
    fun `sin closed_at no se asume que es de otro dia`() {
        assertFalse(cerroOtroDia(null, hoy16Ago))
        assertFalse(cerroOtroDia("", hoy16Ago))
        assertFalse(cerroOtroDia("no es una fecha", hoy16Ago))
    }

    // --- el aviso ----------------------------------------------------------

    /**
     * Gate de tono: recorre [todoCopyCajaCerradaUsuario], mismo criterio que
     * `todoCopyGenteUsuario` en `CopyGenteTest`.
     */
    @Test
    fun `ningun copy de cierre anterior suena a jerga de auditoria`() {
        val jerga = listOf(
            "sistema", "cajón", "cajon", "arqueo", "sesión", "sesion",
            "transacción", "transaccion", "estado closed", "closed",
        )
        todoCopyCajaCerradaUsuario().forEach { texto ->
            jerga.forEach { palabra ->
                assertFalse(
                    "\"$texto\" no debería contener \"$palabra\"",
                    texto.contains(palabra, ignoreCase = true),
                )
            }
        }
    }

    @Test
    fun `feria habla del dia, retail habla de la caja`() {
        assertTrue(tituloCierreAnterior(feria = true).contains("día"))
        assertFalse(tituloCierreAnterior(feria = true).contains("caja"))
        assertTrue(tituloCierreAnterior(feria = false).contains("caja"))
    }

    @Test
    fun `el boton de empezar hoy no dice solo Listo`() {
        assertEquals("Empezar el día de hoy", labelEmpezarHoy(feria = true))
        assertEquals("Abrir la caja de hoy", labelEmpezarHoy(feria = false))
        assertFalse(labelEmpezarHoy(true).equals(labelListoCierre(true), ignoreCase = true))
    }

    // --- el resumen para compartir ------------------------------------------

    @Test
    fun `el resumen para compartir junta titular y explicacion sin la calma`() {
        val copy = copyDeDiferencia(
            moneda = pesos,
            contadoDelServidor = "15000",
            esperadoDelServidor = "17500",
            discrepanciaDelServidor = "-2500",
            feria = true,
        )

        val resumen = resumenDeCierreParaCompartir(copy)

        assertEquals(
            "Faltan $2.500. Contaste $15.000 y lo anotado del día era $17.500.",
            resumen,
        )
        assertFalse(resumen, resumen.contains(copy.calma))
    }

    @Test
    fun `el resumen para compartir no acusa ni usa jerga`() {
        val prohibidas = listOf("te falta", "faltante", "error", "descuadre", "!", "responsab")

        listOf("-2500", "1000", "0").forEach { discrepancia ->
            val copy = copyDeDiferencia(pesos, "15000", "17500", discrepancia, feria = true)
            val resumen = resumenDeCierreParaCompartir(copy).lowercase()
            prohibidas.forEach { palabra ->
                assertFalse(
                    "con discrepancia «$discrepancia» el resumen dice «$palabra»: $resumen",
                    resumen.contains(palabra),
                )
            }
        }
    }
}
