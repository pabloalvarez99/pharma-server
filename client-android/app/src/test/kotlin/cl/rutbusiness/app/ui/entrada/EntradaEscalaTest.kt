package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * La primera pantalla que ve alguien que nunca usó la app, al 100% y al 200%.
 *
 * Es la que más se juega de todas: si acá la persona mayor no encuentra el
 * botón o no puede leer la frase, no llega nunca a las otras seis. Por eso se
 * mide lo mismo que en el resto del producto —táctiles de 56dp, nada cortado a
 * lo ancho— más una cosa que sólo importa acá: que **el botón de avanzar del
 * primer uso se vea sin scrollear**, porque está anclado abajo justamente para
 * eso y un cambio de layout podría empujarlo fuera de pantalla al 200%.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de texto mide ~0,5dp por carácter y toda aserción de ancho pasaría sin
 * significar nada. SDK 34 porque ahí vive la curva no lineal de escala de letra
 * de Android 14, que es la que el usuario recibe de verdad.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class EntradaEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    @Composable
    private fun ConEscala(escala: Float, contenido: @Composable () -> Unit) {
        val base = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(base.density, fontScale = escala),
        ) {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) { contenido() }
            }
        }
    }

    // --- el primer uso ------------------------------------------------------

    private fun mostrarPrimerUso(escala: Float, onListo: () -> Unit = {}) {
        compose.setContent { ConEscala(escala) { PrimerUso(onListo = onListo) } }
        compose.waitForIdle()
    }

    @Test
    fun `el primer uso se lee entero al 100 por ciento`() {
        mostrarPrimerUso(1.0f)
        recorrerLosTresPasos(1.0f)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `el primer uso se lee entero al 200 por ciento`() {
        mostrarPrimerUso(2.0f)
        recorrerLosTresPasos(2.0f)
    }

    /**
     * Recorre los tres pasos revisando lo mismo en cada uno.
     *
     * Va en una sola prueba por escala y no en tres porque `setContent` se puede
     * llamar una sola vez por regla de Compose, y el recorrido es justamente lo
     * que hay que medir: el paso 3 sólo existe después de tocar dos veces.
     */
    private fun recorrerLosTresPasos(escala: Float) {
        // Sin `ServiciosDeEntrada` provistos, `PrimerUso` no tiene Google: los
        // pasos que se recorren son los mismos que arma la pantalla.
        val pasos = pasosDelPrimerUso(googleDisponible = false)
        pasos.forEachIndexed { indice, paso ->
            val ultimo = indice == pasos.lastIndex
            val avanzar = if (ultimo) "Empezar" else "Siguiente"

            compose.onNodeWithText(paso.encabezado, substring = true).assertIsDisplayed()

            // Sin `performScrollTo`: el punto del panel de abajo es que el botón
            // esté a la vista sin que nadie tenga que buscarlo.
            compose.onNodeWithText(avanzar).assertIsDisplayed()
            compose.onNodeWithText("Saltar la explicación").assertIsDisplayed()

            revisarTactiles("$escala x, paso ${indice + 1}")
            revisarQueNadaSeCorte("$escala x, paso ${indice + 1}")

            if (!ultimo) {
                compose.onNodeWithText(avanzar).performClick()
                compose.waitForIdle()
            }
        }
    }

    @Test
    fun `saltar la explicacion sale de la bienvenida`() {
        var listo = false
        mostrarPrimerUso(2.0f, onListo = { listo = true })

        compose.onNodeWithText("Saltar la explicación").performClick()
        compose.waitForIdle()

        assertTrue("saltar no avisó que había terminado", listo)
    }

    /**
     * Cero jerga en las tres pantallas.
     *
     * La regla del encargo es explícita y es la que más fácil se pierde cuando
     * alguien "aclara" un texto agregando la palabra técnica entre paréntesis.
     */
    @Test
    fun `el primer uso no usa jerga`() {
        val prohibidas = listOf("endpoint", "API", "instancia", "tenant", "URL", "HTTP", "IP")
        // Las dos versiones del paso 3, porque la jerga se cuela igual de fácil
        // en la rama que hoy nadie ve.
        val texto = (pasosDelPrimerUso(googleDisponible = false) +
            pasosDelPrimerUso(googleDisponible = true)).joinToString(" ") { paso ->
            listOf(paso.titulo, paso.encabezado).joinToString(" ") +
                paso.parrafos.joinToString(" ") + paso.lista.joinToString(" ") +
                paso.remate.orEmpty()
        }
        val encontradas = prohibidas.filter { texto.contains(it, ignoreCase = true) }
        assertTrue("el primer uso dice $encontradas", encontradas.isEmpty())
    }

    // --- la elección de camino ----------------------------------------------

    private fun mostrarPuerta(
        escala: Float,
        yaEntroAlgunaVez: Boolean = false,
        onCrear: () -> Unit = {},
        onEntrar: () -> Unit = {},
    ) {
        compose.setContent {
            ConEscala(escala) {
                Puerta(
                    yaEntroAlgunaVez = yaEntroAlgunaVez,
                    onCrear = onCrear,
                    onEntrar = onEntrar,
                    onVerExplicacion = {},
                )
            }
        }
        compose.waitForIdle()
    }

    /**
     * Los dos caminos están, y los dos se pueden tocar.
     *
     * Es la pantalla que arregla el problema de origen: la app abría en un
     * formulario que pedía una dirección de servidor que quien la bajó de la
     * tienda no tiene de dónde sacar. Si alguno de los dos botones desaparece o
     * queda fuera de alcance, vuelve ese problema.
     */
    @Test
    fun `los dos caminos se ven y se tocan al 100 por ciento`() {
        mostrarPuerta(1.0f)
        revisarLosDosCaminos(1.0f)
    }

    @Test
    fun `los dos caminos se ven y se tocan al 200 por ciento`() {
        mostrarPuerta(2.0f)
        revisarLosDosCaminos(2.0f)
    }

    private fun revisarLosDosCaminos(escala: Float) {
        compose.onNodeWithText("Crear mi negocio").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Entrar a mi negocio").performScrollTo().assertIsDisplayed()
        revisarTactiles("${escala}x, la puerta")
        revisarQueNadaSeCorte("${escala}x, la puerta")
    }

    @Test
    fun `cada camino lleva a donde dice`() {
        var creando = false
        var entrando = false
        mostrarPuerta(1.0f, onCrear = { creando = true }, onEntrar = { entrando = true })

        compose.onNodeWithText("Crear mi negocio").performClick()
        compose.waitForIdle()
        assertTrue("«Crear mi negocio» no lleva al alta", creando)

        compose.onNodeWithText("Entrar a mi negocio").performClick()
        compose.waitForIdle()
        assertTrue("«Entrar a mi negocio» no lleva al formulario", entrando)
    }

    // --- el formulario ------------------------------------------------------

    private fun mostrarFormulario(
        escala: Float,
        url: String = "192.168.1.10:8080",
        errorDeDireccion: String? = null,
        falla: FallaDeConexion? = null,
        impedimento: String? = null,
    ) {
        compose.setContent {
            ConEscala(escala) {
                FormularioDeEntrada(
                    url = url,
                    onUrl = {},
                    ayudaDeDireccion = "Nos vamos a conectar a 192.168.1.10:8080",
                    errorDeDireccion = errorDeDireccion,
                    negocio = "almacen-dona-rosa",
                    onNegocio = {},
                    email = "rosa@almacen.cl",
                    onEmail = {},
                    password = "secreto",
                    onPassword = {},
                    conexionConfirmada = false,
                    falla = falla,
                    impedimento = impedimento,
                    probando = false,
                    enviando = false,
                    puedeProbar = true,
                    puedeEntrar = true,
                    onProbar = {},
                    onEntrar = {},
                    onVerExplicacion = {},
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun `el formulario se usa entero al 100 por ciento`() {
        mostrarFormulario(1.0f)
        revisarCamposYBotones(1.0f)
    }

    @Test
    fun `el formulario se usa entero al 200 por ciento`() {
        mostrarFormulario(2.0f)
        revisarCamposYBotones(2.0f)
    }

    private fun revisarCamposYBotones(escala: Float) {
        // Por `contentDescription` y no por texto: `RbTextField` borra la
        // semántica de su etiqueta visible para que TalkBack no la lea dos
        // veces, y la pone como descripción del campo.
        listOf(
            "Dirección del computador",
            "Nombre corto del negocio",
            "Correo",
            "Clave",
        ).forEach { etiqueta ->
            compose.onNodeWithContentDescription(etiqueta).performScrollTo().assertIsDisplayed()
        }

        // La columna scrollea, así que el botón puede estar abajo -- lo que no
        // puede es no existir ni quedar fuera de alcance.
        compose.onNodeWithText("Entrar").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Probar la dirección").performScrollTo().assertIsDisplayed()

        revisarTactiles("${escala}x")
        revisarQueNadaSeCorte("${escala}x")
    }

    /**
     * Las cuatro fallas, dibujadas de verdad y al 200%.
     *
     * Son párrafos de tres renglones en el peor caso: si alguna se corta a lo
     * ancho, la instrucción que sirve para arreglar el problema queda a medias.
     */
    @Test
    fun `las cuatro fallas se leen enteras al 200 por ciento`() {
        val direccion = "http://192.168.1.10:8080"
        val fallas = listOf(
            FallaDeConexion.SinRed(),
            FallaDeConexion.NadieContesta(direccion),
            FallaDeConexion.ContestaPeroNoEsElSistema(direccion),
            FallaDeConexion.DatosQueNoCoinciden(),
        )

        // Las cuatro en una sola composición: `setContent` se puede llamar una
        // sola vez por regla de Compose. Van en un scroller porque al 200% cuatro
        // párrafos de tres renglones no entran en 640dp, y lo que se mide acá es
        // el ancho, que es donde no hay a dónde scrollear.
        compose.setContent {
            ConEscala(2.0f) {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(RbTheme.dimens.space3),
                    verticalArrangement = Arrangement.spacedBy(RbTheme.dimens.space3),
                ) {
                    fallas.forEach { falla ->
                        TarjetaDeFallaEntrada(
                            titulo = falla.titulo,
                            queHacer = falla.queHacer,
                        )
                    }
                }
            }
        }
        compose.waitForIdle()

        fallas.forEach { falla ->
            compose.onNodeWithText(falla.titulo).performScrollTo().assertIsDisplayed()
        }
        revisarQueNadaSeCorte("200%, las cuatro fallas")
    }

    // --- las dos revisiones que se repiten ----------------------------------

    private fun revisarTactiles(donde: String) {
        val chicos = mutableListOf<String>()
        val nodos = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        nodos.indices.forEach { i ->
            val nodo = compose.onAllNodes(hasClickAction())[i]
            val caja = nodo.getUnclippedBoundsInRoot()
            if (caja.height < objetivoTactil || caja.width < objetivoTactil) {
                val texto = nodo.fetchSemanticsNode()
                    .config.getOrNull(SemanticsProperties.Text)
                    ?.joinToString(" ") { it.text } ?: "sin texto"
                chicos += "«$texto» mide ${caja.width} x ${caja.height}"
            }
        }
        assertTrue(
            "en $donde estos toques quedaron bajo $objetivoTactil:\n${chicos.joinToString("\n")}",
            chicos.isEmpty(),
        )
    }

    /** A lo ancho no hay a dónde scrollear: lo que no entra, se perdió. */
    private fun revisarQueNadaSeCorte(donde: String) {
        val matcher = hasText("", substring = true)
        val cortados = mutableListOf<String>()
        val nodos = compose.onAllNodes(matcher).fetchSemanticsNodes()
        nodos.indices.forEach { i ->
            val nodo = compose.onAllNodes(matcher)[i]
            val visible = nodo.getBoundsInRoot()
            val completo = nodo.getUnclippedBoundsInRoot()
            if (visible.width <= 0.dp) return@forEach
            if (completo.width - visible.width > 1.dp) {
                val texto = nodo.fetchSemanticsNode()
                    .config.getOrNull(SemanticsProperties.Text)
                    ?.joinToString(" ") { it.text } ?: ""
                cortados += "«$texto»: ${completo.width} de ancho, se ve ${visible.width}"
            }
        }
        assertTrue("en $donde se corta:\n${cortados.joinToString("\n")}", cortados.isEmpty())
    }
}
