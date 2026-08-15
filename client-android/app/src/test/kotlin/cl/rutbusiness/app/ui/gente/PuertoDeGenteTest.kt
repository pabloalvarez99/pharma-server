package cl.rutbusiness.app.ui.gente

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Que el botón de compartir exista **en el APK**, no sólo si un test lo cablea.
 *
 * `LocalCompartirConGente` nace en `null`. Las pantallas de deuda y de Hoy que
 * leen `null` esconden el botón a propósito, igual que el CTA al agente. B
 * dejó el puerto listo en [cl.rutbusiness.app.AppContainer] y no pudo tocar
 * `MainActivity`: el merge que no pone la línea deja una app que compila, pasa
 * [CompartirElDiaTest] (porque ese test provee el local él mismo) y en el
 * teléfono no muestra nada.
 *
 * Esta prueba mira el archivo de verdad. Un `CompositionLocal` no se puede
 * preguntar desde afuera de la composición de `MainActivity` sin montar la app
 * entera, y montar la app entera no distingue "el puerto está" de "alguien lo
 * volvió a proveer más abajo". El texto de la línea es el contrato.
 */
class PuertoDeGenteTest {

    @Test
    fun `MainActivity enchufa el puerto de compartir con gente`() {
        val fuente = archivoDeMainActivity().readText()
        assertTrue(
            "MainActivity tiene que proveer LocalCompartirConGente con " +
                "container.compartirConGente; sin eso el botón no aparece en el teléfono",
            fuente.contains("LocalCompartirConGente provides container.compartirConGente"),
        )
    }

    private fun archivoDeMainActivity(): File {
        val trabajo = System.getProperty("user.dir") ?: "."
        var dir: File? = File(trabajo)
        while (dir != null) {
            val candidato = File(dir, "src/main/kotlin/cl/rutbusiness/app/MainActivity.kt")
            if (candidato.isFile) return candidato
            val desdeRaiz = File(dir, "app/src/main/kotlin/cl/rutbusiness/app/MainActivity.kt")
            if (desdeRaiz.isFile) return desdeRaiz
            dir = dir.parentFile
        }
        throw AssertionError("no se encontró MainActivity.kt desde $trabajo")
    }
}
