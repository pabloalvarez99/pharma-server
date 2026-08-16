package cl.rutbusiness.app.ui.catalogo

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
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.height
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.rubro.PACK_FARMACIA
import cl.rutbusiness.core.rubro.PACK_FERIA
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
 * Cargar la primera cosa en un teléfono de 2015, con la letra al doble.
 *
 * Es la pantalla por donde entra un negocio nuevo, y la persona que la usa es
 * exactamente la que sube la escala de letra porque ve poco. Si el botón de
 * guardar no se alcanza con el teclado abierto, no hay producto; y sin producto
 * no hay venta.
 *
 * Mismo mecanismo que `BuscarConTecladoTest`, por las mismas razones medidas:
 * el teclado se simula acortando el panel (`h320dp`), la escala entra por
 * `LocalDensity` -que es lo que Compose lee de verdad-, `NATIVE` porque sin eso
 * el motor de texto de Robolectric mide ~0,5dp por carácter y todo lo de acá
 * sería ficción, y `sdk = 34` porque ahí vive la curva **no lineal** de la
 * escala que recibe el usuario.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class CatalogoEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil: Dp = RbDefaultDimens.touchTarget

    // --- el formulario con el teclado arriba ---------------------------------

    /**
     * El caso que decide todo: teclado arriba, letra al 200%, y hay que poder
     * guardar. `performScrollTo` es legítimo acá -el formulario scrollea a
     * propósito-, lo que no puede pasar es que el botón no exista o quede
     * demasiado chico para el dedo.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba al 200 por ciento se puede guardar`() {
        montarFormulario(escala = 2.0f)

        compose.onNodeWithContentDescription("¿Cómo se llama?").performTextInput("Tomate")
        compose.onNodeWithContentDescription("¿A cuánto lo vendes?").performTextInput("1200")

        val guardar = botonDeGuardar()
        guardar.performScrollTo().assertIsDisplayed()
        val caja = guardar.getBoundsInRoot()
        println("MEDIDA guardar=${caja.height} escala=2.0")
        assertTrue(
            "a 2x el botón Guardar mide ${caja.height} de alto, bajo el objetivo " +
                "táctil de $objetivoTactil: no se puede tocar",
            caja.height >= objetivoTactil,
        )
    }

    /** Y los tres campos del camino feliz están, todos, al 200%. */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `los tres campos del camino feliz se alcanzan al 200 por ciento`() {
        montarFormulario(escala = 2.0f)

        compose.onNodeWithContentDescription("¿Cómo se llama?").performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithContentDescription("¿A cuánto lo vendes?").performScrollTo()
            .assertIsDisplayed()
        compose.onNodeWithContentDescription("Se vende por").performScrollTo()
            .assertIsDisplayed()
    }

    /**
     * Y ninguno más.
     *
     * "Tres campos y ninguno más en el camino feliz" es la promesa de esta
     * pantalla. Categoría, costo, código o lote convertirían "carga lo que
     * vendes" en un formulario de sistema, que es de lo que la dueña se está
     * escapando.
     */
    @Test
    fun `el formulario no pide nada mas que nombre precio y unidad`() {
        montarFormulario(escala = 1.0f)

        compose.onNodeWithText("Código de barras").assertDoesNotExist()
        compose.onNodeWithText("Categoría").assertDoesNotExist()
        compose.onNodeWithText("Costo").assertDoesNotExist()
        compose.onNodeWithText("Stock").assertDoesNotExist()
    }

    /**
     * Las unidades se tocan, no se escriben.
     *
     * Escribir "kilo" en un teclado de teléfono con la letra al 200% es la parte
     * más lenta de cargar veinte cosas seguidas.
     */
    @Test
    fun `las unidades sugeridas se pueden tocar y son del tamano del dedo`() {
        montarFormulario(escala = 2.0f)

        val kilo = compose.onNode(hasText("kilo") and hasClickAction())
        kilo.performScrollTo().assertIsDisplayed()
        val caja = kilo.getBoundsInRoot()
        assertTrue(
            "a 2x el chip \"kilo\" mide ${caja.height}, bajo $objetivoTactil",
            caja.height >= objetivoTactil,
        )
    }

    /**
     * En un rubro que no declara unidad, el campo no está.
     *
     * El pack manda. Inventarle un "se vende por" a una farmacia sería la app
     * decidiendo por el server, que es justo lo que ADR-0022 evita.
     */
    @Test
    fun `en farmacia no hay campo de unidad`() {
        montarFormulario(escala = 1.0f, copy = copyCatalogo(PACK_FARMACIA))

        compose.onNodeWithContentDescription("Se vende por").assertDoesNotExist()
        compose.onNodeWithContentDescription("¿Cómo se llama?").assertIsDisplayed()
    }

    // --- mesa del puesto -----------------------------------------------------

    /**
     * Cada cosa es una tarjeta táctil de la mesa: nombre grande, precio a la
     * vista, mínimo 56dp. Si la fila se encoge, el puesto deja de ser tocable
     * con el sol de la feria y el dedo gordo.
     */
    @Test
    fun `la tarjeta de una cosa mide al menos el objetivo tactil`() {
        montarTarjeta(escala = 1.0f)

        val fila = compose.onNode(hasText("Tomate") and hasClickAction())
        fila.assertIsDisplayed()
        val caja = fila.getBoundsInRoot()
        assertTrue(
            "la tarjeta mide ${caja.height}, bajo $objetivoTactil",
            caja.height >= objetivoTactil,
        )
        compose.onNodeWithText("$1.200").assertIsDisplayed()
        compose.onNodeWithText("Por kilo").assertIsDisplayed()
    }

    /**
     * El vacío enseña con espacio y un solo CTA: la pista de feria y el botón
     * de agregar. Sin segunda acción ni jerga de sistema.
     */
    @Test
    fun `el vacio de feria ensena y tiene un solo cta primario`() {
        val copy = copyCatalogo(PACK_FERIA)
        montarVacio(escala = 1.0f, copy = copy)

        compose.onNodeWithText(copy.vacioTitulo).assertIsDisplayed()
        compose.onNodeWithText(copy.vacioPista, substring = true).assertIsDisplayed()
        val cta = compose.onNode(hasText(copy.agregar) and hasClickAction())
        cta.assertIsDisplayed()
        val caja = cta.getBoundsInRoot()
        assertTrue(
            "el CTA del vacío mide ${caja.height}, bajo $objetivoTactil",
            caja.height >= objetivoTactil,
        )
    }

    // --- herramientas --------------------------------------------------------

    private fun botonDeGuardar(): SemanticsNodeInteraction =
        compose.onNode(hasText("Guardar") and hasClickAction())

    private fun montarFormulario(
        escala: Float,
        copy: CopyCatalogo = copyCatalogo(PACK_FERIA),
    ) {
        compose.setContent {
            var nombre by remember { mutableStateOf("") }
            var precio by remember { mutableStateOf("") }
            var unidad by remember { mutableStateOf("") }

            ConEscala(escala) {
                Column(modifier = Modifier.fillMaxSize()) {
                    FormularioContenido(
                        copy = copy,
                        nombre = nombre,
                        onNombre = { nombre = it },
                        errorDeNombre = null,
                        precio = precio,
                        onPrecio = { precio = it },
                        errorDePrecio = null,
                        simbolo = "$",
                        unidad = unidad,
                        onUnidad = { unidad = it },
                        onElegirUnidad = { unidad = it },
                        guardando = false,
                        errorAlGuardar = null,
                        onGuardar = {},
                    )
                }
            }
        }
        compose.waitForIdle()
    }

    private fun montarTarjeta(escala: Float) {
        compose.setContent {
            ConEscala(escala) {
                FilaDeCosa(
                    cosa = ProductDto(
                        active = true,
                        createdAt = "2026-01-01T00:00:00Z",
                        id = "p1",
                        name = "Tomate",
                        physicalStock = false,
                        prescriptionType = "none",
                        price = "1200",
                        slug = "tomate",
                        stock = 0,
                        updatedAt = "2026-01-01T00:00:00Z",
                    ),
                    unidad = "kilo",
                    precio = "$1.200",
                    onEditar = {},
                )
            }
        }
        compose.waitForIdle()
    }

    private fun montarVacio(escala: Float, copy: CopyCatalogo) {
        compose.setContent {
            ConEscala(escala) {
                VacioDelCatalogo(
                    titulo = copy.vacioTitulo,
                    pista = copy.vacioPista,
                    accion = copy.agregar,
                    onAccion = {},
                )
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
}
