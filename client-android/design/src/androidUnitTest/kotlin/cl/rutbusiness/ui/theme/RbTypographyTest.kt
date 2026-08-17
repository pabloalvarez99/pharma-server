package cl.rutbusiness.ui.theme

import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Fixes the values this ola added to the type scale, JVM-pure - no Robolectric
 * needed, [androidx.compose.ui.text.TextStyle] and its value classes ([sp],
 * `em`) are plain Kotlin with no Android runtime dependency.
 *
 * Pairs with [RbCmpDisciplineTest], which already fixes the "no `dp` size" and
 * "no platform import in `commonMain`" invariants for this file; this test
 * fixes the ones specific to the visual pass: a real headline register,
 * tracking sign per style, and money that renders tabular figures in the
 * brand sans instead of a monospace face.
 */
class RbTypographyTest {

    private val scale = RbDefaultTypography

    @Test
    fun `display is the biggest, boldest register in the scale`() {
        assertEquals(34.sp, scale.display.fontSize)
        assertEquals(FontWeight.Bold, scale.display.fontWeight)
        assertTrue(
            "display (${scale.display.fontSize}) must be strictly larger than title " +
                "(${scale.title.fontSize}), or the scale still has no register above title",
            scale.display.fontSize.value > scale.title.fontSize.value,
        )
    }

    @Test
    fun `large semibold and bold styles carry negative tracking`() {
        listOf(
            "display" to scale.display,
            "title" to scale.title,
            "heading" to scale.heading,
        ).forEach { (name, style) ->
            assertTrue(
                "$name must declare a letterSpacing, none found",
                style.letterSpacing.isSp || style.letterSpacing.isEm,
            )
            assertTrue(
                "$name tracking must be negative (tight), was ${style.letterSpacing}",
                style.letterSpacing.value < 0f,
            )
        }
    }

    @Test
    fun `small labels carry positive tracking`() {
        listOf(
            "label" to scale.label,
            "chip" to scale.chip,
        ).forEach { (name, style) ->
            assertTrue(
                "$name must declare a letterSpacing, none found",
                style.letterSpacing.isSp || style.letterSpacing.isEm,
            )
            assertTrue(
                "$name tracking must be positive (loose), was ${style.letterSpacing}",
                style.letterSpacing.value > 0f,
            )
        }
    }

    @Test
    fun `amount is tabular sans, not monospace`() {
        assertNotEquals(FontFamily.Monospace, scale.amount.fontFamily)
        assertEquals(RbBrandFont, scale.amount.fontFamily)
        assertEquals(
            "amount must request OpenType tabular figures so digits still line up",
            "\"tnum\" 1",
            scale.amount.fontFeatureSettings,
        )
    }

    @Test
    fun `numeric stays monospace for RUT and folios`() {
        // The one style allowed to keep the terminal look: a RUT's check
        // letter has to sit the same width as the digits around it, which
        // `tnum` alone (digits only) cannot guarantee.
        assertEquals(FontFamily.Monospace, scale.numeric.fontFamily)
    }

    @Test
    fun `every weight in the scale has a real bundled cut`() {
        // RbFont.android.kt bundles exactly Normal, Medium, SemiBold and Bold
        // as static files - no variable-font axis interpolation, because that
        // silently collapses to one weight below API 26 (see its KDoc). A
        // style that asks for any other FontWeight would render synthesized
        // instead of the real cut, on every API level.
        val bundled = setOf(FontWeight.Normal, FontWeight.Medium, FontWeight.SemiBold, FontWeight.Bold)
        val used = listOf(
            "display" to scale.display,
            "title" to scale.title,
            "heading" to scale.heading,
            "body" to scale.body,
            "bodyStrong" to scale.bodyStrong,
            "support" to scale.support,
            "label" to scale.label,
            "button" to scale.button,
            "amount" to scale.amount,
            "chip" to scale.chip,
        )
        used.forEach { (name, style) ->
            assertTrue(
                "$name uses FontWeight ${style.fontWeight}, which has no bundled static cut",
                style.fontWeight in bundled,
            )
        }
    }

    @Test
    fun `brand-facing styles use the bundled family, not the bare system fallback`() {
        // FontFamily.SansSerif is the pre-this-ola default and is still the
        // *fallback* RbBrandFont resolves to if loading ever fails - but the
        // styles themselves must ask for the brand family, not hardcode the
        // fallback and skip the brand face entirely.
        listOf(
            "display" to scale.display,
            "title" to scale.title,
            "heading" to scale.heading,
            "body" to scale.body,
            "bodyStrong" to scale.bodyStrong,
            "support" to scale.support,
            "label" to scale.label,
            "button" to scale.button,
            "amount" to scale.amount,
            "chip" to scale.chip,
        ).forEach { (name, style) ->
            assertEquals("$name must use RbBrandFont", RbBrandFont, style.fontFamily)
        }
        // RUT/folio is the one deliberate exception, asserted separately above.
        assertFalse(scale.numeric.fontFamily == RbBrandFont)
    }
}
