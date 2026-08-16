package cl.rutbusiness.app.ui.fiado

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextInput
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
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * El fiado al 100% y al 200% de escala de letra.
 *
 * Se miden las dos pantallas que la dueña usa con alguien esperando en el
 * mostrador: la lista de quién debe (con su buscador) y el formulario del pago.
 * El caso del teclado numérico va con la pantalla corta (`h320dp`), que es lo
 * que queda de un panel de 640dp con el teclado arriba.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class FiadoEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    /**
     * Tres deudores con la forma exacta que devuelve `/reports/por-cobrar`, en
     * el orden en que los manda el server: mayor saldo primero.
     */
    private val deudores = DeudoresDto(
        total = "1235820",
        cuantos = 3,
        rows = listOf(
            DeudorDto(
                customer = "customer:marta",
                name = "Marta Díaz",
                phone = "+56933333333",
                balance = "1217910",
                ultimoMovimiento = "2026-08-07T14:37:12.869815500Z",
            ),
            DeudorDto(
                customer = "customer:carlos",
                name = "Carlos Soto",
                phone = "+56922222222",
                balance = "11940",
                ultimoMovimiento = "2026-08-06T10:00:00Z",
            ),
            DeudorDto(
                customer = "customer:juana",
                name = "Juana Pérez",
                phone = "+56911111111",
                balance = "5970",
                ultimoMovimiento = "2026-08-05T09:00:00Z",
            ),
        ),
    )

    // --- la lista ------------------------------------------------------------

    private fun mostrarLista(escala: Float, elegido: (DeudorDto) -> Unit = {}) {
        compose.setContent {
            var consulta by remember { mutableStateOf("") }
            ConEscala(escala) {
                ListaDeDeudores(
                    moneda = Moneda.de("CLP"),
                    deudores = deudores,
                    visibles = filtrar(consulta),
                    consulta = consulta,
                    onConsulta = { consulta = it },
                    onElegir = elegido,
                )
            }
        }
        compose.waitForIdle()
    }

    /** El mismo filtro que hace el `ViewModel`, para que la prueba mida la lista real. */
    private fun filtrar(consulta: String): List<DeudorDto> {
        val texto = consulta.trim().lowercase()
        if (texto.isEmpty()) return deudores.rows
        return deudores.rows.filter {
            it.name.lowercase().contains(texto) || it.phone.orEmpty().contains(texto)
        }
    }

    @Test
    fun `al 100 por ciento se lee la lista entera`() {
        mostrarLista(escala = 1.0f)
        revisarLista(1.0f)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `al 200 por ciento se lee la lista entera`() {
        mostrarLista(escala = 2.0f)
        revisarLista(2.0f)
    }

    private fun revisarLista(escala: Float) {
        // El total arriba, en grande, sin partirse aunque tenga siete dígitos.
        enLaLista("$1.235.820").assertIsDisplayed()
        // Gente que te debe, no "cuentas pendientes" de ledger.
        enLaLista("3 personas te deben.").assertIsDisplayed()

        // Y cada deudor con su nombre y su saldo, en el orden del server.
        listOf(
            "Marta Díaz" to "$1.217.910",
            "Carlos Soto" to "$11.940",
            "Juana Pérez" to "$5.970",
        ).forEach { (nombre, saldo) ->
            enLaLista(nombre).assertIsDisplayed()
            enLaLista(saldo).assertIsDisplayed()
            // Lo táctil y lo que se corta se revisan con la fila a la vista: en
            // una lista virtualizada, lo que no está en pantalla no existe como
            // nodo y no se puede medir.
            revisarTactiles(escala)
            revisarQueNadaSeCorte(escala)
        }
    }

    /** Tocar a alguien de la lista abre su cuenta, y el dedo tiene dónde caer. */
    @Test
    fun `tocar un deudor abre su cuenta`() {
        var abierto: DeudorDto? = null
        mostrarLista(escala = 2.0f, elegido = { abierto = it })

        enLaLista("Carlos Soto").performClick()
        compose.waitForIdle()

        assertEquals("customer:carlos", abierto?.customer)
    }

    /**
     * El buscador, que es la razón de que esta pantalla exista como pantalla.
     *
     * Quien llega a pagar está parado esperando: escribir tres letras tiene que
     * dejar una sola fila.
     */
    @Test
    fun `buscar por nombre deja solo a quien vino a pagar`() {
        mostrarLista(escala = 1.0f)

        compose.onNodeWithContentDescription("Buscar a quien vino a pagar")
            .performTextInput("carlos")
        compose.waitForIdle()

        compose.onNodeWithText("Carlos Soto").assertIsDisplayed()
        assertEquals(
            "los otros dos deudores no tenían que quedar en la lista",
            0,
            compose.onAllNodes(hasText("Marta Díaz")).fetchSemanticsNodes().size,
        )
        enLaLista("Se ven 1 de 3.").assertIsDisplayed()
    }

    @Test
    fun `sin deudores se explica que va a pasar cuando fie`() {
        mostrarSinDeudores(escala = 1.0f)

        compose.onNodeWithText("Nadie te debe plata").assertIsDisplayed()
        compose.onNodeWithText("Cuando fíes una venta", substring = true).assertIsDisplayed()
        // Sin deuda no hay a quién buscar: el vacío enseña, no filtra.
        assertEquals(
            0,
            compose.onAllNodes(hasContentDescription("Buscar a quien vino a pagar"))
                .fetchSemanticsNodes()
                .size,
        )
    }

    /**
     * El vacío que más veces se va a ver: el primer día nadie debe nada.
     *
     * En feria fiar no es una pantalla, es una frase — así que el vacío la
     * enseña completa (`EJEMPLO_FIADO_FERIA`), con nombre y monto, para que se
     * copie tal cual.
     */
    @Test
    fun `en feria sin deudores se ensena como fiar`() {
        var hablado = false
        mostrarSinDeudores(escala = 1.0f, rubro = "feria", onIrAlAgente = { hablado = true })

        compose.onNodeWithText("Nadie te debe ahora").assertIsDisplayed()
        compose
            .onNodeWithText(EJEMPLO_FIADO_FERIA, substring = true)
            .assertIsDisplayed()
        compose.onNodeWithText("Dile al agente", substring = true).assertIsDisplayed()

        compose.onNodeWithText("Hablarle al agente").performClick()
        compose.waitForIdle()
        assertTrue("el botón del vacío tiene que llevar a hablarle al agente", hablado)
    }

    /** El caso que define "listo" según el piso de hardware, también en el vacío. */
    @Test
    fun `en feria el vacio de deudas se lee entero al 200 por ciento`() {
        mostrarSinDeudores(escala = 2.0f, rubro = "feria", onIrAlAgente = {})

        compose.onNodeWithText("Nadie te debe ahora").assertIsDisplayed()
        val cta = compose.onNodeWithText("Hablarle al agente")
        cta.assertIsDisplayed()

        val caja = cta.getUnclippedBoundsInRoot()
        assertTrue(
            "a 2x el botón del vacío mide ${caja.width} x ${caja.height}, bajo el objetivo " +
                "táctil de $objetivoTactil",
            caja.height >= objetivoTactil,
        )
        revisarQueNadaSeCorte(2.0f)
    }

    /** Un rubro formal no hereda el copy de feria. */
    @Test
    fun `con pack formal el vacio de deudas no cambia`() {
        mostrarSinDeudores(escala = 1.0f, rubro = "farmacia", onIrAlAgente = {})

        compose.onNodeWithText("Nadie te debe plata").assertIsDisplayed()
        compose.onNodeWithText("Cuando fíes una venta", substring = true).assertIsDisplayed()
        assertEquals(
            "con un pack formal no se ofrece hablarle al agente",
            0,
            compose.onAllNodes(hasText("Hablarle al agente")).fetchSemanticsNodes().size,
        )
    }

    /**
     * Abrir la cuenta de alguien: el objetivo táctil, medido donde se toca.
     *
     * Es el gesto de "¿cuánto debe don Juan?" con don Juan parado adelante. Al
     * 200% la fila crece con la letra, y lo que no puede pasar es que crezca el
     * texto y no el área que recibe el dedo.
     */
    @Test
    fun `el toque para ver una cuenta mide al menos 56 dp al 200 por ciento`() {
        mostrarLista(escala = 2.0f)

        val fila = enLaLista("Marta Díaz")
        val caja = fila.getUnclippedBoundsInRoot()
        assertTrue(
            "a 2x la fila del deudor mide ${caja.width} x ${caja.height}, bajo el objetivo " +
                "táctil de $objetivoTactil",
            caja.height >= objetivoTactil,
        )
    }

    /** Ídem para anotar el pago, que es el otro toque con alguien esperando. */
    @Test
    fun `el toque para anotar el pago mide al menos 56 dp al 200 por ciento`() {
        mostrarAbono(escala = 2.0f)

        val boton = compose.onNodeWithText("Anotar el pago").performScrollTo()
        val caja = boton.getUnclippedBoundsInRoot()
        assertTrue(
            "a 2x «Anotar el pago» mide ${caja.width} x ${caja.height}, bajo el objetivo " +
                "táctil de $objetivoTactil",
            caja.height >= objetivoTactil,
        )
    }

    /**
     * La lista vacía, con el pack que se le quiera dar.
     *
     * @param rubro `null` es el caso histórico -sin servicios de rubro- y
     *   equivale al pack genérico: retail formal.
     */
    private fun mostrarSinDeudores(
        escala: Float,
        rubro: String? = null,
        onIrAlAgente: (() -> Unit)? = null,
    ) {
        val servicios = rubro?.let {
            ServiciosDeRubro(RubroPackRepository().apply { usarLocal(it) })
        }
        compose.setContent {
            CompositionLocalProvider(
                LocalRubro provides servicios,
                LocalIrAlAgente provides onIrAlAgente,
            ) {
                ConEscala(escala) {
                    ListaDeDeudores(
                        moneda = Moneda.de("CLP"),
                        deudores = DeudoresDto(total = "0", cuantos = 0, rows = emptyList()),
                        visibles = emptyList(),
                        consulta = "",
                        onConsulta = {},
                        onElegir = {},
                    )
                }
            }
        }
        compose.waitForIdle()
    }

    // --- el pago -------------------------------------------------------------

    private fun mostrarAbono(
        escala: Float,
        hayCaja: Boolean = true,
        esFeria: Boolean = false,
    ) {
        compose.setContent {
            var monto by remember { mutableStateOf("") }
            var nota by remember { mutableStateOf("") }
            var enEfectivo by remember { mutableStateOf(hayCaja || esFeria) }
            ConEscala(escala) {
                FormularioDeAbono(
                    moneda = Moneda.de("CLP"),
                    deudaActual = "1217910",
                    monto = monto,
                    onMonto = { monto = it },
                    nota = nota,
                    onNota = { nota = it },
                    hayCajaAbierta = hayCaja,
                    entraALaCaja = enEfectivo,
                    onEntraALaCaja = { enEfectivo = it },
                    esFeria = esFeria,
                    impedimento = if (monto.isBlank()) "Escribe cuánto te está pagando." else null,
                    error = null,
                    guardando = false,
                    onAnotar = {},
                    onVolver = {},
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun `feria abono en efectivo no habla de cajon y cuenta en el dia`() {
        mostrarAbono(escala = 1.0f, hayCaja = true, esFeria = true)
        compose.onNodeWithText("Esta plata cuenta en el día.").assertIsDisplayed()
        assertEquals(
            0,
            compose.onAllNodes(hasText("cajón", substring = true, ignoreCase = true))
                .fetchSemanticsNodes()
                .size,
        )
        compose.onNodeWithText("Anotar el pago").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun `al 100 por ciento el pago se usa entero`() {
        mostrarAbono(escala = 1.0f)
        revisarAbono(1.0f)
    }

    @Test
    fun `al 200 por ciento el pago se usa entero`() {
        mostrarAbono(escala = 2.0f)
        revisarAbono(2.0f)
    }

    private fun revisarAbono(escala: Float) {
        val campo = compose.onNodeWithContentDescription("Plata que te da").performScrollTo()
        campo.assertIsDisplayed()
        val caja = campo.getUnclippedBoundsInRoot()
        assertTrue(
            "a ${escala}x el campo del monto mide ${caja.width} x ${caja.height}, bajo el " +
                "objetivo táctil de $objetivoTactil",
            caja.height >= objetivoTactil,
        )

        compose.onNodeWithText("Debe $1.217.910.", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithText("Anotar el pago").performScrollTo().assertIsDisplayed()

        revisarQueNadaSeCorte(escala)
    }

    /**
     * El caso del teclado numérico, medido.
     *
     * `h320dp` es lo que queda del panel cuando el teclado numérico ocupa la
     * mitad de abajo. Si ahí el campo del monto y el botón siguen alcanzables, el
     * teclado no tapa nada. Lo que lo hace posible es la columna con
     * `verticalScroll` + `imePadding`.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba el campo del pago y el boton siguen alcanzables`() {
        mostrarAbono(escala = 1.0f)

        compose.onNodeWithContentDescription("Plata que te da").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Anotar el pago").performScrollTo().assertIsDisplayed()
    }

    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba y la letra al 200 por ciento tambien`() {
        mostrarAbono(escala = 2.0f)

        compose.onNodeWithContentDescription("Plata que te da").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Anotar el pago").performScrollTo().assertIsDisplayed()
    }

    /**
     * Sin caja abierta no se ofrece meter el billete a la caja.
     *
     * Ofrecerlo sería prometer que esa plata va a estar en un arqueo que no
     * existe.
     */
    @Test
    fun `sin caja abierta no se pregunta como paga`() {
        mostrarAbono(escala = 1.0f, hayCaja = false)

        assertEquals(
            0,
            compose.onAllNodes(hasText("¿Cómo te paga?")).fetchSemanticsNodes().size,
        )
    }

    @Test
    fun `con caja abierta el efectivo entra al cierre y la transferencia no`() {
        mostrarAbono(escala = 1.0f, hayCaja = true)

        compose.onNodeWithText("El billete entra a la caja y va a estar en el cierre de hoy.")
            .performScrollTo()
            .assertIsDisplayed()

        compose.onNodeWithText("Transferencia").performScrollTo().performClick()
        compose.waitForIdle()

        compose.onNodeWithText("No toca la caja", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    // --- herramientas --------------------------------------------------------

    /**
     * Trae a la vista una fila de la lista y la devuelve.
     *
     * `performScrollTo` no sirve dentro de un `LazyColumn`: lo que está fuera de
     * pantalla ni siquiera es un nodo todavía, así que hay que pedirle a la lista
     * que lo componga. Es la contracara de virtualizar, y virtualizar no es
     * opcional con el piso de hardware de este producto.
     */
    private fun enLaLista(texto: String): SemanticsNodeInteraction {
        compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasText(texto))
        return compose.onNodeWithText(texto)
    }

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

    private fun revisarTactiles(escala: Float) {
        val chicos = mutableListOf<String>()
        val nodos = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        nodos.indices.forEach { i ->
            val nodo = compose.onAllNodes(hasClickAction())[i]
            val caja = nodo.getUnclippedBoundsInRoot()
            if (caja.height < objetivoTactil) {
                val texto = nodo.fetchSemanticsNode()
                    .config.getOrNull(SemanticsProperties.Text)
                    ?.joinToString(" ") { it.text } ?: "sin texto"
                chicos += "«$texto» mide ${caja.width} x ${caja.height}"
            }
        }
        assertTrue(
            "a ${escala}x estos toques quedaron bajo $objetivoTactil:\n" +
                chicos.joinToString("\n"),
            chicos.isEmpty(),
        )
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
}
