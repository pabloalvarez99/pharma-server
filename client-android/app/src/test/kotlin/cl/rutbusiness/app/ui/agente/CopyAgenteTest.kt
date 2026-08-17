package cl.rutbusiness.app.ui.agente

import cl.rutbusiness.app.ui.assist.AssistSugerencias
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de tono y de contenido para las frases de ejemplo del agente
 * (ADR-0022). Puro JVM, sin Compose.
 */
class CopyAgenteTest {

    /**
     * La ola 23 sacó todo el voseo rioplatense de la app (43 archivos). Estas
     * dos frases son las que enseñan a hablarle al agente por primera vez, así
     * que un "anotá"/"vendé" colado acá es el peor lugar para que reaparezca.
     */
    @Test
    fun `ninguna frase de ejemplo usa voseo`() {
        val voseo = listOf("tenés", "escribí", "querés", "podés", "anotá", "mirá", "vendé", "fiá")
        todoCopyAgenteUsuario().forEach { frase ->
            voseo.forEach { forma ->
                assertFalse(
                    "«$frase» no debería contener la forma voseante «$forma»",
                    frase.contains(forma, ignoreCase = true),
                )
            }
        }
    }

    @Test
    fun `la frase de venta tiene kilos y precio dicho`() {
        assertEquals("vendí 2 kg de tomates a 2000", ASI_SE_ANOTA_UNA_VENTA)
        assertTrue(ASI_SE_ANOTA_UNA_VENTA.contains("kg"))
        assertTrue(ASI_SE_ANOTA_UNA_VENTA.contains(" a 2000"))
    }

    /**
     * "Don Juan me debe 5000" parsea Incomplete (falta el producto) — ver el
     * comentario de [ASI_SE_FIA]. La frase de ejemplo tiene que cerrar el loop
     * como Venta fiado: producto, precio y a quién.
     */
    @Test
    fun `la frase de fiado cierra el loop - que, cuanto y a quien`() {
        assertTrue(ASI_SE_FIA.contains("kg"))
        assertTrue(ASI_SE_FIA.contains(" a 2000"))
        assertTrue(ASI_SE_FIA.contains("fiado a"))
    }

    /**
     * `AssistSugerencias.feria` (`ui/assist`, otro dueño de esta ola) escribe
     * de nuevo las mismas dos frases en vez de importar estas constantes — ver
     * el comentario de cabecera de `CopyAgente.kt`. No se puede unificar desde
     * acá sin tocar ese archivo, pero si una de las dos copias cambia sola,
     * este test lo agarra en vez de dejar que las dos pantallas le enseñen a
     * la dueña dos maneras distintas de hablarle al mismo agente.
     */
    @Test
    fun `la frase de venta no diverge de la que ya usan los chips de bienvenida`() {
        val chipsFeria = AssistSugerencias.feria.map { it.lowercase() }
        assertTrue(
            "los chips de AssistSugerencias.feria ya no traen «$ASI_SE_ANOTA_UNA_VENTA»: $chipsFeria",
            chipsFeria.any { it == ASI_SE_ANOTA_UNA_VENTA.lowercase() },
        )
    }

    @Test
    fun `la frase de fiado no diverge de la que ya usan los chips de bienvenida`() {
        val chipsFeria = AssistSugerencias.feria.map { it.lowercase() }
        assertTrue(
            "los chips de AssistSugerencias.feria ya no traen «$ASI_SE_FIA»: $chipsFeria",
            chipsFeria.any { it == ASI_SE_FIA.lowercase() },
        )
    }
}
