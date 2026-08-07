package cl.rutbusiness.app.ui

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `ui/` no importa `android.*`.
 *
 * La regla está escrita en el `README.md` de `client-android` y sostiene la
 * decisión de ADR-0021: el día que se sume iOS, todo lo que hay bajo `ui/` tiene
 * que poder compilar tal cual. El comentario lo respeta todo el mundo hasta que
 * alguien necesita un `Context` con apuro, y la rotura no se ve hasta meses
 * después, cuando ya hay veinte archivos así.
 *
 * El escáner es el primer caso de la app donde había una tentación real: CameraX
 * y ML Kit son puro Android. Por eso viven en `cl.rutbusiness.app.camara`, del
 * otro lado de la interfaz `CamaraDeCodigos`, y por eso esta prueba existe justo
 * ahora — es el momento en que la frontera pasó de ser gratis a costar algo.
 *
 * `:design` ya tiene la suya (`RbCmpDisciplineTest`) sobre su `commonMain`. Ésta
 * cubre el módulo `:app`, que no tiene `commonMain` y por eso quedaba afuera.
 */
class FronteraDePlataformaTest {

    private val prohibido = Regex("""^\s*import\s+(android|dalvik|java\.awt)\.""")

    /**
     * Las excepciones, y por qué cada una lo es.
     *
     * `androidx.*` no está prohibido: Compose entero vive ahí y tiene versión
     * multiplataforma. Lo que no puede aparecer es el SDK de Android a secas.
     */
    private val excepciones = emptySet<String>()

    @Test
    fun `ningun archivo de ui importa el SDK de Android`() {
        val raiz = directorioDeUi()
        val infractores = mutableListOf<String>()

        raiz.walkTopDown()
            .filter { it.isFile && it.extension == "kt" && it.name !in excepciones }
            .forEach { archivo ->
                archivo.readLines().forEachIndexed { indice, linea ->
                    if (prohibido.containsMatchIn(linea)) {
                        infractores += "${archivo.name}:${indice + 1}: ${linea.trim()}"
                    }
                }
            }

        assertTrue(
            "estos archivos de ui/ importan plataforma; el código de Android va en " +
                "cl.rutbusiness.app.camara o en MainActivity:\n" + infractores.joinToString("\n"),
            infractores.isEmpty(),
        )
    }

    private fun directorioDeUi(): File {
        val trabajo = System.getProperty("user.dir") ?: "."
        var dir: File? = File(trabajo)
        while (dir != null) {
            val candidato = File(dir, "src/main/kotlin/cl/rutbusiness/app/ui")
            if (candidato.isDirectory) return candidato
            val desdeRaiz = File(dir, "app/src/main/kotlin/cl/rutbusiness/app/ui")
            if (desdeRaiz.isDirectory) return desdeRaiz
            dir = dir.parentFile
        }
        throw AssertionError("no se encontró el directorio ui/ desde $trabajo")
    }
}
