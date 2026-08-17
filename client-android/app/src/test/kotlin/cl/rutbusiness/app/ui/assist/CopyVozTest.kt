package cl.rutbusiness.app.ui.assist

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Gate de copy del control de voz (`BotonDeVoz.kt`).
 *
 * Puro JVM: si alguien reemplaza "Hablar" / "Te escucho" por jerga de
 * reconocimiento de voz, este test lo atrapa. Mismo trato que
 * [CopyEscanerTest] y [CopyComprobanteTest] en `ui/cobrar`.
 */
class CopyVozTest {

    @Test
    fun `la etiqueta quieta invita a tocar, no describe el aparato`() {
        assertEquals("Hablar", ETIQUETA_QUIETO)
    }

    @Test
    fun `la etiqueta escuchando dice que el microfono esta abierto`() {
        assertEquals("Te escucho", ETIQUETA_ESCUCHANDO)
    }

    @Test
    fun `ninguna etiqueta usa jerga tecnica de reconocimiento de voz`() {
        listOf(ETIQUETA_QUIETO, ETIQUETA_ESCUCHANDO).forEach(::assertSinJergaTecnica)
    }

    /** Palabras de motor/log que nunca le tocan el botón a la dueña. */
    private fun assertSinJergaTecnica(frase: String) {
        val jerga = listOf(
            "reconocimiento de voz", "micrófono no disponible", "timeout",
            "stt", "speech", "buffer", "endpoint", "backend", "api", "null",
        )
        jerga.forEach { palabra ->
            assertFalse(
                "\"$frase\" no debería contener la palabra técnica \"$palabra\"",
                frase.contains(palabra, ignoreCase = true),
            )
        }
    }
}
