package cl.rutbusiness.app.ui.alta

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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
 * "Crear mi negocio", al 100% y al 200% de escala de letra.
 *
 * Es la pantalla de un negocio que todavía no existe: si acá la persona no
 * encuentra el botón o no puede leer la pregunta, ese negocio no se crea nunca
 * y la app se desinstala. Se mide lo mismo que en el resto del producto
 * —táctiles de 56dp, nada cortado a lo ancho— más lo propio de un formulario en
 * pasos: que **el botón de avanzar se vea sin scrollear en los cuatro pasos**,
 * porque está anclado abajo justamente para eso.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de texto mide ~0,5dp por carácter y toda aserción de ancho pasaría sin
 * significar nada. SDK 34 porque ahí vive la curva no lineal de escala de letra
 * de Android 14, que es la que el usuario recibe de verdad — a 200% un texto de
 * 17sp queda en ~29sp, no en 34.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class AltaEscalaTest {

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

    /**
     * El alta con estado de mentira, para poder recorrerla.
     *
     * `setContent` se puede llamar una sola vez por prueba, así que el paseo por
     * los pasos tiene que ocurrir dentro de una sola composición. Lo único que
     * este doble hace de verdad es avanzar el índice: el resto de las reglas
     * —cuándo se puede avanzar, qué falla mostrar— son del ViewModel y se
     * prueban aparte.
     *
     * @param conNube el caso de la nube compilada: tres pasos y ninguno es una
     *   dirección. Sin nube son cuatro.
     */
    @Composable
    private fun AltaDeMentira(conNube: Boolean, falla: FallaDeAlta? = null) {
        val pasos = remember(conNube) {
            buildList {
                if (!conNube) add(PasoDelAlta.Donde)
                add(PasoDelAlta.Negocio)
                add(PasoDelAlta.Rubro)
                add(PasoDelAlta.Cuenta)
            }
        }
        var indice by remember { mutableStateOf(0) }
        var url by remember { mutableStateOf("192.168.1.10:8080") }
        var nombre by remember { mutableStateOf("Almacén Doña Rosa") }
        var rubro by remember { mutableStateOf<Rubro?>(null) }
        var email by remember { mutableStateOf("rosa@almacen.cl") }
        var clave by remember { mutableStateOf("almacen2026") }

        PantallaDeAlta(
            paso = pasos[indice],
            indice = indice,
            total = pasos.size,
            estado = EstadoDelAlta.Preguntando,
            falla = falla,
            impedimento = null,
            puedeAvanzar = true,
            url = url,
            onUrl = { url = it },
            ayudaDeDireccion = "Ahí vamos a crear tu negocio: 192.168.1.10:8080",
            errorDeDireccion = null,
            nombre = nombre,
            onNombre = { nombre = it },
            rubro = rubro,
            onRubro = { rubro = it },
            email = email,
            onEmail = { email = it },
            errorDeCorreo = null,
            clave = clave,
            onClave = { clave = it },
            errorDeClave = null,
            onAtras = {},
            onAvanzar = { if (indice < pasos.lastIndex) indice += 1 },
            onCrear = {},
            onIrAEntrar = {},
        )
    }

    private fun mostrar(escala: Float, conNube: Boolean, falla: FallaDeAlta? = null) {
        compose.setContent { ConEscala(escala) { AltaDeMentira(conNube, falla) } }
        compose.waitForIdle()
    }

    // --- el recorrido completo ----------------------------------------------

    @Test
    fun `el alta se usa entera al 100 por ciento`() {
        mostrar(1.0f, conNube = false)
        recorrer(1.0f, total = 4)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `el alta se usa entera al 200 por ciento`() {
        mostrar(2.0f, conNube = false)
        recorrer(2.0f, total = 4)
    }

    /**
     * Con nube compilada la dirección no se pregunta nunca.
     *
     * Es el encargo entero en una aserción: "sin tocar una URL". Si alguien
     * agrega el paso de la dirección al camino de la nube, esto se cae.
     */
    @Test
    fun `con nube compilada no se pregunta ninguna direccion`() {
        mostrar(2.0f, conNube = true)

        compose.onNodeWithText(tituloDelPaso(PasoDelAlta.Negocio)).assertIsDisplayed()
        compose.onNodeWithText("Paso 1 de 3").assertIsDisplayed()
        recorrer(2.0f, total = 3)
    }

    private fun recorrer(escala: Float, total: Int) {
        repeat(total) { indice ->
            val ultimo = indice == total - 1
            val avanzar = if (ultimo) "Crear mi negocio" else "Siguiente"

            compose.onNodeWithText("Paso ${indice + 1} de $total").assertIsDisplayed()

            // Sin `performScrollTo`: el punto del panel de abajo es que el botón
            // esté a la vista sin que nadie tenga que buscarlo.
            compose.onNodeWithText(avanzar).assertIsDisplayed()

            revisarTactiles("$escala x, paso ${indice + 1}")
            revisarQueNadaSeCorte("$escala x, paso ${indice + 1}")

            if (!ultimo) {
                compose.onNodeWithText(avanzar).performClick()
                compose.waitForIdle()
            }
        }
    }

    // --- los campos y el rubro ----------------------------------------------

    /**
     * Los cuatro campos existen y se alcanzan.
     *
     * Por `contentDescription` y no por texto: `RbTextField` borra la semántica
     * de su etiqueta visible para que TalkBack no la lea dos veces.
     */
    @Test
    fun `los campos del alta se alcanzan al 200 por ciento`() {
        mostrar(2.0f, conNube = false)

        compose.onNodeWithContentDescription("Dirección del computador")
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()

        compose.onNodeWithContentDescription("Nombre del negocio")
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()

        compose.onNodeWithContentDescription("Tu correo").performScrollTo().assertIsDisplayed()
        compose.onNodeWithContentDescription("Tu clave").performScrollTo().assertIsDisplayed()
    }

    /**
     * Los ocho rubros están, se alcanzan, y elegir uno se ve.
     *
     * Al 200% los ocho no entran en 640dp: se llega a los de abajo scrolleando,
     * que es aceptable porque son opciones de una lista. Lo que no sería
     * aceptable es que alguna no exista o que elegirla no cambie nada en
     * pantalla.
     */
    @Test
    fun `los ocho rubros se eligen al 200 por ciento`() {
        mostrar(2.0f, conNube = true)
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()

        RUBROS.forEach { rubro ->
            compose.onNodeWithText(rubro.etiqueta).performScrollTo().assertIsDisplayed()
        }

        compose.onNodeWithText("Minimarket / Almacén").performScrollTo().performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Elegido").assertIsDisplayed()

        revisarTactiles("200%, los ocho rubros")
        revisarQueNadaSeCorte("200%, los ocho rubros")
    }

    // --- el lugar ocupado ---------------------------------------------------

    /**
     * "Entrar a ese negocio" lleva al formulario de entrada, no a la pantalla
     * anterior.
     *
     * Es una prueba de una línea contra un bug que ya ocurrió: el botón salía
     * cableado al mismo callback que la flecha de atrás, así que decía "entrar"
     * y devolvía a elegir camino de nuevo — justo después de haberle dicho a la
     * dueña que ahí no puede crear nada. Que los dos destinos existan por
     * separado es lo que hace que el botón cumpla lo que promete.
     */
    @Test
    fun `el lugar ocupado local manda a entrar y no a la pantalla anterior`() {
        var entrando = false
        var atras = false

        compose.setContent {
            ConEscala(2.0f) {
                PantallaDeAlta(
                    paso = PasoDelAlta.Negocio,
                    indice = 0,
                    total = 3,
                    estado = EstadoDelAlta.LugarOcupado,
                    falla = null,
                    impedimento = null,
                    puedeAvanzar = false,
                    url = "",
                    onUrl = {},
                    ayudaDeDireccion = "",
                    errorDeDireccion = null,
                    nombre = "",
                    onNombre = {},
                    rubro = null,
                    onRubro = {},
                    email = "",
                    onEmail = {},
                    errorDeCorreo = null,
                    clave = "",
                    onClave = {},
                    errorDeClave = null,
                    onAtras = { atras = true },
                    onAvanzar = {},
                    onCrear = {},
                    onIrAEntrar = { entrando = true },
                    nube = false,
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Ese computador ya tiene un negocio creado").assertIsDisplayed()
        compose.onNodeWithText("Entrar a ese negocio").assertIsDisplayed().performClick()
        compose.waitForIdle()

        assertTrue("el botón no llevó a entrar", entrando)
        assertTrue("el botón se comportó como la flecha de atrás", !atras)
        revisarTactiles("200%, lugar ocupado local")
        revisarQueNadaSeCorte("200%, lugar ocupado local")
    }

    /**
     * En el APK de la nube el recuadro de "lugar ocupado" no puede hablar de
     * computador ni de dirección: el feriante no instaló nada en un PC.
     */
    @Test
    fun `el lugar ocupado de la nube no habla de computador ni direccion`() {
        var entrando = false

        compose.setContent {
            ConEscala(2.0f) {
                PantallaDeAlta(
                    paso = PasoDelAlta.Negocio,
                    indice = 0,
                    total = 3,
                    estado = EstadoDelAlta.LugarOcupado,
                    falla = null,
                    impedimento = null,
                    puedeAvanzar = false,
                    url = "",
                    onUrl = {},
                    ayudaDeDireccion = "",
                    errorDeDireccion = null,
                    nombre = "",
                    onNombre = {},
                    rubro = null,
                    onRubro = {},
                    email = "",
                    onEmail = {},
                    errorDeCorreo = null,
                    clave = "",
                    onClave = {},
                    errorDeClave = null,
                    onAtras = {},
                    onAvanzar = {},
                    onCrear = {},
                    onIrAEntrar = { entrando = true },
                    nube = true,
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Ese puesto ya está creado").assertIsDisplayed()
        // El copy local no puede colarse.
        compose.onNodeWithText("Ese computador ya tiene un negocio creado").assertDoesNotExist()
        compose.onNodeWithText("Entrar a ese puesto").assertIsDisplayed().performClick()
        compose.waitForIdle()
        assertTrue("el botón no llevó a entrar", entrando)
        revisarTactiles("200%, lugar ocupado nube")
        revisarQueNadaSeCorte("200%, lugar ocupado nube")
    }

    // --- las fallas ---------------------------------------------------------

    /**
     * Las cuatro fallas, dibujadas de verdad y al 200%.
     *
     * Son párrafos de hasta cuatro renglones en el peor caso: si alguna se
     * corta a lo ancho, la instrucción que sirve para arreglar el problema
     * queda a medias. Van en una sola composición porque `setContent` se puede
     * llamar una sola vez, y en un scroller porque al 200% no entran en 640dp
     * — lo que se mide es el ancho, que es donde no hay a dónde scrollear.
     */
    @Test
    fun `las cuatro fallas del alta se leen enteras al 200 por ciento`() {
        val donde = "http://192.168.1.10:8080"
        val fallas = listOf(
            FallaDeAlta.SinRed(),
            FallaDeAlta.ServicioNoResponde(donde),
            FallaDeAlta.CorreoYaTieneNegocio("rosa@almacen.cl", donde),
            FallaDeAlta.ClaveDebil(),
            FallaDeAlta.CreadoPeroNoEntro("almacen-dona-rosa"),
        )

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
                        // Misma tarjeta que pinta PantallaDeAlta: card, no muro rojo.
                        TarjetaDeFalla(
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
        revisarQueNadaSeCorte("200%, las cinco fallas del alta")
    }

    /**
     * Una falla en el último paso se ve sin que nadie tenga que buscarla, y el
     * botón sigue abajo.
     */
    @Test
    fun `la falla aparece a la vista en el ultimo paso al 200 por ciento`() {
        mostrar(2.0f, conNube = true, falla = FallaDeAlta.ClaveDebil())
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()

        // La columna del paso es un scroller normal, no un `LazyColumn`: el nodo
        // existe aunque esté fuera de cuadro y `performScrollTo` lo alcanza.
        compose.onNodeWithText("Esa clave es muy corta").performScrollTo().assertIsDisplayed()
        // Y el botón sigue abajo, sin haber tenido que buscarlo.
        compose.onNodeWithText("Crear mi negocio").assertIsDisplayed()
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
