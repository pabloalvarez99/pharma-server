package cl.rutbusiness.app.ui.caja

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Gate de [copyTranquilidadArqueo] (`CopyArqueo.kt`).
 *
 * Puro JVM: si alguien le agrega jerga contable o tono de reto a la línea de
 * tranquilidad de antes de cerrar, este test lo atrapa. No repite el gate de
 * `CopyCajaTest` sobre `copyArqueoCaja` / `copyConfirmarCierre`: esas frases
 * son de otro dueño (`CopyCaja.kt`), este archivo cubre sólo lo nuevo.
 */
class CopyArqueoTest {

    /** Palabras contables/técnicas que la dueña -feria o retail- no dice. */
    private fun assertSinJergaContable(donde: String, texto: String) {
        val lower = texto.lowercase()
        for (palabra in listOf("arqueo", "cuadratura", "descuadre", "saldo teórico", "conciliar")) {
            assertFalse("$donde no debe decir «$palabra»: $texto", lower.contains(palabra))
        }
    }

    /** Nunca en tono de reto: la plata sobra o falta, la persona no falló. */
    private fun assertSinTonoDeReto(donde: String, texto: String) {
        val lower = texto.lowercase()
        assertFalse("$donde no debe decir «te sobra»: $texto", lower.contains("te sobra"))
        assertFalse("$donde no debe decir «te falta»: $texto", lower.contains("te falta"))
    }

    @Test
    fun `feria dice que el dia se cierra igual aunque sobre o falte`() {
        val texto = copyTranquilidadArqueo(feria = true)
        assertTrue(texto.lowercase().contains("día"))
        assertTrue(
            "debe decir que se cierra igual pase lo que pase con el monto",
            texto.lowercase().contains("igual"),
        )
        assertTrue(texto.lowercase().contains("sobra") && texto.lowercase().contains("falta"))
        assertFalse(texto.lowercase().contains("cajón") || texto.lowercase().contains("cajon"))
        assertSinJergaContable("tranquilidad feria", texto)
        assertSinTonoDeReto("tranquilidad feria", texto)
    }

    @Test
    fun `retail dice que la caja se cierra igual aunque sobre o falte`() {
        val texto = copyTranquilidadArqueo(feria = false)
        assertTrue(texto.lowercase().contains("caja"))
        assertTrue(texto.lowercase().contains("igual"))
        assertTrue(texto.lowercase().contains("sobra") && texto.lowercase().contains("falta"))
        assertSinJergaContable("tranquilidad retail", texto)
        assertSinTonoDeReto("tranquilidad retail", texto)
    }

    @Test
    fun `feria y retail no usan exactamente el mismo texto`() {
        assertFalse(copyTranquilidadArqueo(feria = true) == copyTranquilidadArqueo(feria = false))
    }
}
