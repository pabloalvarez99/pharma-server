package cl.rutbusiness.app.ui.impresora

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de copy feria en Impresora (ADR-0022).
 *
 * Feria no tiene impresora ni computador de día 1: SinBluetooth no puede mandar
 * a reimprimir en un PC ni hablar de boleta. Farmacia conserva esa salida.
 */
class CopyImpresoraTest {

    @Test
    fun `feria SinBluetooth no manda a computador ni boleta ni IP`() {
        val texto = FallaDeImpresion.SinBluetooth(feria = true).queHacer
        val lower = texto.lowercase()
        assertFalse("feria no menciona computador", lower.contains("computador"))
        assertFalse("feria no menciona boleta", lower.contains("boleta"))
        assertFalse("feria no filtra IP local", lower.contains("192.168"))
        assertTrue(texto.contains("anotada"))
        assertTrue(texto.contains("teléfono") || texto.contains("telefono"))
    }

    @Test
    fun `farmacia SinBluetooth sigue ofreciendo el computador del negocio`() {
        val texto = FallaDeImpresion.SinBluetooth(feria = false).queHacer
        assertTrue(texto.lowercase().contains("computador"))
        assertTrue(texto.lowercase().contains("boleta"))
    }

    @Test
    fun `helper feria y farmacia coinciden con la sealed class`() {
        assertTrue(
            copyQueHacerSinBluetooth(feria = true) ==
                FallaDeImpresion.SinBluetooth(feria = true).queHacer,
        )
        assertTrue(
            copyQueHacerSinBluetooth(feria = false) ==
                FallaDeImpresion.SinBluetooth(feria = false).queHacer,
        )
    }
}
