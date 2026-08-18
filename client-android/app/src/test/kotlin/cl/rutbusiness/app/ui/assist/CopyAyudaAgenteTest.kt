package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.rubro.PACK_FARMACIA
import cl.rutbusiness.core.rubro.PACK_FERIA
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * La lista de ayuda (ola 30, paso 3): tres grupos por lo que la dueña está
 * tratando de hacer, ninguno vacío, sin jerga en feria, sin voseo en ninguna
 * rama — mismo gate que el resto del copy de puesto.
 */
class CopyAyudaAgenteTest {

    private val VOSEO = listOf(
        "tenés", "escribí", "probá", "querés", "podés", "decí", "mirá", "fijate", "anotá", "andá",
    )

    private val JERGA_FERIA = listOf(
        "asistente virtual", " ia ", "consulta", "comando", "función", "capacidad", "módulo",
        "stock", "catálogo", "boleta", "transacción", "ítem", "sku",
    )

    @Test
    fun `hay exactamente tres grupos, en el orden esperado, ninguno vacio`() {
        for (pack in listOf(PACK_FERIA, PACK_FARMACIA)) {
            val grupos = gruposDeAyuda(pack)
            assertEquals(3, grupos.size)
            assertEquals("Saber cómo voy", grupos[0].titulo)
            assertEquals("Anotar lo que pasó", grupos[1].titulo)
            assertEquals("Arreglar algo que salió mal", grupos[2].titulo)
            grupos.forEach { grupo ->
                assertTrue("grupo vacío: ${grupo.titulo}", grupo.frases.isNotEmpty())
            }
        }
    }

    @Test
    fun `los grupos nunca son por modulo del sistema`() {
        val titulos = gruposDeAyuda(PACK_FERIA).map { it.titulo.lowercase() }
        for (prohibido in listOf("ventas", "inventario", "clientes")) {
            assertFalse("grupo por módulo: $prohibido", titulos.any { it == prohibido })
        }
    }

    @Test
    fun `cada frase es completa y tocable`() {
        for (pack in listOf(PACK_FERIA, PACK_FARMACIA)) {
            gruposDeAyuda(pack).flatMap { it.frases }.forEach { frase ->
                assertFalse("termina en dos puntos: $frase", frase.trim().endsWith(":"))
                assertTrue("muy corta: $frase", frase.trim().length >= 8)
                assertTrue("una sola palabra: $frase", frase.trim().contains(" "))
            }
        }
    }

    @Test
    fun `feria no dice jerga de app`() {
        val todas = gruposDeAyuda(PACK_FERIA).flatMap { it.frases }.joinToString(" | ") { " $it " }.lowercase()
        JERGA_FERIA.forEach { palabra ->
            assertFalse("jerga «$palabra» en copy feria: $todas", todas.contains(palabra))
        }
    }

    @Test
    fun `ninguna rama usa voseo argentino`() {
        for (pack in listOf(PACK_FERIA, PACK_FARMACIA)) {
            val todas = gruposDeAyuda(pack).flatMap { it.frases }.joinToString(" | ").lowercase()
            VOSEO.forEach { palabra ->
                assertFalse("voseo «$palabra» en pack=$pack: $todas", todas.contains(palabra))
            }
        }
    }

    @Test
    fun `copyAyuda feria suena de puesto, no de asistente virtual`() {
        val chrome = copyAyuda(feria = true)
        assertFalse(chrome.titulo.contains("asistente", ignoreCase = true))
        assertFalse(chrome.titulo.contains("virtual", ignoreCase = true))
    }
}
