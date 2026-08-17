package cl.rutbusiness.ui.theme

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * [rbElevated] must be a pure draw effect.
 *
 * Every touch-target and no-clip-at-200% assertion in `RbFontScaleTest` reads
 * *measured* bounds. If a shadow modifier ever nudged those bounds - the way a
 * naive `Modifier.padding` companion would - this design system would ship a
 * touch target that measures correctly in a screenshot and wrong under a
 * finger, which is precisely the failure class the hardware floor exists to
 * catch. This proves the shadow never does that, independent of any one
 * component that happens to call it.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class RbElevationTest {

    @get:Rule
    val compose = createComposeRule()

    @Composable
    private fun sized(tag: String, elevated: Boolean) {
        Box(
            modifier = Modifier
                .testTag(tag)
                .size(40.dp)
                .let { if (elevated) it.rbElevated(20.dp, RectangleShape) else it },
        )
    }

    @Test
    fun `a shadow does not change the measured size of its receiver`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                sized("plain", elevated = false)
                sized("elevated", elevated = true)
            }
        }
        compose.waitForIdle()

        val plain = compose.onNodeWithTag("plain").getUnclippedBoundsInRoot()
        val elevated = compose.onNodeWithTag("elevated").getUnclippedBoundsInRoot()

        assertEquals("a shadow must not change measured width", plain.width, elevated.width)
        assertEquals("a shadow must not change measured height", plain.height, elevated.height)
    }
}
