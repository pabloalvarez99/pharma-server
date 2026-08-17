package cl.rutbusiness.ui.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.height
import cl.rutbusiness.ui.icons.RbIcons
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
 * The wave-27 audit's "espacio vacío enorme" finding, turned into an
 * assertion: an empty state must carry a real mark from [RbIcons] (not a
 * typographic glyph standing in for one), a benefit sentence separate from
 * the how-to hint, and a live action - and none of it may sit at a height
 * fixed in dp, because that is exactly what breaks under a 200% system font
 * scale.
 *
 * Robolectric + [GraphicsMode.Mode.NATIVE], same setup as `RbFontScaleTest`:
 * the assertions are about measured layout and real font metrics, and a JVM
 * run is cheap enough to sit in CI on every commit.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class RbEmptyStateTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `las cuatro partes se muestran cuando estan todas`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                RbEmptyState(
                    title = "Todavía no cargaste nada",
                    icon = RbIcons.catalogoContorno,
                    benefit = "Así cobras sin tener que acordarte del precio.",
                    hint = "Agrega tomates, cilantro, lo que sea, con el precio.",
                    actionLabel = "Agregar una cosa",
                    onAction = {},
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Todavía no cargaste nada").assertIsDisplayed()
        compose.onNodeWithText("Así cobras sin tener que acordarte del precio.")
            .assertIsDisplayed()
        compose.onNodeWithText("Agrega tomates, cilantro, lo que sea, con el precio.")
            .assertIsDisplayed()
        compose.onNodeWithText("Agregar una cosa").assertIsDisplayed()
    }

    /**
     * Regla dura del asiento: "el botón de un estado vacío tiene que llamar a
     * algo que ya funciona". Si el caller no tiene una acción real, no hay
     * botón fantasma que no lleve a ninguna parte.
     */
    @Test
    fun `sin accion no aparece un boton muerto`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                RbEmptyState(title = "Nada por acá todavía")
            }
        }
        compose.waitForIdle()

        assertTrue(
            "sin actionLabel/onAction no debería haber nada tocable",
            compose.onAllNodes(hasClickAction()).fetchSemanticsNodes().isEmpty(),
        )
    }

    /**
     * El botón del vacío es la puerta de entrada al primer paso: tiene que
     * seguir siendo alcanzable con el dedo incluso cuando el resto del bloque
     * creció por la escala de letra del sistema.
     */
    @Test
    fun `el boton de accion sostiene el objetivo tactil al 200 por ciento`() {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = 2f),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    RbEmptyState(
                        title = "Nadie te debe ahora",
                        icon = RbIcons.fiadoContorno,
                        benefit = "Así no se te olvida quién quedó debiendo.",
                        actionLabel = "Hablarle al agente",
                        onAction = {},
                    )
                }
            }
        }
        compose.waitForIdle()

        val boton = compose.onNodeWithText("Hablarle al agente").getUnclippedBoundsInRoot()
        assertTrue(
            "el botón mide ${boton.height}, bajo el piso táctil de " +
                "${RbDefaultDimens.touchTarget}",
            boton.height >= RbDefaultDimens.touchTarget,
        )
    }

    /**
     * Regresión directa del bug que motivó este asiento: un vacío armado con
     * una altura fija en dp se ve bien a 100% y recorta el texto a 200%. Esto
     * mide el bloque completo a las dos escalas y exige que crezca - la misma
     * forma que usa `RbFontScaleTest` para probar que un control no está fijo
     * en dp donde debería seguir el `sp` del sistema.
     */
    @Test
    fun `el bloque crece con la escala de letra en vez de quedar fijo`() {
        compose.setContent {
            val base = LocalDensity.current
            RbTheme(darkTheme = true, reducedMotion = true) {
                // Con scroll a propósito: sin él la Column reparte el alto de la
                // pantalla y el segundo bloque mide lo que sobró del primero, no
                // lo que necesita. Los dos tienen que medirse con alto libre o la
                // comparación no dice nada sobre la escala de letra.
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    listOf(1.0f to "1x", 2.0f to "2x").forEach { (scale, tag) ->
                        CompositionLocalProvider(
                            LocalDensity provides Density(base.density, fontScale = scale),
                        ) {
                            RbEmptyState(
                                title = "Nadie te debe ahora",
                                icon = RbIcons.fiadoContorno,
                                benefit = "Así no se te olvida quién quedó debiendo.",
                                hint = "Dile al agente: «fié tomates a Rosa a 2000». " +
                                    "Queda acá hasta que te pague.",
                                actionLabel = "Hablarle al agente",
                                onAction = {},
                                modifier = Modifier.testTag("vacio-$tag"),
                            )
                        }
                    }
                }
            }
        }
        compose.waitForIdle()

        val pequeno = compose.onNodeWithTag("vacio-1x").getUnclippedBoundsInRoot().height
        val grande = compose.onNodeWithTag("vacio-2x").getUnclippedBoundsInRoot().height
        assertTrue(
            "el vacío midió $pequeno a 1x y $grande a 2x - no creció, así que algún " +
                "texto está fijo en dp en vez de seguir la escala del sistema",
            grande > pequeno,
        )
    }
}
