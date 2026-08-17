package cl.rutbusiness.app.ui.nav

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy de la barra de abajo (ADR-0022).
 *
 * Puro JVM: sin Compose, sin Robolectric. Fija las tres etiquetas por pack,
 * barre la jerga de software que la dueña no usa, y le pone un techo al largo
 * de cada palabra -- el peor caso de [BarraDeNavegacion] es tres etiquetas
 * completas, con la letra del sistema al 200%, repartidas en un panel de
 * 360dp. Si alguien cuela una etiqueta larga acá, ese es el lugar donde se
 * corta una palabra o la barra se rompe, y este test lo atrapa antes.
 */
class CopyNavegacionTest {

    private val palabrasProhibidas = listOf(
        "dashboard",
        "pos",
        "inventario",
        "módulo",
        "modulo",
        "configuración",
        "configuracion",
        "sistema",
        "menú",
        "menu",
    )

    /**
     * Techo de largo para una etiqueta de la barra. Las seis palabras hoy
     * vigentes ("Hablar", "Vender", "Hoy", "Agente", "Cobrar", "Tu día") son
     * todas de 3 a 6 caracteres; 10 deja margen sin abrir la puerta a una
     * frase.
     */
    private val largoMaximo = 10

    @Test
    fun `feria dice Hablar, Vender y Hoy`() {
        assertEquals("Hablar", etiquetaAgente(feria = true))
        assertEquals("Vender", etiquetaCobrar(feria = true))
        assertEquals("Hoy", etiquetaResumen(feria = true))
    }

    @Test
    fun `retail formal dice Agente, Cobrar y Tu dia`() {
        assertEquals("Agente", etiquetaAgente(feria = false))
        assertEquals("Cobrar", etiquetaCobrar(feria = false))
        assertEquals("Tu día", etiquetaResumen(feria = false))
    }

    @Test
    fun `Destino etiquetaPara delega en el copy de la barra`() {
        assertEquals("Hablar", Destino.Agente.etiquetaPara(agentHome = true))
        assertEquals("Vender", Destino.Cobrar.etiquetaPara(agentHome = true))
        assertEquals("Hoy", Destino.Resumen.etiquetaPara(agentHome = true))
        assertEquals("Agente", Destino.Agente.etiquetaPara(agentHome = false))
        assertEquals("Cobrar", Destino.Cobrar.etiquetaPara(agentHome = false))
        assertEquals("Tu día", Destino.Resumen.etiquetaPara(agentHome = false))
    }

    @Test
    fun `la propiedad etiqueta sigue siendo la de retail formal`() {
        assertEquals("Agente", Destino.Agente.etiqueta)
        assertEquals("Cobrar", Destino.Cobrar.etiqueta)
        assertEquals("Tu día", Destino.Resumen.etiqueta)
    }

    @Test
    fun `ninguna etiqueta de la barra usa jerga de software`() {
        for (etiqueta in todasLasEtiquetas()) {
            val bajo = etiqueta.lowercase()
            for (palabra in palabrasProhibidas) {
                assertFalse(
                    "la barra no puede decir «$palabra»: «$etiqueta»",
                    bajo.contains(palabra),
                )
            }
        }
    }

    @Test
    fun `ninguna etiqueta pasa el largo razonable de la barra`() {
        for (etiqueta in todasLasEtiquetas()) {
            assertTrue(
                "«$etiqueta» tiene ${etiqueta.length} caracteres, más que el techo de $largoMaximo",
                etiqueta.length <= largoMaximo,
            )
        }
    }

    /**
     * Los tres destinos existen en los dos packs -feria no esconde ninguno- y
     * el orden de declaración pone "Vender" (Cobrar) antes que "Hoy"
     * (Resumen), que es como se vive el día de feria: primero vender,
     * después el día -y adentro de Hoy, primero las ventas y después quién
     * debe, fijado por separado en `ordenDeBloquesHoy`-.
     */
    @Test
    fun `los tres destinos existen en feria y Vender queda antes que Hoy`() {
        val orden = Destino.entries.toList()
        assertEquals(listOf(Destino.Agente, Destino.Cobrar, Destino.Resumen), orden)
        assertTrue(orden.indexOf(Destino.Cobrar) < orden.indexOf(Destino.Resumen))
    }

    private fun todasLasEtiquetas(): List<String> =
        listOf(true, false).flatMap { feria ->
            Destino.entries.map { it.etiquetaPara(feria) }
        }
}
