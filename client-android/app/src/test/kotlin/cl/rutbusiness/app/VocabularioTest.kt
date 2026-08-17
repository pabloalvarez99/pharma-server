package cl.rutbusiness.app

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Barrido GLOBAL de vocabulario sobre `app/src/main/kotlin/**`, no archivo por
 * archivo. Existe porque cada `Copy*.kt` tiene su propio test de frases fijas
 * y palabras prohibidas, pero eso deja afuera cualquier string de cara a la
 * dueña que viva en otro lado: un mapa de etiquetas, un `when` con literales,
 * un mensaje de error, un `contentDescription`. Ahí se coló `"stock" to
 * "Stock"` en `DetalleAccion.kt` — ese mapa llega a pantallas de feria sin que
 * ningún test lo mirara.
 *
 * ## Cómo distingue string de cara a la dueña de clave de protocolo
 *
 * Se extraen los literales string de cada `.kt` con un mini-lexer (no regex
 * sobre el archivo entero) que sigue el código carácter por carácter y separa
 * comentarios (`//`, `/* */`, KDoc) de literales reales. Esto es a propósito:
 * KDoc en este mismo repo explica en español, con las palabras prohibidas
 * adentro, por qué están prohibidas (`CopyMenu.kt`: `/** En feria no se dice
 * "caja" ni "arqueo" */`) — si se barriera el archivo como texto plano, el
 * comentario que EXPLICA la regla haría fallar el test que la hace cumplir.
 * Solo importan los literales string reales.
 *
 * De los literales extraídos, se descarta como "clave de máquina" (y no se
 * revisa) cualquiera que sea ASCII puro, todo en minúscula (sin una sola
 * mayúscula) y sin espacios. `low_stock`, `pos_cash`, `product_id` caen ahí
 * por tener `_`, pero también caen las claves de una palabra sin `_` como
 * `stock`, `set`, `cash`, `id`: son justo la forma que describe el encargo
 * para una clave del protocolo del server. Lo que SÍ se revisa es todo lo
 * demás: cualquier string con tilde/ñ/¿/¡, con espacio, o con una mayúscula.
 * Por eso el mapa `ETIQUETAS` de `DetalleAccion.kt` cae distinto en cada
 * lado: la clave `"stock"` se descarta (clave de máquina), pero el valor
 * `"Stock"` tiene mayúscula → se revisa → contiene la palabra prohibida → el
 * test falla. Así se pensó, a propósito, para cazar ese caso exacto.
 *
 * ## La otra mitad: por qué la palabra prohibida no se banea globalmente
 *
 * "Stock", "boleta", "catálogo", "arqueo" y "cajón" son palabras VÁLIDAS en
 * retail — `CopyBoleta.kt`, `Boleta.kt`, `PasoBuscar.kt` las dicen a propósito
 * en la rama que no es feria (`else -> "Boleta impresa."`). Un barrido que las
 * prohibiera en TODO el árbol fallaría hoy mismo sobre código correcto, y "que
 * falle por la razón correcta" es el criterio del encargo. La señal que se usa
 * para decidir si un archivo entra al barrido de palabras-de-feria es
 * simple y deliberadamente burda: si el archivo menciona `feria` en cualquier
 * parte (parámetro, KDoc, `if (feria)`), se asume que YA tiene una rama que
 * separa feria de retail — y que la corrección de esa rama es trabajo de un
 * test de ese archivo (como los `Copy*Test.kt` existentes, o el nuevo test en
 * `DetalleAccionTest.kt` que llama `lineas(params, feria = true)`), no de este
 * barrido. Si el archivo NO menciona `feria` en ninguna parte, no hay ninguna
 * rama que pueda estar evitando la palabra prohibida en feria — cualquier
 * ocurrencia ahí es sospechosa por definición, que es exactamente la forma
 * del bug de `DetalleAccion.kt` (el archivo no sabía que existía feria).
 *
 * ## Qué NO cubre (a propósito, para no prometer de más)
 *
 * - **No verifica que la rama de feria esté bien**, solo que un archivo sin
 *   ninguna mención de `feria` no use la palabra prohibida. Un archivo que
 *   YA tiene un `if (feria)` pero con la condición dada vuelta (le muestra la
 *   palabra prohibida justo cuando `feria == true`) no lo cacha — eso es
 *   trabajo de un test que llame la función con `feria = true` de verdad.
 * - **Interpolación de variables** (`"$nombre hizo algo"`): el lexer no
 *   evalúa la plantilla, solo mira el texto literal tal cual está escrito.
 * - **Palabras en español sin tilde, en minúscula y sin espacio**
 *   (`"efectivo"`, `"tarjeta"`, `"fiado"`): quedan indistinguibles de una
 *   clave de máquina bajo esta regla y se descartan igual. En este repo el
 *   texto de UI de una sola palabra siempre va con mayúscula inicial (`Stock`,
 *   `Monto`, `Cambio`), así que en la práctica no se ha visto el caso — pero
 *   si algún día se escribe una palabra de cara a la dueña toda en minúscula
 *   y sin tilde, este barrido no la va a mirar.
 * - **Nombres de variable, parámetros, comentarios, KDoc**: fuera de alcance
 *   a propósito — no son texto que la dueña vea.
 * - **Recursos fuera de `.kt`** (`strings.xml`, si existiera): no se leen.
 * - **Cadenas armadas por concatenación en varias partes** (`"a" + b + "c"`):
 *   se revisa cada trozo literal por separado, no el resultado final unido.
 * - **`excepcionesVoseo`**: dos literales (`"Probá con "`, `"Proba con "` en
 *   `FallaDeAlta.kt`) están exentos del barrido de voseo a propósito — no son
 *   copy propio, son la marca exacta que el CLIENTE busca dentro de un mensaje
 *   de error que el SERVER manda con voseo (`«…Probá con huevos-de-marta-2.»`)
 *   para sacarle el slug sugerido. El voseo ahí es un dato de otro sistema
 *   (server, fuera de este worktree), no algo que este barrido pueda arreglar
 *   sin romper el parseo. Ver Parte C del reporte.
 * - **`pendientesDeArreglar`**: el barrido SÍ encontró un bug real con la
 *   misma forma que el de `DetalleAccion.kt` en `ui/impresora/Boleta.kt`
 *   (`"Boleta N° "` + folio, en el papel impreso, sin ninguna rama
 *   de feria) — pero arreglarlo es Parte C de este encargo (inventariar sin
 *   tocar, para no pisar a otro asiento), no Parte B. Se exceptúa acá, un
 *   archivo y una palabra a la vez, para que el test quede en verde con la
 *   Parte B sola y siga fallando fuerte si aparece la MISMA palabra en
 *   cualquier OTRO archivo sin rama de feria. Sacar de la lista cuando se
 *   arregle.
 */
class VocabularioTest {

    /** Palabras prohibidas en feria (ADR-0022): un feriante no tiene stock. */
    private val palabrasProhibidasFeria = listOf(
        "stock",
        "catálogo",
        "catalogo",
        "boleta",
        "cajón",
        "cajon",
        "arqueo",
    )

    /**
     * Voseo argentino, prohibido en TODA la app (no solo feria): el español
     * es de Chile, con tuteo. Solo las formas acentuadas de la lista del
     * encargo — la forma sin tilde de varias de estas ("mira", "anda",
     * "hace") es tuteo chileno legítimo y de uso frecuente, así que agregarla
     * sin tilde metería falsos positivos, no los evitaría.
     */
    private val voseoProhibido = listOf(
        "tenés",
        "probá",
        "escribí",
        "querés",
        "andá",
        "hacé",
        "mirá",
        "dale que",
    )

    /** Ver KDoc de la clase, sección "excepcionesVoseo". */
    private val excepcionesVoseo = setOf("Probá con ", "Proba con ")

    /** Ver KDoc de la clase, sección "pendientesDeArreglar". */
    private val pendientesDeArreglar = setOf(
        "ui/impresora/Boleta.kt" to "boleta",
    )

    /**
     * Busca `src/main/kotlin` desde el working dir del test hacia arriba: en
     * Gradle suele ser la raíz del módulo `:app` (ahí está directo), pero si
     * algún runner lo deja parado en la raíz del repo o del proyecto Android,
     * también lo encuentra bajando por `app/`.
     */
    private fun raizFuentesApp(): File {
        var dir: File? = File(System.getProperty("user.dir") ?: ".").absoluteFile
        repeat(8) {
            val actual = dir ?: return@repeat
            val directo = File(actual, "src/main/kotlin")
            if (File(directo, "cl/rutbusiness/app").isDirectory) return directo
            val anidado = File(actual, "app/src/main/kotlin")
            if (File(anidado, "cl/rutbusiness/app").isDirectory) return anidado
            dir = actual.parentFile
        }
        error(
            "No se encontró app/src/main/kotlin desde ${System.getProperty("user.dir")}. " +
                "El barrido de vocabulario no puede correr sin la raíz de fuentes.",
        )
    }

    /**
     * Mini-lexer: separa comentarios de literales string. No es un parser de
     * Kotlin completo (no entiende raw strings con `$` escapado de forma
     * especial, por ejemplo), pero sí lo esencial: `//`, `/* */` no son
     * texto de usuario, y `"..."` / `"""..."""` sí pueden serlo.
     */
    private fun literalesDe(codigo: String): List<String> {
        val literales = mutableListOf<String>()
        var i = 0
        val n = codigo.length
        while (i < n) {
            val c = codigo[i]
            when {
                c == '/' && i + 1 < n && codigo[i + 1] == '/' -> {
                    i += 2
                    while (i < n && codigo[i] != '\n') i++
                }
                c == '/' && i + 1 < n && codigo[i + 1] == '*' -> {
                    i += 2
                    while (i + 1 < n && !(codigo[i] == '*' && codigo[i + 1] == '/')) i++
                    i = (i + 2).coerceAtMost(n)
                }
                c == '"' && i + 2 < n && codigo[i + 1] == '"' && codigo[i + 2] == '"' -> {
                    i += 3
                    val inicio = i
                    while (i + 2 < n && !(codigo[i] == '"' && codigo[i + 1] == '"' && codigo[i + 2] == '"')) {
                        i++
                    }
                    literales += codigo.substring(inicio, i.coerceAtMost(n))
                    i = (i + 3).coerceAtMost(n)
                }
                c == '"' -> {
                    i++
                    val sb = StringBuilder()
                    while (i < n && codigo[i] != '"' && codigo[i] != '\n') {
                        if (codigo[i] == '\\' && i + 1 < n) {
                            sb.append(codigo[i + 1])
                            i += 2
                        } else {
                            sb.append(codigo[i])
                            i++
                        }
                    }
                    i++
                    literales += sb.toString()
                }
                else -> i++
            }
        }
        return literales
    }

    /** Ver KDoc de la clase: clave de máquina = ASCII, sin mayúscula, sin espacio. */
    private fun esClaveDeMaquina(literal: String): Boolean {
        if (literal.isBlank()) return true
        if (' ' in literal) return false
        if (literal.any { it.isUpperCase() }) return false
        return literal.all { it.code < 128 }
    }

    private fun archivosFuenteApp(): List<File> =
        raizFuentesApp().walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()

    private data class ArchivoBarrido(val archivo: File, val literales: List<String>, val esFeriaAware: Boolean)

    private fun barrer(): List<ArchivoBarrido> {
        val archivos = archivosFuenteApp()
        check(archivos.isNotEmpty()) { "No se encontraron .kt en app/src/main/kotlin" }
        return archivos.map { archivo ->
            val codigo = archivo.readText()
            ArchivoBarrido(
                archivo = archivo,
                literales = literalesDe(codigo).filterNot(::esClaveDeMaquina),
                esFeriaAware = "feria" in codigo.lowercase(),
            )
        }
    }

    /**
     * Ver KDoc de la clase, sección "La otra mitad": solo se revisan los
     * archivos que no mencionan `feria` en ninguna parte. Un archivo que ya
     * distingue feria de retail es responsabilidad de su propio test (llamar
     * la función con `feria = true` y comprobar el resultado), no de este
     * barrido de texto.
     */
    @Test
    fun `ninguna palabra prohibida de feria aparece en un literal sin rama de feria`() {
        val fallos = mutableListOf<String>()
        barrer().filterNot { it.esFeriaAware }.forEach { (archivo, literales, _) ->
            val rutaNormalizada = archivo.path.replace('\\', '/')
            literales.forEach { literal ->
                val minuscula = literal.lowercase()
                palabrasProhibidasFeria.forEach { palabra ->
                    val exceptuado = pendientesDeArreglar.any { (sufijo, palabraExceptuada) ->
                        palabra == palabraExceptuada && rutaNormalizada.endsWith(sufijo)
                    }
                    if (palabra in minuscula && !exceptuado) {
                        fallos += "${archivo.path}: \"$literal\" contiene \"$palabra\""
                    }
                }
            }
        }
        assertTrue(
            "Palabra prohibida de feria en un archivo que no distingue feria de retail " +
                "(no hay ninguna rama que la esté evitando en feria): \n${fallos.joinToString("\n")}",
            fallos.isEmpty(),
        )
    }

    @Test
    fun `ningun literal de cara a la duena usa voseo argentino`() {
        val fallos = mutableListOf<String>()
        barrer().forEach { (archivo, literales, _) ->
            literales.filterNot { it in excepcionesVoseo }.forEach { literal ->
                val minuscula = literal.lowercase()
                voseoProhibido.forEach { forma ->
                    if (forma in minuscula) {
                        fallos += "${archivo.path}: \"$literal\" contiene voseo \"$forma\""
                    }
                }
            }
        }
        assertTrue(
            "Voseo argentino en literales de cara a la dueña (la app habla en tuteo " +
                "chileno): \n${fallos.joinToString("\n")}",
            fallos.isEmpty(),
        )
    }

    @Test
    fun `el lexer separa comentarios de literales, no barre KDoc`() {
        // CopyMenu.kt explica en un comentario, en español, por qué "arqueo"
        // está prohibido — si el lexer no separara comentario de literal, ese
        // mismo comentario haría fallar el test que hace cumplir la regla.
        val codigoDeEjemplo = """
            /** En feria no se dice "caja" ni "arqueo": se cuenta la plata del puesto. */
            internal fun tituloCaja(feria: Boolean): String =
                if (feria) "Contar la plata del puesto" else "La caja"
        """.trimIndent()
        val literales = literalesDe(codigoDeEjemplo)
        assertTrue(
            "el comentario no debe quedar como literal: $literales",
            literales.none { "arqueo" in it },
        )
        assertTrue(literales.contains("Contar la plata del puesto"))
    }

    @Test
    fun `un archivo sin mencion de feria con la palabra prohibida se marca sospechoso`() {
        // Reproduce la forma exacta del bug histórico: un objeto que no sabe
        // que existe feria, con una palabra prohibida en un valor de mapa.
        val codigoSinFeria = """
            object Etiquetas {
                private val mapa = mapOf("stock" to "Stock")
            }
        """.trimIndent()
        val literales = literalesDe(codigoSinFeria).filterNot(::esClaveDeMaquina)
        assertTrue(literales.contains("Stock"))
        assertTrue("stock" !in literales)
    }
}
