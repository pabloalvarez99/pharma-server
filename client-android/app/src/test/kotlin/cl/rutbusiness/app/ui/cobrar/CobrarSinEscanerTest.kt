package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.height
import cl.rutbusiness.app.ui.catalogo.copyCatalogo
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.rubro.PACK_FERIA
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
 * El puesto de feria abriendo la app por primera vez.
 *
 * El pack `feria` apaga el escáner a propósito -en un puesto no hay un EAN
 * pegado en el tomate-, y hasta ahora ése era el único camino de la app para
 * cargar un producto: la pantalla de cobrar terminaba en "cuando cargues
 * productos van a aparecer acá" sin decir dónde, porque no había dónde.
 *
 * Estas pruebas fijan las dos salidas de ese callejón: el vacío ofrece cargar, y
 * el cobro rápido está a la vista sin haber cargado nada.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class CobrarSinEscanerTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil: Dp = RbDefaultDimens.touchTarget
    private val copyFeria = copyCatalogo(PACK_FERIA)

    // --- nada que escanear ----------------------------------------------------

    /**
     * Que la cámara esté apagada en el pack tiene que verse en la pantalla.
     *
     * Que la función exista y esté apagada no sirve de nada si la navegación
     * igual empuja para allá.
     */
    @Test
    fun `sin escaner no hay boton de escanear`() {
        montar(hayEscaner = false)
        compose.onNodeWithText("Escanear código").assertDoesNotExist()
    }

    /** Y con un pack que sí lo tiene, sigue estando: esto no apaga a nadie más. */
    @Test
    fun `con escaner el boton sigue estando`() {
        montar(hayEscaner = true)
        compose.onNodeWithText("Escanear código").assertIsDisplayed()
    }

    // --- el vacío enseña ------------------------------------------------------

    /**
     * Sin nada cargado, la pantalla dice qué hacer y trae el botón.
     *
     * Antes decía "cuando cargues productos en el sistema del negocio, van a
     * aparecer acá": una frase que manda a un lugar que la dueña no tiene —el
     * sistema del negocio es este teléfono— y que no ofrece nada que tocar.
     */
    @Test
    fun `el catalogo vacio ofrece cargar la primera cosa`() {
        var abierto = 0
        montar(hayEscaner = false, resultados = emptyList(), onCargar = { abierto += 1 })

        compose.onNodeWithText(copyFeria.vacioTitulo).assertIsDisplayed()
        val boton = compose.onNode(hasText(copyFeria.agregar) and hasClickAction())
        boton.performScrollTo().assertIsDisplayed()
        boton.performClick()

        assertEquals("el botón del vacío tiene que abrir \"Lo que vendo\"", 1, abierto)
    }

    /** Y al 200%, que es cuando el vacío se queda sin lugar. */
    @Test
    fun `el boton del vacio se puede tocar al 200 por ciento`() {
        montar(escala = 2.0f, hayEscaner = false, resultados = emptyList())

        val boton = compose.onNode(hasText(copyFeria.agregar) and hasClickAction())
        boton.performScrollTo().assertIsDisplayed()
        val caja = boton.getBoundsInRoot()
        println("MEDIDA vacio-agregar=${caja.height} escala=2.0")
        assertTrue(
            "a 2x el botón del vacío mide ${caja.height}, bajo $objetivoTactil",
            caja.height >= objetivoTactil,
        )
    }

    /**
     * Sin forma de cargar -sin sesión- no se ofrece un botón que no lleva a
     * ninguna parte.
     */
    @Test
    fun `sin sesion el vacio no promete un boton que no existe`() {
        montar(hayEscaner = false, resultados = emptyList(), onCargar = null)
        compose.onNodeWithText(copyFeria.agregar).assertDoesNotExist()
    }

    // --- cobro rápido ---------------------------------------------------------

    /**
     * "Son $2.000, una bolsa": la venta más común del puesto, a un toque y sin
     * haber cargado nada.
     */
    @Test
    fun `se puede cobrar un monto suelto sin tener nada cargado`() {
        var abierto = 0
        montar(hayEscaner = false, resultados = emptyList(), onMontoSuelto = { abierto += 1 })

        val boton = botonDeMontoSuelto()
        boton.assertIsDisplayed()
        boton.performClick()

        assertEquals(1, abierto)
    }

    /** Y el botón dice cuánto hay puesto, para no cobrar dos veces lo mismo. */
    @Test
    fun `el boton dice el monto que ya esta puesto`() {
        montar(hayEscaner = false, montoPuesto = "$2.000")
        compose.onNodeWithText("Monto puesto: $2.000").assertIsDisplayed()
    }

    /**
     * Mientras se escribe un nombre, el cobro rápido se esconde.
     *
     * Son los dp que le faltan a la lista de resultados con el teclado arriba, y
     * quien está tipeando un nombre no está por cobrar un monto suelto.
     */
    @Test
    fun `escribiendo un nombre el cobro rapido cede el lugar`() {
        montar(hayEscaner = false)
        compose.onNodeWithContentDescription("Buscar producto").performTextInput("tom")
        compose.waitForIdle()

        compose.onNodeWithText("Cobrar un monto").assertDoesNotExist()
    }

    // --- herramientas ---------------------------------------------------------

    private fun botonDeMontoSuelto(): SemanticsNodeInteraction =
        compose.onNode(hasText("Cobrar un monto") and hasClickAction())

    private fun montar(
        escala: Float = 1.0f,
        hayEscaner: Boolean,
        resultados: List<ProductDto> = catalogo,
        onCargar: (() -> Unit)? = {},
        onMontoSuelto: (() -> Unit)? = {},
        montoPuesto: String? = null,
    ) {
        compose.setContent {
            var consulta by remember { mutableStateOf("") }
            ConEscala(escala) {
                Column(modifier = Modifier.fillMaxSize()) {
                    BuscarContenido(
                        modifier = Modifier.fillMaxSize(),
                        consulta = consulta,
                        onConsulta = { consulta = it },
                        ayuda = "Escribe el nombre de lo que vendes.",
                        onEscanear = if (hayEscaner) ({}) else null,
                        resultados = resultados,
                        buscando = false,
                        errorBusqueda = null,
                        onBuscarAhora = {},
                        cantidadEnCarrito = { 0 },
                        precioDe = { "$1.490" },
                        onAgregar = {},
                        unidades = 0,
                        total = null,
                        onCobrar = {},
                        onCargarLoQueVendo = onCargar,
                        etiquetaCargar = copyFeria.agregar,
                        tituloSinCatalogo = copyFeria.vacioTitulo,
                        pistaSinCatalogo = copyFeria.vacioPista,
                        onMontoSuelto = onMontoSuelto,
                        montoSueltoPuesto = montoPuesto,
                    )
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
        producto("p0", "Tomate"),
        producto("p1", "Cilantro"),
    )

    private fun producto(id: String, nombre: String) = ProductDto(
        active = true,
        createdAt = "2026-08-09T00:00:00Z",
        id = id,
        name = nombre,
        physicalStock = true,
        prescriptionType = "none",
        price = "1490",
        slug = id,
        stock = 10L,
        updatedAt = "2026-08-09T00:00:00Z",
    )
}
