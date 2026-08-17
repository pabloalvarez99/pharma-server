package cl.rutbusiness.app.ui.nav

import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * El mecanismo de [PilaDeNavegacion] que reemplaza los dos `enum` anidados de
 * la ola 27 (`RutinaDelDia` en `TuDiaRoute.kt`, `SeccionDelMenu` adentro de
 * `MenuDelPuestoScreen.kt`).
 *
 * Monta la misma pila y la misma regla de "volver" que usa [TuDiaRoute] —
 * empujar/sacar sobre `Pantalla` más el incremento de `vueltas` sólo al
 * dejar [Pantalla.Caja] o [Pantalla.Fiado] — pero con pantallas de mentira en
 * vez de `CajaRoute` y las demás reales: esas necesitan sesión y server para
 * montarse, y lo que está bajo prueba acá es la pila, no las pantallas de
 * producción. Mismo criterio que usa `NavegacionTest` para
 * [ContenedorDeDestinos].
 *
 * El caso que le da sentido a todo esto: [Pantalla.Caja] se alcanza desde
 * dos padres, [Pantalla.Resumen] y [Pantalla.Menu]. Antes tenían semánticas
 * de "volver" distintas según por dónde se entraba; con la pila, volver
 * siempre saca la última pantalla empujada, sin que Caja sepa quién la abrió.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class PilaDeNavegacionTest {

    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    /**
     * Mismo armazón que [TuDiaRoute]: una pila que arranca en
     * [Pantalla.Resumen], un `BackHandler` habilitado mientras no se esté en
     * la raíz, y `vueltas` que sólo sube al dejar la caja o el fiado — no al
     * dejar el menú, la impresora o el rubro.
     */
    @Composable
    private fun ArbolDeMentira() {
        val pila = rememberPilaDeNavegacion(Pantalla.Resumen)
        var vueltas by rememberSaveable { mutableStateOf(0) }
        val volver: () -> Unit = {
            if (pila.actual == Pantalla.Caja || pila.actual == Pantalla.Fiado) vueltas += 1
            pila.volver()
        }
        BackHandler(enabled = !pila.enRaiz) { volver() }

        RbTheme(darkTheme = true, reducedMotion = true) {
            Column {
                when (pila.actual) {
                    Pantalla.Resumen -> {
                        Text("Resumen, vueltas=$vueltas")
                        RbButton(label = "Ir a la caja desde resumen", onClick = { pila.ir(Pantalla.Caja) })
                        RbButton(label = "Ir al menu desde resumen", onClick = { pila.ir(Pantalla.Menu) })
                    }

                    Pantalla.Caja -> {
                        Text("Caja")
                        RbButton(label = "Volver de la caja", onClick = volver)
                    }

                    Pantalla.Fiado -> {
                        Text("Fiado")
                        RbButton(label = "Volver del fiado", onClick = volver)
                    }

                    Pantalla.Menu -> {
                        Text("Menu")
                        RbButton(label = "Ir a la caja desde el menu", onClick = { pila.ir(Pantalla.Caja) })
                        RbButton(label = "Volver del menu", onClick = volver)
                    }

                    Pantalla.Impresora -> Text("Impresora")

                    Pantalla.Rubro -> Text("Rubro")
                }
            }
        }
    }

    private fun montar() {
        compose.setContent { ArbolDeMentira() }
        compose.waitForIdle()
    }

    @Test
    fun `arranca en el resumen con cero vueltas`() {
        montar()
        compose.onNodeWithText("Resumen, vueltas=0").assertIsDisplayed()
    }

    @Test
    fun `caja abierta desde el resumen vuelve al resumen y suma una vuelta`() {
        montar()

        compose.onNodeWithText("Ir a la caja desde resumen").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Caja").assertIsDisplayed()

        compose.onNodeWithText("Volver de la caja").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Resumen, vueltas=1").assertIsDisplayed()
    }

    /** El caso que da nombre a todo esto: la caja tiene dos padres. */
    @Test
    fun `caja abierta desde el menu vuelve al menu, no al resumen`() {
        montar()

        compose.onNodeWithText("Ir al menu desde resumen").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Menu").assertIsDisplayed()

        compose.onNodeWithText("Ir a la caja desde el menu").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Caja").assertIsDisplayed()

        compose.onNodeWithText("Volver de la caja").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Menu").assertIsDisplayed()
    }

    @Test
    fun `volver del menu no suma vueltas pero volver de la caja adentro del menu si`() {
        montar()

        compose.onNodeWithText("Ir al menu desde resumen").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Ir a la caja desde el menu").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Volver de la caja").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Menu").assertIsDisplayed()

        compose.onNodeWithText("Volver del menu").performClick()
        compose.waitForIdle()
        // Una sola vuelta: la que sumó salir de la caja. Salir del menú no cuenta.
        compose.onNodeWithText("Resumen, vueltas=1").assertIsDisplayed()
    }

    @Test
    fun `atras fisico hace lo mismo que el boton volver de la pantalla`() {
        montar()

        compose.onNodeWithText("Ir al menu desde resumen").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Ir a la caja desde el menu").performClick()
        compose.waitForIdle()

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithText("Menu").assertIsDisplayed()

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithText("Resumen, vueltas=1").assertIsDisplayed()

        assertEquals(
            "en la raíz, atrás tiene que quedar en manos del sistema",
            false,
            compose.activity.onBackPressedDispatcher.hasEnabledCallbacks(),
        )
    }
}
