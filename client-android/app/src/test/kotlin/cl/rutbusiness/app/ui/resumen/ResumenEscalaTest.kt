package cl.rutbusiness.app.ui.resumen

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
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.app.ui.agente.LocalIrAlAgente
import cl.rutbusiness.app.ui.rubro.LocalRubro
import cl.rutbusiness.app.ui.rubro.ServiciosDeRubro
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.rubro.RubroPackRepository
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
 * La cifra del día al 100% y al 200% de escala de letra.
 *
 * Un monto es una palabra sin espacios, así que cuando no entra Compose lo parte
 * por la mitad: "$1.234.567" se lee "$1.234." arriba y "567" abajo. La dueña
 * mira eso y no sabe si vendió un millón o mil doscientos. Por eso la cifra se
 * mide con montos de siete dígitos, que es el peor caso real de un negocio
 * chico en pesos, y en las dos monedas que el producto tiene que soportar hoy:
 * CLP sin decimales y una de dos decimales.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de texto mide ~0,5dp por carácter y toda aserción de ancho pasaría sin
 * significar nada. La pantalla es 360x640dp, el panel del aparato de referencia.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class ResumenEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    /**
     * @param rubro pack que se le da a la pantalla. `null` es el caso histórico
     *   -sin servicios de rubro- y equivale al pack genérico: retail formal.
     * @param onIrAlAgente cómo se llega al agente desde acá. `null` es el
     *   contenedor que todavía no lo provee, y ahí el vacío no dibuja botón.
     */
    private fun mostrar(
        escala: Float,
        moneda: Moneda = Moneda.POR_DEFECTO,
        vendidoHoy: String = "1234567",
        boletas: Long = 42L,
        comparacion: Comparacion = Comparacion.Mejor,
        vendidoAyer: String? = "987654",
        rubro: String? = null,
        onIrAlAgente: (() -> Unit)? = null,
    ) {
        val servicios = rubro?.let {
            ServiciosDeRubro(RubroPackRepository().apply { usarLocal(it) })
        }
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
                LocalRubro provides servicios,
                LocalIrAlAgente provides onIrAlAgente,
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    // La tarjeta vive en una lista que scrollea; el contenedor
                    // de la prueba imita eso para que "alcanzable" signifique lo
                    // mismo que en la pantalla real.
                    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        TarjetaDelDia(
                            moneda = moneda,
                            vendidoHoy = vendidoHoy,
                            boletas = boletas,
                            comparacion = comparacion,
                            vendidoAyer = vendidoAyer,
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    /** A lo ancho no hay a dónde scrollear: lo que no entra, se perdió. */
    private fun revisarQueNadaSeCorte(escala: Float) {
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
        assertTrue("a ${escala}x se corta:\n${cortados.joinToString("\n")}", cortados.isEmpty())
    }

    @Test
    fun `al 100 por ciento se lee la tarjeta entera`() {
        mostrar(escala = 1.0f)

        listOf(
            "Vendiste hoy",
            "$1.234.567",
            "42 boletas.",
            "Mejor que ayer",
            "Ayer, día completo: $987.654.",
        ).forEach { texto ->
            compose.onNodeWithText(texto, substring = true).performScrollTo().assertIsDisplayed()
        }
        revisarQueNadaSeCorte(1.0f)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `al 200 por ciento se lee la tarjeta entera`() {
        mostrar(escala = 2.0f)

        listOf(
            "Vendiste hoy",
            "$1.234.567",
            "42 boletas.",
            "Mejor que ayer",
            "Ayer, día completo: $987.654.",
        ).forEach { texto ->
            compose.onNodeWithText(texto, substring = true).performScrollTo().assertIsDisplayed()
        }
        revisarQueNadaSeCorte(2.0f)
    }

    /**
     * La cifra no puede quedar partida en dos líneas.
     *
     * Es lo que motiva que el multiplicador de tamaño se suelte con la letra
     * grande. Se mide contando líneas, no ancho: un monto que envuelve ocupa dos
     * renglones aunque no se "corte" en el sentido de quedar tapado.
     */
    @Test
    fun `la cifra del dia entra en una sola linea al 100 por ciento`() {
        mostrar(escala = 1.0f)
        revisarQueLaCifraNoSeParta(1.0f)
    }

    @Test
    fun `la cifra del dia entra en una sola linea al 200 por ciento`() {
        mostrar(escala = 2.0f)
        revisarQueLaCifraNoSeParta(2.0f)
    }

    private fun revisarQueLaCifraNoSeParta(escala: Float) {
        val lineas = compose.onNodeWithText("$1.234.567")
            .fetchSemanticsNode()
            .textLayoutLineCount()
        assertTrue(
            "a ${escala}x la cifra del día quedó en $lineas líneas: un monto partido " +
                "por la mitad se lee como otro monto",
            lineas == 1,
        )
    }

    /**
     * Una moneda de dos decimales no rompe nada.
     *
     * `USD 12345.67` es más largo que su equivalente en pesos y es el caso que
     * un tenant fuera de Chile recibe sin que nadie toque esta pantalla.
     */
    @Test
    fun `una moneda con decimales tambien entra al 200 por ciento`() {
        mostrar(
            escala = 2.0f,
            moneda = Moneda.de("USD"),
            vendidoHoy = "12345.67",
            vendidoAyer = "9876.54",
        )

        compose.onNodeWithText("US$12.345,67").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("US$9.876,54", substring = true).performScrollTo().assertIsDisplayed()
        revisarQueNadaSeCorte(2.0f)
    }

    /** Sin dato de ayer se dice, no se inventa una comparación. */
    @Test
    fun `sin lo de ayer la tarjeta lo dice`() {
        mostrar(escala = 1.0f, comparacion = Comparacion.SinDatoDeAyer, vendidoAyer = null)

        compose.onNodeWithText("Sin comparación").assertIsDisplayed()
        compose.onNodeWithText("No pudimos traer lo de ayer", substring = true).assertIsDisplayed()
    }

    /** Un día sin ventas no muestra "0 boletas" sino la frase que corresponde. */
    @Test
    fun `un dia sin ventas se dice con palabras`() {
        mostrar(escala = 1.0f, vendidoHoy = "0", boletas = 0L, comparacion = Comparacion.Peor)

        compose.onNodeWithText("Todavía no hay ninguna venta.").assertIsDisplayed()
        compose.onNodeWithText("$0").assertIsDisplayed()
    }

    // --- feria: el día antes de la primera venta ------------------------------

    /**
     * El vacío enseña el paso siguiente, no el estado.
     *
     * "$0 · 0 boletas · menos que ayer" es la misma nada dicha tres veces. Lo que
     * hace falta es la frase que se le dice al agente, con el ejemplo adentro:
     * la dueña copia el ejemplo, no la instrucción.
     */
    @Test
    fun `en feria un dia sin ventas ensena a anotar la primera`() {
        var hablado = false
        mostrar(
            escala = 1.0f,
            vendidoHoy = "0",
            boletas = 0L,
            comparacion = Comparacion.SinDatoDeAyer,
            vendidoAyer = null,
            rubro = "feria",
            onIrAlAgente = { hablado = true },
        )

        compose.onNodeWithText("Todavía no vendiste nada hoy").assertIsDisplayed()
        compose.onNodeWithText("vendí 2 kg de tomates a 2000", substring = true).assertIsDisplayed()

        compose.onNodeWithText("Hablarle al agente").performScrollTo().performClick()
        compose.waitForIdle()
        assertTrue("el botón del vacío tiene que llevar a hablarle al agente", hablado)
    }

    /** El caso que define "listo" según el piso de hardware, también en el vacío. */
    @Test
    fun `en feria el vacio del dia se lee entero al 200 por ciento`() {
        mostrar(
            escala = 2.0f,
            vendidoHoy = "0",
            boletas = 0L,
            comparacion = Comparacion.SinDatoDeAyer,
            vendidoAyer = null,
            rubro = "feria",
            onIrAlAgente = {},
        )

        compose.onNodeWithText("Todavía no vendiste nada hoy").performScrollTo().assertIsDisplayed()
        val cta = compose.onNodeWithText("Hablarle al agente").performScrollTo()
        cta.assertIsDisplayed()

        val caja = cta.getUnclippedBoundsInRoot()
        assertTrue(
            "a 2x el botón del vacío mide ${caja.width} x ${caja.height}, bajo el objetivo " +
                "táctil de $objetivoTactil",
            caja.height >= objetivoTactil,
        )
        revisarQueNadaSeCorte(2.0f)
    }

    /** Sin agente al alcance no se dibuja un botón que no lleva a ninguna parte. */
    @Test
    fun `sin manera de llegar al agente el vacio no ofrece boton`() {
        mostrar(
            escala = 1.0f,
            vendidoHoy = "0",
            boletas = 0L,
            comparacion = Comparacion.SinDatoDeAyer,
            vendidoAyer = null,
            rubro = "feria",
            onIrAlAgente = null,
        )

        compose.onNodeWithText("Todavía no vendiste nada hoy").assertIsDisplayed()
        assertTrue(
            "sin manera de llegar al agente el botón no va",
            compose.onAllNodes(hasText("Hablarle al agente")).fetchSemanticsNodes().isEmpty(),
        )
    }

    /**
     * Un rubro formal no hereda el copy de feria.
     *
     * Es la mitad que se rompe sola: quien toca el vacío de feria tiende a
     * mirarlo sólo con el pack de feria puesto, y la farmacia se entera después.
     */
    @Test
    fun `con pack formal un dia sin ventas sigue mostrando la cifra`() {
        mostrar(
            escala = 1.0f,
            vendidoHoy = "0",
            boletas = 0L,
            comparacion = Comparacion.Peor,
            rubro = "farmacia",
            onIrAlAgente = {},
        )

        compose.onNodeWithText("Vendiste hoy").assertIsDisplayed()
        compose.onNodeWithText("$0").assertIsDisplayed()
        compose.onNodeWithText("Todavía no hay ninguna venta.").assertIsDisplayed()
        assertTrue(
            "el copy de feria no puede aparecer con un pack formal",
            compose.onAllNodes(hasText("Todavía no vendiste nada hoy")).fetchSemanticsNodes()
                .isEmpty(),
        )
    }

    // --- feria: después de la primera venta (venta, no boleta) ----------------

    /**
     * Con ventas del día, feria dice «ventas» y nunca «boleta».
     *
     * La tarjeta ya no está en el vacío de HoySinVentas: es el camino que la
     * dueña ve todo el día, y el feriante no llama boleta a lo que anotó.
     */
    @Test
    fun `en feria con ventas se lee ventas y no boleta`() {
        mostrar(
            escala = 1.0f,
            vendidoHoy = "1234567",
            boletas = 42L,
            rubro = "feria",
        )

        compose.onNodeWithText("42 ventas.", substring = true).assertIsDisplayed()
        assertTrue(
            "feria no debe mostrar la palabra boleta en la tarjeta del día",
            compose.onAllNodes(hasText("boleta", substring = true)).fetchSemanticsNodes().isEmpty(),
        )
    }

    @Test
    fun `en feria una sola venta se dice en singular`() {
        mostrar(
            escala = 1.0f,
            vendidoHoy = "1500",
            boletas = 1L,
            rubro = "feria",
        )

        compose.onNodeWithText("1 venta.", substring = true).assertIsDisplayed()
        assertTrue(
            "singular de feria tampoco dice boleta",
            compose.onAllNodes(hasText("boleta", substring = true)).fetchSemanticsNodes().isEmpty(),
        )
    }

    /** La farmacia no hereda el vocabulario de feria en el conteo. */
    @Test
    fun `con pack formal el conteo sigue siendo boletas`() {
        mostrar(
            escala = 1.0f,
            vendidoHoy = "1234567",
            boletas = 42L,
            rubro = "farmacia",
        )

        compose.onNodeWithText("42 boletas.", substring = true).assertIsDisplayed()
        assertTrue(
            "retail formal no dice «ventas» en el conteo de la tarjeta",
            compose.onAllNodes(hasText("42 ventas.")).fetchSemanticsNodes().isEmpty(),
        )
    }
}

/**
 * Cuántas líneas ocupó el texto de un nodo.
 *
 * `SemanticsNode` no expone el `TextLayoutResult` directamente: hay que pedirle
 * el resultado a la acción `GetTextLayoutResult` que Compose instala en todo
 * `Text`.
 */
private fun androidx.compose.ui.semantics.SemanticsNode.textLayoutLineCount(): Int {
    val resultados = mutableListOf<androidx.compose.ui.text.TextLayoutResult>()
    config.getOrNull(androidx.compose.ui.semantics.SemanticsActions.GetTextLayoutResult)
        ?.action
        ?.invoke(resultados)
    return resultados.firstOrNull()?.lineCount ?: 0
}
