package cl.rutbusiness.app.ui.entrada

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy del cartel de primer uso (feria/nube vs on-prem).
 *
 * Nube: puesto + mesa; cero computador/IP/sistema/servidor.
 * Retail: puede nombrar el computador del negocio.
 */
class CopyPrimerUsoTest {

    /** Jerga on-prem / lab. El remate nube puede negar «computador» a propósito. */
    private val jergaNube = listOf(
        "computador del negocio",
        "servidor",
        "sistema",
        "192.168",
        "10.0.2.2",
        "dashboard",
    )

    private fun textoDe(pasos: List<PasoDelPrimerUso>): String =
        pasos.joinToString(" ") { paso ->
            listOf(
                paso.titulo,
                paso.encabezado,
                paso.parrafos.joinToString(" "),
                paso.lista.joinToString(" "),
                paso.remate.orEmpty(),
            ).joinToString(" ")
        }

    private fun assertSinJergaNube(donde: String, pasos: List<PasoDelPrimerUso>) {
        val lower = textoDe(pasos).lowercase()
        for (palabra in jergaNube) {
            assertFalse(
                "$donde no debe decir «$palabra»: $lower",
                lower.contains(palabra),
            )
        }
    }

    @Test
    fun `nube feria paso 1 nombra puesto y habla de la mesa`() {
        val paso1 = pasosDelPrimerUso(googleDisponible = false, nube = true).first()
        assertEquals("Esto es RutAgent", paso1.titulo)
        assertTrue(paso1.encabezado.contains("puesto", ignoreCase = true))
        assertFalse(paso1.encabezado.contains("negocio", ignoreCase = true))
        assertTrue(
            "feria habla como en la mesa",
            paso1.parrafos.any { it.contains("como en la mesa", ignoreCase = true) },
        )
        assertFalse(
            "feria no dice empleado de confianza",
            paso1.parrafos.any { it.contains("empleado de confianza", ignoreCase = true) },
        )
    }

    @Test
    fun `nube nunca dice computador del negocio IP sistema ni servidor`() {
        val conYSinGoogle = listOf(
            pasosDelPrimerUso(googleDisponible = false, nube = true),
            pasosDelPrimerUso(googleDisponible = true, nube = true),
        )
        conYSinGoogle.forEach { pasos ->
            assertSinJergaNube("primer uso nube", pasos)
        }
        // Promesa del paso 3: se niega el computador; no se pide dirección.
        val remate = pasosDelPrimerUso(googleDisponible = false, nube = true).last().remate
        assertTrue(
            "nube sigue prometiendo que no hace falta computador",
            remate!!.contains("No hace falta un computador", ignoreCase = true),
        )
        // Fuera del remate, ni la palabra «computador».
        val sinRemate = pasosDelPrimerUso(googleDisponible = false, nube = true)
            .flatMap { listOf(it.titulo, it.encabezado) + it.parrafos + it.lista }
            .joinToString(" ")
            .lowercase()
        assertFalse(sinRemate.contains("computador"))
    }

    @Test
    fun `on-prem retail conserva negocio y puede nombrar computador`() {
        val paso1 = pasosDelPrimerUso(googleDisponible = false, nube = false).first()
        assertTrue(paso1.encabezado.contains("negocio", ignoreCase = true))
        assertTrue(
            paso1.parrafos.any { it.contains("empleado de confianza", ignoreCase = true) },
        )
        val paso3 = pasosDelPrimerUso(googleDisponible = false, nube = false).last()
        val texto = textoDe(listOf(paso3))
        assertTrue(texto.contains("192.168.1.10"))
        assertTrue(texto.contains("computador del negocio", ignoreCase = true))
    }

    @Test
    fun `CTAs feria suenan a la mesa y retail a la explicacion`() {
        assertEquals("Siguiente", ctaPrimarioPrimerUso(ultimo = false, nube = true))
        assertEquals("Abrir el puesto", ctaPrimarioPrimerUso(ultimo = true, nube = true))
        assertEquals("Ya sé, entrar", ctaSaltarPrimerUso(nube = true))

        assertEquals("Siguiente", ctaPrimarioPrimerUso(ultimo = false, nube = false))
        assertEquals("Empezar", ctaPrimarioPrimerUso(ultimo = true, nube = false))
        assertEquals("Saltar la explicación", ctaSaltarPrimerUso(nube = false))
    }

    @Test
    fun `Google cableado no promete pronto y sin Google si`() {
        val con = pasosDelPrimerUso(googleDisponible = true, nube = true).last()
        val sin = pasosDelPrimerUso(googleDisponible = false, nube = true).last()
        assertTrue(con.lista.none { it.contains("Pronto", ignoreCase = true) })
        assertTrue(con.lista.any { it.contains("cuenta de Google") })
        assertTrue(sin.lista.any { it.contains("Pronto también con tu cuenta de Google") })
    }
}
