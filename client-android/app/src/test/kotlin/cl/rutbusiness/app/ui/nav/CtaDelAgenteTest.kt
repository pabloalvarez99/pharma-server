package cl.rutbusiness.app.ui.nav

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import cl.rutbusiness.app.ui.agente.irAlAgente
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Que el camino al agente exista **desde adentro de las pestañas**.
 *
 * `LocalIrAlAgente` nace en `null`, y una pantalla que lee `null` esconde el
 * botón a propósito. Eso convierte un cableado faltante en un síntoma mudo: los
 * vacíos de "Hoy" y de "Quién me debe" siguen dibujándose, el texto que enseña
 * el paso siguiente sigue ahí, y lo único que falta es justamente la parte que
 * lo vuelve accionable. Nadie ve un error; se ve una pantalla un poco más
 * pobre, y esa es la clase de regresión que sobrevive meses.
 *
 * Los tests de escala de Resumen y de Fiado proveen el local ellos mismos, así
 * que prueban que el CTA funciona *si alguien lo cablea* — no que alguien lo
 * cablee. Esta prueba mira lo otro: monta el [ContenedorDeDestinos] de verdad y
 * pregunta desde el contenido de una pestaña.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class CtaDelAgenteTest {

    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    /**
     * Una pantalla cualquiera que hace lo que hacen los vacíos de verdad:
     * pregunta si desde acá se puede llegar al agente y ofrece el botón sólo si
     * la respuesta es que sí.
     */
    @Composable
    private fun PantallaQuePregunta(nombre: String) {
        val ir = irAlAgente()
        Column {
            Text("Estoy en $nombre")
            if (ir == null) {
                Text("Desde acá no se llega al agente")
            } else {
                RbButton(label = "Hablarle al agente", onClick = ir)
            }
        }
    }

    private fun montar() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                ContenedorDeDestinos { destino ->
                    when (destino) {
                        Destino.Agente -> Text("Estoy en el agente")
                        Destino.Cobrar -> PantallaQuePregunta("cobrar")
                        Destino.Resumen -> PantallaQuePregunta("el resumen")
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    private fun irA(destino: Destino) {
        compose.onNodeWithText(destino.etiqueta).performClick()
        compose.waitForIdle()
    }

    @Test
    fun `el contenido de una pestana ve el camino al agente`() {
        montar()
        irA(Destino.Resumen)

        compose.onNodeWithText("Estoy en el resumen").assertIsDisplayed()
        compose.onNodeWithText("Desde acá no se llega al agente").assertDoesNotExist()
    }

    @Test
    fun `el CTA cambia de pestana de verdad`() {
        montar()
        irA(Destino.Resumen)

        compose.onNodeWithText("Hablarle al agente").performClick()
        compose.waitForIdle()

        // No alcanza con que el lambda no reviente: tiene que quedar el agente
        // en pantalla. Un `remember` mal escrito deja el callback apuntando a
        // un estado viejo y el toque no hace nada visible.
        compose.onNodeWithText("Estoy en el agente").assertIsDisplayed()
    }
}
