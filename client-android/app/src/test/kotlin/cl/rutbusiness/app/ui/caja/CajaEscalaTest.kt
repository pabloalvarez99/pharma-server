package cl.rutbusiness.app.ui.caja

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * La caja al 100% y al 200% de escala de letra, en el panel del aparato de
 * referencia.
 *
 * Dos pantallas se miden acá y las dos por el mismo motivo del piso de hardware:
 *
 * - **La diferencia del arqueo**, porque tiene números que no se pueden cortar
 *   ni confundir. Un monto es una palabra sin espacios: cuando no entra, Compose
 *   lo parte por la mitad y "$2.500" se lee como otra cosa.
 * - **El formulario del arqueo**, porque es el que se usa con el teclado
 *   numérico arriba. El caso del teclado se mide con la pantalla corta
 *   (`h320dp`), que es lo que queda de un panel de 640dp cuando el teclado
 *   numérico ocupa su mitad.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de texto mide ~0,5dp por carácter y toda aserción de ancho pasaría sin
 * significar nada.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class CajaEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    // --- la diferencia del cierre --------------------------------------------

    private fun mostrarDiferencia(
        escala: Float,
        moneda: Moneda = Moneda.de("CLP"),
        contado: String? = "15000",
        esperado: String? = "17500",
        discrepancia: String? = "-2500",
        nota: String? = null,
    ) {
        compose.setContent {
            ConEscala(escala, enUnScroller = true) {
                TarjetaDeDiferencia(
                    moneda = moneda,
                    contadoDelServidor = contado,
                    esperadoDelServidor = esperado,
                    discrepanciaDelServidor = discrepancia,
                    notaDeCierre = nota,
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun `al 100 por ciento se lee la diferencia entera`() {
        mostrarDiferencia(escala = 1.0f)
        revisarTextosDeLaDiferencia()
        revisarQueNadaSeCorte(1.0f)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `al 200 por ciento se lee la diferencia entera`() {
        mostrarDiferencia(escala = 2.0f)
        revisarTextosDeLaDiferencia()
        revisarQueNadaSeCorte(2.0f)
    }

    private fun revisarTextosDeLaDiferencia() {
        listOf(
            "Faltan $2.500",
            "Contaste $15.000 y el sistema tenía anotados $17.500.",
            "Casi siempre es un vuelto de más",
        ).forEach { texto ->
            compose.onNodeWithText(texto, substring = true).performScrollTo().assertIsDisplayed()
        }
    }

    /**
     * La cifra de la diferencia no puede quedar partida en dos líneas.
     *
     * Es exactamente el caso en que un número partido se lee como otro número, y
     * acá ese otro número sería la plata que supuestamente falta.
     */
    @Test
    fun `la cifra de la diferencia entra en una linea al 100 por ciento`() {
        mostrarDiferencia(escala = 1.0f)
        assertEquals(1, lineasDe("Faltan $2.500"))
    }

    @Test
    fun `la cifra de la diferencia entra en una linea al 200 por ciento`() {
        mostrarDiferencia(escala = 2.0f)
        assertEquals(1, lineasDe("Faltan $2.500"))
    }

    /**
     * Millones al 200%: el peor caso real de un negocio que factura en pesos.
     *
     * Acá la frase sí puede envolver -- tiene un espacio donde partirse -- pero
     * el monto no: "$1.234.568" en dos renglones se lee como otro monto. Se mide
     * que quede en dos líneas como mucho, que es "Faltan" arriba y la cifra
     * entera abajo.
     */
    @Test
    fun `una diferencia de siete digitos no parte el monto al 200 por ciento`() {
        mostrarDiferencia(
            escala = 2.0f,
            contado = "3765432",
            esperado = "5000000",
            discrepancia = "-1234568",
        )
        assertTrue(
            "la frase de la diferencia ocupó más de dos líneas: el monto quedó partido",
            lineasDe("Faltan $1.234.568") <= 2,
        )
        revisarQueNadaSeCorte(2.0f)
    }

    /** Un tenant en dólares lee dólares sin que nadie toque esta pantalla. */
    @Test
    fun `una moneda con decimales se lee entera al 200 por ciento`() {
        mostrarDiferencia(
            escala = 2.0f,
            moneda = Moneda.de("USD"),
            contado = "150.00",
            esperado = "175.00",
            discrepancia = "-25.00",
        )
        compose.onNodeWithText("Faltan US$25,00").performScrollTo().assertIsDisplayed()
        revisarQueNadaSeCorte(2.0f)
    }

    @Test
    fun `cuando cuadra no se muestra un cero grande`() {
        mostrarDiferencia(escala = 1.0f, contado = "17500", discrepancia = "0")
        compose.onNodeWithText("La caja cuadró").assertIsDisplayed()
        assertEquals(
            "cuadrando no puede haber una cifra grande: el cero no aporta nada",
            0,
            compose.onAllNodes(hasText("$0")).fetchSemanticsNodes().size,
        )
    }

    @Test
    fun `la nota del cierre se muestra cuando la escribio`() {
        mostrarDiferencia(escala = 1.0f, nota = "le di mal el vuelto a un cliente")
        compose.onNodeWithText("Anotaste: «le di mal el vuelto a un cliente»")
            .performScrollTo()
            .assertIsDisplayed()
    }

    // --- el formulario del arqueo --------------------------------------------

    private fun mostrarArqueo(escala: Float, moneda: Moneda = Moneda.de("CLP")) {
        compose.setContent {
            // `remember`, no `mutableStateOf` a secas: sin él el estado se crea
            // de nuevo en cada recomposición y lo tipeado se borra solo. Lo que
            // la prueba estaría midiendo entonces es un campo que no anda.
            var monto by remember { mutableStateOf("") }
            var nota by remember { mutableStateOf("") }
            ConEscala(escala) {
                FormularioDeArqueo(
                    moneda = moneda,
                    montoContado = monto,
                    onMontoContado = { monto = it },
                    nota = nota,
                    onNota = { nota = it },
                    impedimento = if (monto.isBlank()) "Cuenta la plata del cajón" else null,
                    error = null,
                    guardando = false,
                    onCerrar = {},
                    onVolver = {},
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun `al 100 por ciento el arqueo se usa entero`() {
        mostrarArqueo(escala = 1.0f)
        revisarQueElArqueoSeaUsable(1.0f)
    }

    @Test
    fun `al 200 por ciento el arqueo se usa entero`() {
        mostrarArqueo(escala = 2.0f)
        revisarQueElArqueoSeaUsable(2.0f)
    }

    /**
     * El caso del teclado numérico, medido.
     *
     * `h320dp` es lo que queda del panel de 640dp cuando el teclado numérico
     * ocupa la mitad de abajo. Si en esa altura el campo del monto y el botón de
     * cerrar siguen siendo alcanzables, el teclado no tapa nada — que es el error
     * clásico de esta pantalla y el que la arruina entera. Lo que lo hace posible
     * es la columna con `verticalScroll` + `imePadding`: sin eso, el botón queda
     * fuera de la pantalla y no hay a dónde scrollear.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba el campo del monto y el boton siguen alcanzables`() {
        mostrarArqueo(escala = 1.0f)

        compose.onNodeWithContentDescription("Plata contada").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Cerrar la caja").performScrollTo().assertIsDisplayed()
    }

    /** Y con la letra al doble, que es cuando queda todavía menos lugar. */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba y la letra al 200 por ciento tambien`() {
        mostrarArqueo(escala = 2.0f)

        compose.onNodeWithContentDescription("Plata contada").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Cerrar la caja").performScrollTo().assertIsDisplayed()
    }

    /** Lo que se tipea se ve. El filtro de lo que no es plata vive en
     *  `PlataEscritaTest`, que es donde está el código que filtra. */
    @Test
    fun `lo que se escribe en el campo del monto se ve`() {
        mostrarArqueo(escala = 1.0f)

        compose.onNodeWithContentDescription("Plata contada").performTextInput("15000")
        compose.waitForIdle()

        compose.onNodeWithText("15000").assertIsDisplayed()
    }

    /** Cerrar no puede pasar de un toque accidental: primero se pregunta. */
    @Test
    fun `cerrar la caja pregunta antes`() {
        mostrarArqueo(escala = 1.0f)

        compose.onNodeWithContentDescription("Plata contada").performTextInput("15000")
        compose.waitForIdle()
        compose.onNodeWithText("Cerrar la caja").performScrollTo().performClick()
        compose.waitForIdle()

        compose.onNodeWithText("¿Cerramos la caja?").assertIsDisplayed()
        compose.onNodeWithText("Sí, cerrar").assertIsDisplayed()
        compose.onNodeWithText("Seguir contando").assertIsDisplayed()
    }

    private fun revisarQueElArqueoSeaUsable(escala: Float) {
        val campos = listOf("Plata contada", "Nota del cierre (opcional)")
        campos.forEach { etiqueta ->
            val nodo = compose.onNodeWithContentDescription(etiqueta).performScrollTo()
            nodo.assertIsDisplayed()
            val caja = nodo.getUnclippedBoundsInRoot()
            assertTrue(
                "a ${escala}x el campo «$etiqueta» mide ${caja.width} x ${caja.height}, " +
                    "bajo el objetivo táctil de $objetivoTactil",
                caja.height >= objetivoTactil,
            )
        }

        listOf("Cerrar la caja", "Todavía no").forEach { boton ->
            compose.onNodeWithText(boton).performScrollTo().assertIsDisplayed()
        }

        revisarQueNadaSeCorte(escala)
    }

    // --- herramientas --------------------------------------------------------

    /**
     * Monta contenido con la escala de letra pedida.
     *
     * @param enUnScroller para lo que **no** trae su propio scroll. La tarjeta de
     *   la diferencia vive dentro de una columna que scrollea, así que la prueba
     *   la imita para que "alcanzable" signifique lo mismo que en la pantalla
     *   real. El formulario del arqueo, en cambio, ya scrollea solo, y meterlo
     *   dentro de otro scroller anidaría dos y `performScrollTo` dejaría de medir
     *   lo que la pantalla hace de verdad.
     */
    @Composable
    private fun ConEscala(
        escala: Float,
        enUnScroller: Boolean = false,
        contenido: @Composable () -> Unit,
    ) {
        val base = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(base.density, fontScale = escala),
        ) {
            RbTheme(darkTheme = true, reducedMotion = true) {
                if (enUnScroller) {
                    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) { contenido() }
                } else {
                    Box(Modifier.fillMaxSize()) { contenido() }
                }
            }
        }
    }

    /**
     * A lo ancho no hay a dónde scrollear: lo que no entra, se perdió.
     *
     * Es la misma revisión que hace `ResumenEscalaTest` y por el mismo motivo:
     * un texto cortado a la derecha no se nota en un emulador grande y es
     * exactamente lo que ve la persona que subió la letra.
     */
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

    private fun lineasDe(texto: String): Int =
        compose.onNodeWithText(texto).fetchSemanticsNode().lineasDeTexto()
}

/** Cuántas líneas ocupó el texto de un nodo. */
private fun SemanticsNode.lineasDeTexto(): Int {
    val resultados = mutableListOf<TextLayoutResult>()
    config.getOrNull(SemanticsActions.GetTextLayoutResult)?.action?.invoke(resultados)
    return resultados.firstOrNull()?.lineCount ?: 0
}

