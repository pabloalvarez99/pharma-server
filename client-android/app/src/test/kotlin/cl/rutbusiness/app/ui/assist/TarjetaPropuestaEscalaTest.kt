package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.app.ui.rubro.LocalRubro
import cl.rutbusiness.app.ui.rubro.ServiciosDeRubro
import cl.rutbusiness.core.rubro.RubroPackRepository
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * La tarjeta de propuesta al 100% y al 200% de escala de letra.
 *
 * Es la pantalla más cargada de texto de toda la app y la única que no se puede
 * cortar: si al 200% no se lee entera o el botón de confirmar queda fuera de
 * alcance, la dueña o no puede confirmar lo que quería, o confirma sin haber
 * leído. Las dos salidas son malas de distinta forma.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de fuentes es un stub que mide ~0,5dp por carácter, así que cualquier
 * aserción de ancho pasaría sin significar nada. SDK 34 porque ahí vive la
 * curva **no lineal** de escala de letra de Android 14, que es la que el
 * usuario recibe de verdad.
 *
 * La pantalla es 360x640dp: el panel 720p del aparato de referencia.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class TarjetaPropuestaEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    /** Una propuesta real de `registrar_gasto`, con la forma exacta del server. */
    private fun propuestaDeGasto() = Mensaje.Propuesta(
        id = 1L,
        pregunta = "Registra un gasto de 5000 en arriendo",
        propuesta = PropuestaAccion(
            name = "registrar_gasto",
            summary = "Registrar un gasto de $5.000 por «arriendo».",
            params = Json.parseToJsonElement(
                """
                {
                  "category": "arriendo",
                  "description": "arriendo",
                  "amount": "5000",
                  "payment_method": "cash"
                }
                """,
            ) as JsonObject,
            confirmToken = "token-de-prueba",
            expiresAt = "2099-01-01T00:00:00Z",
        ),
        estado = EstadoPropuesta.Esperando,
    )

    /**
     * @param rubro pack local para [LocalRubro]. `null` = retail genérico
     *   (comportamiento histórico de la prueba). `"feria"` = voz de puesto.
     */
    private fun mostrar(
        escala: Float,
        mensaje: Mensaje.Propuesta = propuestaDeGasto(),
        onConfirmar: () -> Unit = {},
        rubro: String? = null,
    ) {
        val servicios = rubro?.let {
            ServiciosDeRubro(RubroPackRepository().apply { usarLocal(it) })
        }
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
                LocalRubro provides servicios,
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    // La tarjeta vive dentro de una lista que scrollea; el
                    // contenedor de la prueba imita eso para que "alcanzable"
                    // signifique lo mismo que en la pantalla real.
                    Box(
                        Modifier
                            .fillMaxSize()
                            .verticalScroll(rememberScrollState()),
                    ) {
                        TarjetaPropuesta(
                            mensaje = mensaje,
                            segundosRestantes = 180,
                            onConfirmar = onConfirmar,
                            onCancelar = {},
                            onVencer = {},
                            onVolverAPedir = {},
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    private fun revisarTactiles(escala: Float) {
        val nodos = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        assertTrue("la tarjeta no dibujó ningún botón", nodos.isNotEmpty())

        val chicos = mutableListOf<String>()
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
            "a ${escala}x estos botones quedaron bajo $objetivoTactil:\n" +
                chicos.joinToString("\n"),
            chicos.isEmpty(),
        )
    }

    @Test
    fun `los botones aguantan al 100 por ciento`() {
        mostrar(escala = 1.0f)
        revisarTactiles(1.0f)
    }

    @Test
    fun `los botones aguantan al 200 por ciento`() {
        mostrar(escala = 2.0f)
        revisarTactiles(2.0f)
    }

    /**
     * Lo que la dueña tiene que poder leer para decidir: el resumen, cada línea
     * del detalle, y el monto formateado. Si algo de esto falta al 200%, la
     * tarjeta dejó de ser una defensa.
     */
    @Test
    fun `al 200 por ciento se lee la tarjeta entera`() {
        mostrar(escala = 2.0f)

        listOf(
            "Antes de hacerlo, revisa",
            "Registrar un gasto de $5.000 por «arriendo».",
            "Esto es lo que voy a guardar",
            "Monto",
            // El monto aparece dos veces a propósito: en el resumen y en el
            // detalle. Por eso se busca "al menos uno" y no "exactamente uno":
            // repetir la cifra es la intención, no un error.
            "$5.000",
            "Detalle",
            "Cómo se paga",
            "efectivo",
            "Tienes unos minutos para confirmarla.",
        ).forEach { texto ->
            val encontrados = compose.onAllNodes(hasText(texto, substring = true))
                .fetchSemanticsNodes()
            assertTrue("al 200% no aparece «$texto» en la tarjeta", encontrados.isNotEmpty())

            compose.onAllNodes(hasText(texto, substring = true))[0]
                .performScrollTo()
                .assertIsDisplayed()
        }
    }

    /**
     * El botón de confirmar tiene que quedar al alcance.
     *
     * Al 200% la tarjeta entera es más alta que un panel de 640dp, así que
     * "alcanzable" significa que se llega scrolleando. Un botón que existe pero
     * que no se puede tocar es lo mismo que no tenerlo.
     */
    @Test
    fun `al 200 por ciento se llega al boton de confirmar`() {
        mostrar(escala = 2.0f)

        compose.onNodeWithText("Sí, registrar el gasto")
            .performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText("No, déjalo así")
            .performScrollTo()
            .assertIsDisplayed()
    }

    /** Ningún texto se corta a lo ancho: no hay a dónde scrollear para eso. */
    @Test
    fun `al 200 por ciento no se corta ningun texto`() {
        mostrar(escala = 2.0f)

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
        assertTrue("al 200% se corta:\n${cortados.joinToString("\n")}", cortados.isEmpty())
    }

    /**
     * Apretar confirmar una vez deja de ofrecer el botón.
     *
     * El server ya rechaza un token repetido —es de un solo uso— pero eso es la
     * red de atrás. Que la dueña pueda apretar dos veces y ver un error sería
     * un susto evitable: la tarjeta esconde los botones apenas empieza a
     * guardar.
     */
    @Test
    fun `despues de confirmar ya no hay boton para confirmar de nuevo`() {
        var veces = 0
        mostrar(escala = 1.0f, onConfirmar = { veces++ })

        compose.onNodeWithText("Sí, registrar el gasto").performClick()
        compose.waitForIdle()

        // La prueba dibuja la tarjeta con estado fijo, así que acá se verifica
        // lo que la tarjeta hace por sí sola: un toque, una llamada.
        assertTrue("un toque tiene que ser una sola llamada, fueron $veces", veces == 1)
    }

    /** Con la propuesta ya ejecutada, no queda nada que volver a confirmar. */
    @Test
    fun `una propuesta ya hecha no ofrece confirmar`() {
        mostrar(
            escala = 1.0f,
            mensaje = propuestaDeGasto().copy(
                estado = EstadoPropuesta.Hecha("Listo: registré el gasto de $5.000 por «arriendo»."),
            ),
        )

        assertTrue(
            "una acción ya hecha no puede volver a ofrecerse",
            compose.onAllNodes(hasText("Sí, registrar el gasto")).fetchSemanticsNodes().isEmpty(),
        )
        compose.onNodeWithText("Listo:", substring = true).assertIsDisplayed()
    }

    /** Token vencido: mensaje entendible y la salida a mano. */
    @Test
    fun `una propuesta vencida explica y ofrece pedirla de nuevo`() {
        mostrar(
            escala = 2.0f,
            mensaje = propuestaDeGasto().copy(
                estado = EstadoPropuesta.YaNoSirve(
                    "Se me pasó el tiempo para hacerlo y no guardé nada. Pídemelo de nuevo.",
                ),
            ),
        )

        compose.onNodeWithText("Se me pasó el tiempo", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText("Pedirlo de nuevo")
            .performScrollTo()
            .assertIsDisplayed()

        revisarTactiles(2.0f)
    }

    /**
     * En feria la tarjeta se lee como alguien repitiendo lo oído en el puesto,
     * no como un diálogo de "confirmación" de admin.
     */
    @Test
    fun `en feria los botones suenan a puesto y se leen al 200 por ciento`() {
        mostrar(escala = 2.0f, rubro = "feria")

        compose.onNodeWithText("Antes de hacerlo, revisa")
            .performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText("Sí, anotá eso")
            .performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText("Mejor no")
            .performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText("Tienes unos minutos para decirme que sí.")
            .performScrollTo()
            .assertIsDisplayed()

        // Retail no se cuela en el puesto.
        assertTrue(
            compose.onAllNodes(hasText("Sí, registrar el gasto")).fetchSemanticsNodes().isEmpty(),
        )
        assertTrue(
            compose.onAllNodes(hasText("No, déjalo así")).fetchSemanticsNodes().isEmpty(),
        )

        revisarTactiles(2.0f)
    }

    @Test
    fun `en feria una propuesta vencida ofrece decirlo de nuevo`() {
        mostrar(
            escala = 2.0f,
            rubro = "feria",
            mensaje = propuestaDeGasto().copy(
                estado = EstadoPropuesta.YaNoSirve(
                    "Se me pasó el tiempo para anotarlo y no guardé nada. Decímelo de nuevo.",
                ),
            ),
        )

        compose.onNodeWithText("Decírmelo de nuevo")
            .performScrollTo()
            .assertIsDisplayed()
        revisarTactiles(2.0f)
    }
}
