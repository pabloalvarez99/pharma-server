package cl.rutbusiness.app.ui.nav

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Copy de pestañas feria (ADR-0022): agent_home reescribe etiquetas sin
 * cambiar el enum (Cobrar sigue siendo Cobrar por dentro).
 */
class DestinoFeriaTest {

    @Test
    fun `sin agent_home las etiquetas son las de siempre`() {
        assertEquals("Agente", Destino.Agente.etiquetaPara(false))
        assertEquals("Cobrar", Destino.Cobrar.etiquetaPara(false))
        assertEquals("Tu día", Destino.Resumen.etiquetaPara(false))
    }

    @Test
    fun `con agent_home Cobrar es Vender y Resumen es Hoy`() {
        assertEquals("Agente", Destino.Agente.etiquetaPara(true))
        assertEquals("Vender", Destino.Cobrar.etiquetaPara(true))
        assertEquals("Hoy", Destino.Resumen.etiquetaPara(true))
    }
}
