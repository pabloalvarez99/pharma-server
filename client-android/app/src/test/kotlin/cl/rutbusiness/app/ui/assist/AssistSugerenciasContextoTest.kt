package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.rubro.PACK_FARMACIA
import cl.rutbusiness.core.rubro.PACK_FERIA
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Sugerencias contextuales (ola 30, paso 2): ningún estado devuelve un set
 * vacío, y cada frase es tocable y completa — no un verbo suelto, no una
 * categoría.
 */
class AssistSugerenciasContextoTest {

    private fun esFraseCompleta(s: String) {
        assertFalse("termina en dos puntos: $s", s.trim().endsWith(":"))
        assertTrue("muy corta para ser una frase: $s", s.trim().length >= 8)
        assertTrue("una sola palabra no es una frase: $s", s.trim().contains(" "))
    }

    @Test
    fun `recien empezando en feria enseña a vender y a fiar`() {
        val chips = AssistSugerenciasContexto.para(
            pack = PACK_FERIA,
            recienEmpezando = true,
            ultimaAccionConfirmada = null,
            yaUsadas = emptySet(),
        )
        assertTrue(chips.isNotEmpty())
        assertTrue(chips.any { it.contains("kg", ignoreCase = true) })
        assertTrue(chips.any { it.contains("fiado", ignoreCase = true) })
        // La más valiosa (paso 2): sin ella nadie adivina cómo cargar costos.
        assertTrue(chips.any { it.contains("cuesta", ignoreCase = true) })
        chips.forEach(::esFraseCompleta)
    }

    @Test
    fun `recien empezando fuera de feria enseña a vender y a anotar un gasto`() {
        val chips = AssistSugerenciasContexto.para(
            pack = PACK_FARMACIA,
            recienEmpezando = true,
            ultimaAccionConfirmada = null,
            yaUsadas = emptySet(),
        )
        assertTrue(chips.contains("¿Cuánto vendí hoy?"))
        assertTrue(chips.any { it.contains("gasto", ignoreCase = true) })
        chips.forEach(::esFraseCompleta)
    }

    @Test
    fun `acaba de vender ofrece revisar lo que vendio`() {
        val chips = AssistSugerenciasContexto.para(
            pack = PACK_FARMACIA,
            recienEmpezando = false,
            ultimaAccionConfirmada = "vender",
            yaUsadas = emptySet(),
        )
        assertTrue(chips.contains(AssistSugerenciasContexto.QUE_VENDI_RECIEN))
        assertTrue(chips.contains(AssistSugerenciasContexto.MUESTRAME_LA_ULTIMA_VENTA))
        chips.forEach(::esFraseCompleta)
    }

    @Test
    fun `acaba de fiar ofrece revisar quien debe`() {
        val chips = AssistSugerenciasContexto.para(
            pack = PACK_FERIA,
            recienEmpezando = false,
            ultimaAccionConfirmada = "fiar_venta",
            yaUsadas = emptySet(),
        )
        assertTrue(chips.contains(AssistSugerenciasContexto.QUIEN_ME_DEBE))
        chips.forEach(::esFraseCompleta)
    }

    @Test
    fun `una sugerencia ya usada no se repite arriba`() {
        val usadas = setOf(AssistSugerenciasContexto.QUE_VENDI_RECIEN)
        val chips = AssistSugerenciasContexto.para(
            pack = PACK_FARMACIA,
            recienEmpezando = false,
            ultimaAccionConfirmada = "vender",
            yaUsadas = usadas,
        )
        assertFalse(chips.contains(AssistSugerenciasContexto.QUE_VENDI_RECIEN))
    }

    @Test
    fun `ningun estado deja el set vacio`() {
        val estados = listOf(
            Triple(PACK_FERIA, true, null),
            Triple(PACK_FARMACIA, true, null),
            Triple(PACK_FERIA, false, "vender"),
            Triple(PACK_FARMACIA, false, "fiar_venta"),
            Triple(PACK_FERIA, false, null),
            Triple(PACK_FARMACIA, false, null),
        )
        estados.forEach { (pack, recienEmpezando, ultimaAccion) ->
            val chips = AssistSugerenciasContexto.para(
                pack = pack,
                recienEmpezando = recienEmpezando,
                ultimaAccionConfirmada = ultimaAccion,
                yaUsadas = emptySet(),
            )
            assertTrue("vacío para pack=$pack recienEmpezando=$recienEmpezando accion=$ultimaAccion", chips.isNotEmpty())
        }
    }

    @Test
    fun `cuando no entiende nunca queda vacio, ni cuando ya se usaron todas las sugerencias`() {
        val todo = AssistSugerencias.para(PACK_FARMACIA).toSet() +
            setOf(
                AssistSugerenciasContexto.QUE_VENDI_RECIEN,
                AssistSugerenciasContexto.MUESTRAME_LA_ULTIMA_VENTA,
                AssistSugerenciasContexto.QUIEN_ME_DEBE,
                AssistSugerenciasContexto.EL_TOMATE_ME_CUESTA_800,
                "¿Cuánto vendí hoy?",
                "Registra un gasto de 5000 en arriendo",
            )
        val chips = AssistSugerenciasContexto.cuandoNoEntiende(
            pack = PACK_FARMACIA,
            ultimaAccionConfirmada = "vender",
            yaUsadas = todo,
        )
        assertTrue(chips.isNotEmpty())
    }
}
