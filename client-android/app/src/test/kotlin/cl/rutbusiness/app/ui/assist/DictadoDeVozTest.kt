package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.app.voz.AccionAlTocarVoz
import cl.rutbusiness.app.voz.accionAlTocarVoz
import cl.rutbusiness.core.rubro.PACK_OTRO
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Hablarle al puesto: el cableado del dictado, sin micrófono ni teléfono.
 *
 * Lo que estas pruebas sostienen no es que el `SpeechRecognizer` funcione -eso
 * es del aparato- sino las cuatro reglas que hacen que la voz **no rompa nada**:
 *
 * 1. Sin reconocedor no hay control de voz, y la pantalla es la de ayer.
 * 2. Con reconocedor, el control se puede tocar: 56 dp al 100% y al 200%.
 * 3. Lo dictado entra por el mismo camino que lo escrito, no por uno propio.
 * 4. Cuando el micrófono no se puede usar, el teclado sigue entero y nadie se
 *    lleva un susto.
 *
 * `NATIVE` porque acá se miden anchos y altos: con los gráficos por defecto de
 * Robolectric el motor de fuentes es un stub de ~0,5 dp por carácter y cualquier
 * aserción de tamaño pasaría sin significar nada. SDK 34 porque ahí vive la
 * curva **no lineal** de escala de letra que el usuario recibe de verdad.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class DictadoDeVozTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil: Dp = RbDefaultDimens.touchTarget

    @Before
    fun antes() {
        // El `viewModelScope` del agente sale a buscar la sesión. Sin un main
        // que corra ya, la prueba mediría el silencio anterior a ese trabajo.
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @After
    fun despues() {
        Dispatchers.resetMain()
    }

    // --- sin reconocedor --------------------------------------------------

    /**
     * Un teléfono sin motor de voz -o un test- no ve ningún micrófono, y eso no
     * es un caso raro que haya que blindar: es el estado por defecto del
     * `CompositionLocal`.
     */
    @Test
    fun `sin reconocedor no hay control de voz`() {
        montar(dictado = null)

        compose.onNodeWithText(ETIQUETA_QUIETO).assertDoesNotExist()
        compose.onNodeWithText(ETIQUETA_ESCUCHANDO).assertDoesNotExist()
    }

    /** Y el teclado sigue llegando al agente igual que siempre. */
    @Test
    fun `sin reconocedor se sigue escribiendo`() {
        val vm = montar(dictado = null)

        escribirYEnviar("¿cuánto vendí hoy?")

        assertEquals("¿cuánto vendí hoy?", primeraPregunta(vm))
    }

    // --- con reconocedor --------------------------------------------------

    /** El control existe, se ve, y el teclado no se fue a ninguna parte. */
    @Test
    fun `con reconocedor hay control de voz sin sacar el teclado`() {
        montar(DictadoFalso(reconoce = "vendí 2 kg de tomates"))

        botonDeVoz().assertIsDisplayed()
        compose.onNodeWithContentDescription(CAMPO).assertIsDisplayed()
        compose.onNodeWithText("Enviar").assertIsDisplayed()
    }

    /**
     * El piso del puesto: una mano, sol, y el dedo de alguien que no tiene
     * veinte años. 56 dp en los dos ejes, no los 48 de Material.
     */
    @Test
    fun `el control de voz se puede tocar al 100 por ciento`() {
        montar(DictadoFalso(reconoce = "hola"))
        revisarTactil(1.0f)
    }

    /** Y al 200%, que es cuando el campo y el botón se pelean el ancho. */
    @Test
    fun `el control de voz se puede tocar al 200 por ciento`() {
        montar(DictadoFalso(reconoce = "hola"), escala = 2.0f)
        revisarTactil(2.0f)
    }

    /** Mientras escucha lo dice con la palabra, no sólo con el dibujo. */
    @Test
    fun `mientras escucha el control lo dice`() {
        montar(DictadoFalso(reconoce = "vendí 2 kg de tomates", quedaEscuchando = true))

        botonDeVoz().performClick()
        compose.waitForIdle()

        compose.onNodeWithText(ETIQUETA_ESCUCHANDO).assertIsDisplayed()
    }

    // --- el mismo camino que el teclado -----------------------------------

    /**
     * La regla que más importa de este carril: **no hay un camino de voz**. Lo
     * reconocido cae en el campo y se manda como si se hubiera tipeado, así que
     * todo lo que ya protege a la dueña -la propuesta que hay que confirmar, el
     * reintento, el mensaje de sesión vencida- la protege igual hablando.
     */
    @Test
    fun `lo dictado se manda igual que lo escrito`() {
        val vm = montar(DictadoFalso(reconoce = "vendí 2 kg de tomates"))

        botonDeVoz().performClick()
        compose.waitForIdle()

        assertEquals("vendí 2 kg de tomates", primeraPregunta(vm))
        assertEquals("lo dictado no puede quedarse a medias en el campo", "", vm.borrador)
    }

    /** Dos toques, dos preguntas: nada se manda solo ni se manda dos veces. */
    @Test
    fun `un toque es una sola pregunta`() {
        val vm = montar(DictadoFalso(reconoce = "¿cuánto vendí hoy?"))

        botonDeVoz().performClick()
        compose.waitForIdle()

        assertEquals(
            "un toque tiene que dejar una sola pregunta",
            1,
            vm.mensajes.count { it is Mensaje.Mia },
        )
    }

    // --- cuando el micrófono no se puede usar -----------------------------

    /**
     * Permiso negado: no se manda nada, no se cae nada, y lo que se lee es una
     * línea que termina en cómo seguir. Nada de "error de voz".
     */
    @Test
    fun `sin permiso no se manda nada y se explica como seguir`() {
        val vm = montar(DictadoFalso(reconoce = null))

        botonDeVoz().performClick()
        compose.waitForIdle()

        assertTrue("un micrófono que no se puede usar no manda preguntas", vm.mensajes.isEmpty())
        compose.onNodeWithText("Puedes escribir igual", substring = true).assertIsDisplayed()
    }

    /** Y desde ahí mismo se puede seguir escribiendo, sin salir ni reintentar. */
    @Test
    fun `sin permiso el teclado sigue siendo camino`() {
        val vm = montar(DictadoFalso(reconoce = null))

        botonDeVoz().performClick()
        compose.waitForIdle()
        escribirYEnviar("¿quién me debe plata?")

        assertEquals("¿quién me debe plata?", primeraPregunta(vm))
    }

    /** Empezar a escribir baja el aviso: ya eligió el otro camino. */
    @Test
    fun `al escribir se baja el aviso del dictado`() {
        montar(DictadoFalso(reconoce = null))

        botonDeVoz().performClick()
        compose.waitForIdle()
        compose.onNodeWithContentDescription(CAMPO).performTextInput("dos kilos")
        compose.waitForIdle()

        compose.onNodeWithText("Puedes escribir igual", substring = true).assertDoesNotExist()
    }

    // --- lo que no se tocó -------------------------------------------------

    /**
     * Los chips siguen enteros. Son lo único que tiene una persona mayor frente
     * a un campo vacío, y agregar un micrófono no puede costarlos.
     */
    @Test
    fun `con microfono los chips de ejemplo siguen estando`() {
        montar(DictadoFalso(reconoce = "hola"))

        AssistSugerencias.para(PACK_OTRO).forEach { chip ->
            compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasText(chip))
            compose.onNode(hasText(chip) and hasClickAction()).assertIsDisplayed()
        }
    }

    // --- herramientas ------------------------------------------------------

    private fun botonDeVoz(): SemanticsNodeInteraction =
        compose.onNode(hasText(ETIQUETA_QUIETO) and hasClickAction())

    private fun revisarTactil(escala: Float) {
        val caja = botonDeVoz().assertIsDisplayed().getUnclippedBoundsInRoot()
        println("MEDIDA control-de-voz=${caja.width} x ${caja.height} escala=$escala")
        assertTrue(
            "a ${escala}x el control de voz mide ${caja.width} x ${caja.height}, " +
                "bajo $objetivoTactil",
            caja.height >= objetivoTactil && caja.width >= objetivoTactil,
        )
    }

    private fun escribirYEnviar(pregunta: String) {
        compose.onNodeWithContentDescription(CAMPO).performTextInput(pregunta)
        compose.onNodeWithText("Enviar").performClick()
        compose.waitForIdle()
    }

    private fun primeraPregunta(vm: AssistViewModel): String? =
        (vm.mensajes.firstOrNull { it is Mensaje.Mia } as? Mensaje.Mia)?.texto

    /**
     * Monta la pantalla entera del agente.
     *
     * Sin sesión guardada, `apiActiva()` devuelve `null` y cada pregunta termina
     * en un mensaje de sesión vencida: alcanza y sobra, porque lo que se mide acá
     * es **por dónde entró** la pregunta, no qué contestó el server.
     */
    private fun montar(dictado: DictadoDeVoz?, escala: Float = 1.0f): AssistViewModel {
        val app = RuntimeEnvironment.getApplication()
        val vm = AssistViewModel(SessionRepository(AlmacenamientoPlataforma(app)))

        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
                LocalDictadoDeVoz provides dictado,
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    AssistScreen(vm = vm, modifier = Modifier.fillMaxSize())
                }
            }
        }
        compose.waitForIdle()
        return vm
    }

    private companion object {
        /** La etiqueta del campo es lo que TalkBack lee, y lo que la prueba busca. */
        const val CAMPO = "¿Qué necesitas?"
    }
}

/**
 * Un micrófono de mentira.
 *
 * No imita al `SpeechRecognizer`: imita las tres cosas que la pantalla puede
 * recibir de él -entendió algo, no se pudo usar, o está escuchando- que son las
 * únicas que cambian lo que se dibuja.
 */
private class DictadoFalso(
    /** Qué "entiende" al tocarlo. `null` = acá no se pudo usar el micrófono. */
    private val reconoce: String?,
    /** Si el primer toque lo deja escuchando en vez de contestar de una. */
    private val quedaEscuchando: Boolean = false,
) : DictadoDeVoz {

    private var escuchandoAhora by mutableStateOf(false)
    private var avisoAhora by mutableStateOf<String?>(null)

    @Composable
    override fun recordarSesion(onTexto: (String) -> Unit): SesionDeDictado {
        val alTexto by rememberUpdatedState(onTexto)
        return remember {
            object : SesionDeDictado {
                override val escuchando: Boolean get() = escuchandoAhora

                // Este fake resuelve todo en el mismo toque -no simula el
                // tramo "cortó pero el reconocedor no contestó todavía"-, así
                // que acá siempre es `false`. Esa carrera la cubre
                // `AccionAlTocarVozTest` más abajo, sin Compose ni Robolectric
                // de por medio.
                override val procesando: Boolean get() = false

                override val aviso: String? get() = avisoAhora

                override fun alternar() {
                    when {
                        escuchandoAhora -> {
                            escuchandoAhora = false
                            reconoce?.let(alTexto)
                        }

                        reconoce == null -> avisoAhora = AVISO_SIN_PERMISO

                        quedaEscuchando -> escuchandoAhora = true

                        else -> alTexto(reconoce)
                    }
                }

                override fun olvidarAviso() {
                    avisoAhora = null
                }
            }
        }
    }

    private companion object {
        /** Mismo tono que el de verdad: termina en cómo seguir. */
        const val AVISO_SIN_PERMISO =
            "Para dictar tienes que dejarme usar el micrófono. Puedes escribir igual."
    }
}

/**
 * La carrera de la ola 23, aislada de Compose y de Robolectric.
 *
 * [DictadoDeVozTest] necesita los dos para montar la pantalla, pero esta
 * prueba no: `accionAlTocarVoz` es una función pura de tres booleanos a una
 * decisión, así que corre en JVM lisa y se puede verificar leyéndola sin
 * levantar Gradle.
 *
 * El bug que arregla: `DictadoDeVozAndroid` ponía `escuchando = false` apenas
 * se tocaba cortar, **antes** de que el reconocedor entregara `onResults()`.
 * El botón volvía a decir "Hablar" mientras el motor todavía procesaba lo
 * último dicho, así que un tercer toque rápido -`escuchando == false`- caía
 * en la rama de arrancar y abría una sesión nueva de `SpeechRecognizer` con
 * la anterior todavía en vuelo. El estado `procesando` cierra ese hueco: un
 * tercer toque ahí no hace nada, en vez de arrancar.
 */
class AccionAlTocarVozTest {

    @Test
    fun `escuchando corta, sin importar procesando ni el permiso`() {
        assertEquals(
            AccionAlTocarVoz.CORTAR,
            accionAlTocarVoz(escuchando = true, procesando = true, tienePermiso = true),
        )
        assertEquals(
            AccionAlTocarVoz.CORTAR,
            accionAlTocarVoz(escuchando = true, procesando = false, tienePermiso = false),
        )
    }

    /**
     * La ventana exacta de la carrera: ya cortó -`escuchando == false`- pero
     * el reconocedor no contestó. Antes del fix esto caía en `EMPEZAR`
     * -o en `PEDIR_PERMISO`, si además no había permiso- y arrancaba una
     * sesión nueva encima de la que seguía viva.
     */
    @Test
    fun `procesando ignora el toque en vez de arrancar una sesion encima de la que sigue viva`() {
        assertEquals(
            AccionAlTocarVoz.IGNORAR,
            accionAlTocarVoz(escuchando = false, procesando = true, tienePermiso = true),
        )
        assertEquals(
            AccionAlTocarVoz.IGNORAR,
            accionAlTocarVoz(escuchando = false, procesando = true, tienePermiso = false),
        )
    }

    @Test
    fun `quieto y con permiso empieza`() {
        assertEquals(
            AccionAlTocarVoz.EMPEZAR,
            accionAlTocarVoz(escuchando = false, procesando = false, tienePermiso = true),
        )
    }

    @Test
    fun `quieto y sin permiso lo pide`() {
        assertEquals(
            AccionAlTocarVoz.PEDIR_PERMISO,
            accionAlTocarVoz(escuchando = false, procesando = false, tienePermiso = false),
        )
    }
}
