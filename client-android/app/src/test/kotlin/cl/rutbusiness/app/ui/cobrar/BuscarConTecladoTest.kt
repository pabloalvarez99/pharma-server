package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTopBar
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
 * Buscar un producto con el teclado arriba.
 *
 * La pantalla que la cajera usa doscientas veces al día tenía un defecto que
 * sólo aparece **mientras se escribe**: con el teclado abierto en el panel de
 * 720x1280 del aparato de referencia, la lista de resultados quedaba en unos
 * pocos píxeles. El código de barras se resolvía y el producto no se veía hasta
 * cerrar el teclado — o sea, justo cuando la cajera está buscando es cuando no
 * puede ver lo que encontró.
 *
 * El teclado se simula acortando el panel (`h320dp`), que es el mecanismo
 * canónico del repo — `CajaEscalaTest` mide el arqueo igual. Y la escala de
 * letra entra por `LocalDensity`, que es lo que Compose lee de verdad: en un
 * emulador `settings put system font_scale 2.0` **sí** llega al render, pero
 * sólo después de `adb reboot`, y sin reiniciar las capturas a 1.0 y a 2.0
 * salen idénticas y uno concluye que la app ignora la escala.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de texto mide ~0,5dp por carácter y todo lo que se mida acá sería ficción.
 * `sdk = 34` porque ahí vive la curva **no lineal** de la escala de letra, que
 * es la que recibe el usuario.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class BuscarConTecladoTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil: Dp = RbDefaultDimens.touchTarget

    // --- con el teclado abierto ----------------------------------------------

    /**
     * El caso del encargo: teclado arriba, escribiendo, al 100%.
     *
     * La medida que importa no es "la lista existe" sino "cuánto alto tiene":
     * antes de este arreglo medía **0dp**, y aun así estaba en el árbol y
     * respondía a `performScrollTo`. El piso es una fila entera — 56dp —
     * porque una fila a medias no se puede tocar.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba la lista de resultados se ve al 100 por ciento`() {
        montar(escala = 1.0f)
        escribir("arroz")

        val alto = altoDeLaLista()
        assertTrue(
            "con el teclado abierto la lista mide $alto: no entra ni una fila de $objetivoTactil",
            alto >= objetivoTactil,
        )
        compose.onNodeWithText("Arroz 1 kg").assertIsDisplayed()
    }

    /** Y con la letra al doble, que es cuando queda todavía menos lugar. */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba la lista de resultados se ve al 200 por ciento`() {
        montar(escala = 2.0f)
        escribir("arroz")

        val alto = altoDeLaLista()
        assertTrue(
            "a 2x con el teclado abierto la lista mide $alto: no entra ni una " +
                "fila de $objetivoTactil",
            alto >= objetivoTactil,
        )
        compose.onNodeWithText("Arroz 1 kg").assertIsDisplayed()
    }

    /**
     * El total y el botón de cobrar están anclados abajo, siempre.
     *
     * `assertIsDisplayed` y no `performScrollTo().assertIsDisplayed()` a
     * propósito: la barra no scrollea con la lista, y una prueba que scrollea
     * antes de mirar dejaría pasar exactamente el bug que se está arreglando.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba el total y cobrar siguen visibles al 100 por ciento`() {
        montar(escala = 1.0f, unidades = 3, total = "$4.470")
        escribir("arroz")
        revisarLaBarraDeTotal(1.0f)
    }

    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba el total y cobrar siguen visibles al 200 por ciento`() {
        montar(escala = 2.0f, unidades = 3, total = "$4.470")
        escribir("arroz")
        revisarLaBarraDeTotal(2.0f)
    }

    /**
     * El campo de búsqueda sigue estando mientras se escribe.
     *
     * El campo vive fuera de la lista justamente para esto: si dependiera de
     * la rama "hay resultados", desaparecería en cuanto la búsqueda no
     * encuentra nada — o sea, en la mitad de las teclas.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `el campo de busqueda no desaparece cuando no hay resultados`() {
        montar(escala = 1.0f, resultados = emptyList())
        escribir("xyz")

        compose.onNodeWithContentDescription("Buscar producto").assertIsDisplayed()
        botonDeCobrar().assertIsDisplayed()
    }

    /**
     * La barra de título cede sólo cuando no cabe.
     *
     * Es la regla que hace posible todo lo de arriba, así que se mide en los dos
     * sentidos: con el teclado abierto al 200% no está -no entraría el botón
     * Cobrar-, y con el panel entero sí está, con "Salir" adentro.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba al 200 por ciento la barra de titulo cede`() {
        montar(escala = 2.0f)
        compose.onNodeWithText("Busca el producto y agrégalo").assertDoesNotExist()
        botonDeCobrar().assertIsDisplayed()
    }

    @Test
    fun `con el panel entero la barra de titulo esta`() {
        montar(escala = 2.0f)
        compose.onNodeWithText("Busca el producto y agrégalo").assertIsDisplayed()
        compose.onNodeWithText("Salir").assertIsDisplayed()
    }

    // --- sin teclado ---------------------------------------------------------

    /**
     * Con el panel entero -barra de título puesta, ayuda y botón de la cámara a
     * la vista- la lista se lleva igual la mayor parte. Es el control de que
     * recortar es la excepción del teclado y no la forma normal de la pantalla.
     */
    @Test
    fun `sin teclado la lista se lleva el alto que sobra`() {
        montar(escala = 1.0f)

        compose.onNodeWithText("Busca el producto y agrégalo").assertIsDisplayed()
        val alto = altoDeLaLista()
        assertTrue("sin teclado la lista mide $alto, menos de 200dp", alto >= 200.dp)
        compose.onNodeWithText("Arroz 1 kg").assertIsDisplayed()
    }

    // --- herramientas --------------------------------------------------------

    private fun revisarLaBarraDeTotal(escala: Float) {
        compose.onNodeWithText("3 productos", substring = true).assertIsDisplayed()

        val cobrar = botonDeCobrar()
        cobrar.assertIsDisplayed()
        val caja = cobrar.getBoundsInRoot()
        assertTrue(
            "a ${escala}x el botón Cobrar se ve ${caja.height} de alto, bajo el objetivo " +
                "táctil de $objetivoTactil: no se puede tocar",
            caja.height >= objetivoTactil,
        )
    }

    /**
     * El botón, no el título.
     *
     * La barra de arriba también dice "Cobrar" -es el nombre de la pantalla-,
     * así que buscar por texto a secas trae dos nodos y la prueba muere sin
     * medir nada. El botón es el que se puede tocar.
     */
    private fun botonDeCobrar(): SemanticsNodeInteraction =
        compose.onNode(hasText("Cobrar") and hasClickAction())

    /** El alto **visible** de la lista virtualizada. */
    private fun altoDeLaLista(): Dp {
        val lista = compose.onNode(hasScrollToNodeAction()).getBoundsInRoot()
        val pantalla = compose.onRoot().getUnclippedBoundsInRoot()
        // Va a stdout y queda en el XML del reporte: es el número que se cita
        // cuando alguien pregunta "¿cuánto mide la lista con el teclado arriba?".
        println(
            "MEDIDA lista=${lista.height} (arranca en ${lista.top}) " +
                "pantalla=${pantalla.height}",
        )
        return lista.height
    }

    private fun escribir(texto: String) {
        compose.onNodeWithContentDescription("Buscar producto").performTextInput(texto)
        compose.waitForIdle()
    }

    /**
     * Monta el paso de buscar **con la barra de título arriba**, igual que
     * `CobrarScreen`: el alto que le queda a la lista depende de lo que se haya
     * llevado esa barra, y medir el paso solo daría un número que la cajera
     * nunca ve.
     */
    @OptIn(ExperimentalLayoutApi::class)
    private fun montar(
        escala: Float,
        resultados: List<ProductDto> = catalogo,
        unidades: Int = 0,
        total: String? = null,
    ) {
        compose.setContent {
            var consulta by remember { mutableStateOf("") }
            ConEscala(escala) {
                BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
                val alto = maxHeight
                Column(modifier = Modifier.fillMaxSize()) {
                    // `cabeLaBarraDeTitulo` es **la** función de producción, no
                    // una copia: si la regla cambia, esta prueba cambia con
                    // ella en vez de seguir midiendo una pantalla que ya no
                    // existe.
                    if (cabeLaBarraDeTitulo(alto)) {
                        RbTopBar(
                            title = "Cobrar",
                            subtitle = "Busca el producto y agrégalo",
                            actions = {
                                FlowRow(
                                    horizontalArrangement = Arrangement.spacedBy(
                                        RbTheme.dimens.space2,
                                    ),
                                    verticalArrangement = Arrangement.spacedBy(
                                        RbTheme.dimens.space1,
                                    ),
                                ) {
                                    RbButton(
                                        label = "Salir",
                                        onClick = {},
                                        variant = RbButtonVariant.Secondary,
                                    )
                                }
                            },
                        )
                    }
                    BuscarContenido(
                        modifier = Modifier.weight(1f),
                        consulta = consulta,
                        onConsulta = { consulta = it },
                        ayuda = "Escribe parte del nombre, o el código de barras completo.",
                        onEscanear = {},
                        resultados = resultados,
                        buscando = false,
                        errorBusqueda = null,
                        onBuscarAhora = {},
                        cantidadEnCarrito = { 0 },
                        precioDe = { "$1.490" },
                        onAgregar = {},
                        unidades = unidades,
                        total = total,
                        onCobrar = {},
                    )
                }
                }
            }
        }
        compose.waitForIdle()
    }

    @Composable
    private fun ConEscala(escala: Float, contenido: @Composable () -> Unit) {
        val base = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(base.density, fontScale = escala),
        ) {
            RbTheme(darkTheme = true, reducedMotion = true) { contenido() }
        }
    }

    private val catalogo: List<ProductDto> = listOf(
        "Arroz 1 kg",
        "Aceite maravilla 1 L",
        "Azúcar 1 kg",
        "Fideos spaghetti 400 g",
        "Harina 1 kg",
        "Leche entera 1 L",
        "Atún en agua 160 g",
        "Café instantáneo 170 g",
    ).mapIndexed { indice, nombre -> producto("p$indice", nombre) }

    /** Un producto con la forma exacta que devuelve el server. */
    private fun producto(id: String, nombre: String) = ProductDto(
        active = true,
        createdAt = "2026-08-08T00:00:00Z",
        id = id,
        name = nombre,
        physicalStock = true,
        prescriptionType = "none",
        price = "1490",
        slug = id,
        stock = 10L,
        updatedAt = "2026-08-08T00:00:00Z",
    )
}
