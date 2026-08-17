package cl.rutbusiness.ui.components

import androidx.compose.foundation.layout.Column
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.width
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * `loading` is a third button state, not a re-skin of `enabled = false`.
 *
 * The KDoc on [RbButton] draws the line: disabled says "you can't", loading
 * says "you already did, hang on" - so a loading button must still refuse the
 * tap the way disabled does (asserted here through the same `disabled`
 * semantics [rbClickable][cl.rutbusiness.ui.theme.rbClickable] sets for
 * `enabled = false`, since that is the one signal both states are guaranteed
 * to carry), while looking different from both the ordinary and the disabled
 * button - which here means it is measurably wider because a spinner and its
 * gap sit before the label. Colour alone was already ruled out as a state
 * signal by the brief; this is that rule applied to the state this wave adds.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class RbButtonStatesTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `a loading button is reported as not enabled, same as a disabled one`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                RbButton(
                    label = "Cobrar",
                    onClick = {},
                    loading = true,
                    modifier = Modifier.testTag("boton"),
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithTag("boton").assertIsNotEnabled()
    }

    @Test
    fun `a disabled button is reported as not enabled`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                RbButton(
                    label = "Cobrar",
                    onClick = {},
                    enabled = false,
                    modifier = Modifier.testTag("boton"),
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithTag("boton").assertIsNotEnabled()
    }

    @Test
    fun `an ordinary button is enabled and fires onClick`() {
        var clicks = 0
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                RbButton(
                    label = "Cobrar",
                    onClick = { clicks++ },
                    modifier = Modifier.testTag("boton"),
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithTag("boton").assertIsEnabled().performClick()
        assertTrue("a plain enabled button must still fire onClick", clicks > 0)
    }

    @Test
    fun `loading is visually distinct from the ordinary button, not just disabled`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Column {
                    RbButton(
                        label = "Cobrar",
                        onClick = {},
                        modifier = Modifier.testTag("normal"),
                    )
                    RbButton(
                        label = "Cobrar",
                        onClick = {},
                        loading = true,
                        modifier = Modifier.testTag("cargando"),
                    )
                }
            }
        }
        compose.waitForIdle()

        val normalWidth = compose.onNodeWithTag("normal").getUnclippedBoundsInRoot().width
        val loadingWidth = compose.onNodeWithTag("cargando").getUnclippedBoundsInRoot().width

        assertFalse(
            "a loading button with the same label must not measure identically to " +
                "the plain button - the spinner has to occupy real space, or the state " +
                "is carried by colour alone",
            normalWidth == loadingWidth,
        )
        assertTrue(
            "the loading button ($loadingWidth) should be wider than the plain one " +
                "($normalWidth): it carries an extra spinner and gap",
            loadingWidth > normalWidth,
        )
    }
}
