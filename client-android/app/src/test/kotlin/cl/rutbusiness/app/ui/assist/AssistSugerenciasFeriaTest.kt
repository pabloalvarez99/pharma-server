package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.rubro.PACK_FARMACIA
import cl.rutbusiness.core.rubro.PACK_FERIA
import cl.rutbusiness.core.rubro.PACK_OTRO
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Chips day-1 feria (ADR-0022): kg, fiado, total del día.
 *
 * Si el copy deja de mencionar unidades o deudas, el feriante no aprende el
 * lenguaje del agente en el primer minuto.
 */
class AssistSugerenciasFeriaTest {

    @Test
    fun `feria incluye venta kg fiado y total del dia`() {
        val chips = AssistSugerencias.para(PACK_FERIA)
        val texto = chips.joinToString(" | ")
        assertTrue("falta kg: $texto", chips.any { it.contains("kg", ignoreCase = true) })
        assertTrue(
            "falta fiado: $texto",
            chips.any { it.contains("fiado", ignoreCase = true) || it.contains("debe", ignoreCase = true) },
        )
        assertTrue(
            "falta total del día: $texto",
            chips.any { it.contains("hoy", ignoreCase = true) },
        )
        // precio_dicho (feria_catalogo): cola ` a 2000` / ` a 1500` para 1ª venta sin SKU
        assertTrue(
            "falta precio dicho (a 2000 / a 1500): $texto",
            chips.any { it.contains(" a 2000") || it.contains(" a 1500") },
        )
        assertTrue(chips.size >= 3)
    }

    @Test
    fun `intro feria habla de puesto`() {
        val intro = AssistSugerencias.intro(PACK_FERIA)
        assertTrue(intro.contains("puesto", ignoreCase = true))
    }

    @Test
    fun `farmacia y otro no usan chips feria de kg`() {
        val farma = AssistSugerencias.para(PACK_FARMACIA)
        val otro = AssistSugerencias.para(PACK_OTRO)
        assertFalse(farma.any { it.contains("2 kg de tomates") })
        assertFalse(otro.any { it.contains("2 kg de tomates") })
        assertTrue(farma.contains("¿Cuánto vendí hoy?"))
    }
}
