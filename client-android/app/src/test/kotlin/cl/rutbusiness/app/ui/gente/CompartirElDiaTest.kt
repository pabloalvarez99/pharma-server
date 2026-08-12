package cl.rutbusiness.app.ui.gente

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import cl.rutbusiness.app.ui.resumen.Comparacion
import cl.rutbusiness.app.ui.resumen.TarjetaDelDia
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
 * Que el botón de compartir exista, se pueda tocar, y mande lo que se ve.
 *
 * El texto ya está probado sin `Activity` en `TextoParaGenteTest`. Lo que falta
 * probar es lo que ese test no puede: que la pantalla realmente llame al puerto,
 * con el monto que está dibujando y no con otro, y que el control siga siendo
 * tocable con la letra al 200%.
 *
 * `NATIVE` no es opcional, por lo mismo que en `ResumenEscalaTest`: con los
 * gráficos por defecto de Robolectric el texto mide ~0,5dp por carácter y
 * cualquier aserción de tamaño pasaría sin significar nada.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class CompartirElDiaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    /** Un puerto de mentira que anota lo que le mandaron. */
    private class PuertoFalso : CompartirConGente {
        val mandados = mutableListOf<String>()
        override fun compartir(texto: String) {
            mandados += texto
        }
    }

    /**
     * @param puerto `null` imita el contenedor que todavía no lo provee. Ahí el
     *   botón no se dibuja, que es el contrato de [LocalCompartirConGente].
     */
    private fun mostrar(
        escala: Float = 1.0f,
        boletas: Long = 12L,
        vendidoHoy: String = "45000",
        rubro: String? = null,
        puerto: CompartirConGente? = PuertoFalso(),
    ) {
        val servicios = rubro?.let {
            ServiciosDeRubro(RubroPackRepository().apply { usarLocal(it) })
        }
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
                LocalRubro provides servicios,
                LocalCompartirConGente provides puerto,
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        TarjetaDelDia(
                            moneda = Moneda.de("CLP"),
                            vendidoHoy = vendidoHoy,
                            boletas = boletas,
                            comparacion = Comparacion.Mejor,
                            vendidoAyer = "30000",
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun `con ventas el boton manda el monto y las boletas que la tarjeta muestra`() {
        val puerto = PuertoFalso()
        mostrar(puerto = puerto)

        compose.onNodeWithText("Compartir el resumen").performScrollTo().performClick()
        compose.waitForIdle()

        assertEquals(listOf("Hoy en el negocio: $45.000 en 12 boletas"), puerto.mandados)
    }

    @Test
    fun `una sola boleta se dice en singular`() {
        val puerto = PuertoFalso()
        mostrar(boletas = 1L, vendidoHoy = "3500", puerto = puerto)

        compose.onNodeWithText("Compartir el resumen").performScrollTo().performClick()
        compose.waitForIdle()

        assertEquals(listOf("Hoy en el negocio: $3.500 en 1 boleta"), puerto.mandados)
    }

    @Test
    fun `sin ventas todavia no hay dia que contar`() {
        // El vacío de A3 enseña a anotar la primera venta. Ofrecer ahí
        // "compartí que no vendiste nada" es una broma pesada, y en el pack
        // formal la tarjeta dice "Todavía no hay ninguna venta": tampoco hay
        // nada que mandar.
        mostrar(boletas = 0L, vendidoHoy = "0")

        compose.onNodeWithText("Compartir el resumen").assertDoesNotExist()
    }

    @Test
    fun `sin ventas en feria tampoco, que ahi la tarjeta es el vacio`() {
        mostrar(boletas = 0L, vendidoHoy = "0", rubro = "feria")

        compose.onNodeWithText("Contar cómo va el día").assertDoesNotExist()
        compose.onNodeWithText("Compartir el resumen").assertDoesNotExist()
    }

    @Test
    fun `sin puerto detras el boton no se dibuja`() {
        // Un botón que no abre nada es peor que no tenerlo: se toca, no pasa
        // nada, y la dueña queda sin saber si falló ella o el teléfono.
        mostrar(puerto = null)

        compose.onNodeWithText("Compartir el resumen").assertDoesNotExist()
    }

    @Test
    fun `en feria el copy es otro y el boton sigue estando`() {
        val puerto = PuertoFalso()
        mostrar(rubro = "feria", puerto = puerto)

        compose.onNodeWithText("Contar cómo va el día").performScrollTo().performClick()
        compose.waitForIdle()

        assertEquals(listOf("Hoy en el negocio: $45.000 en 12 boletas"), puerto.mandados)
    }

    @Test
    fun `al 200 por ciento el boton sigue siendo tocable`() {
        mostrar(escala = 2.0f)

        val boton = compose.onNodeWithText("Compartir el resumen").performScrollTo()
        boton.assertIsDisplayed()

        val alto = boton.getUnclippedBoundsInRoot().height
        assertTrue(
            "el botón quedó de $alto, menos que el objetivo táctil de $objetivoTactil",
            alto >= objetivoTactil - 1.dp,
        )
    }
}
