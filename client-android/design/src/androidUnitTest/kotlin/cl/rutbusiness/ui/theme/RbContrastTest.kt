package cl.rutbusiness.ui.theme

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Proves the palette rather than assuming it.
 *
 * Every pair a component can actually put on screen is measured here with the
 * WCAG 2.1 formula. If someone nudges a token to make a screenshot prettier,
 * this fails and says which pair and by how much.
 *
 * The floors are the ones the hardware floor asks for: AAA for body copy where
 * it is free, AA everywhere else, 3.0 for control boundaries.
 */
class RbContrastTest {

    private fun assertContrast(
        foreground: Color,
        background: Color,
        minimum: Double,
        what: String,
    ) {
        val ratio = contrastRatio(foreground, background)
        assertTrue(
            "$what measured %.2f:1, needs %.1f:1".format(ratio, minimum),
            ratio >= minimum,
        )
    }

    /** Every surface a foreground token can land on, per theme. */
    private fun surfaces(colors: RbColors) = listOf(
        "background" to colors.background,
        "surface" to colors.surface,
        "surfaceVariant" to colors.surfaceVariant,
        "surfaceRaised" to colors.surfaceRaised,
    )

    private fun checkTheme(name: String, colors: RbColors) {
        surfaces(colors).forEach { (surfaceName, surface) ->
            // Body copy: AAA. This is the text the older person reads all day.
            assertContrast(
                colors.textPrimary, surface, RbContrast.AAA,
                "$name textPrimary on $surfaceName",
            )
            assertContrast(
                colors.textSecondary, surface, RbContrast.AAA,
                "$name textSecondary on $surfaceName",
            )

            // Everything else: AA is the hard floor.
            assertContrast(
                colors.textTertiary, surface, RbContrast.AA,
                "$name textTertiary on $surfaceName",
            )
            assertContrast(
                colors.brandText, surface, RbContrast.AA,
                "$name brandText on $surfaceName",
            )
            assertContrast(
                colors.dangerText, surface, RbContrast.AA,
                "$name dangerText on $surfaceName",
            )
            assertContrast(
                colors.warnText, surface, RbContrast.AA,
                "$name warnText on $surfaceName",
            )
            assertContrast(
                colors.infoText, surface, RbContrast.AA,
                "$name infoText on $surfaceName",
            )

            // Control boundaries: WCAG 1.4.11.
            assertContrast(
                colors.outlineStrong, surface, RbContrast.NON_TEXT,
                "$name outlineStrong on $surfaceName",
            )
        }
    }

    @Test
    fun `dark theme meets every floor`() = checkTheme("dark", RbDarkColors)

    @Test
    fun `light theme meets every floor`() = checkTheme("light", RbLightColors)

    /** Labels sitting on a filled button. */
    @Test
    fun `filled button labels are readable`() {
        listOf("dark" to RbDarkColors, "light" to RbLightColors).forEach { (name, colors) ->
            assertContrast(
                colors.onBrandFill, colors.brandFill, RbContrast.AA,
                "$name primary button label",
            )
            assertContrast(
                colors.onDangerFill, colors.dangerFill, RbContrast.AA,
                "$name destructive button label",
            )
        }
    }

    /**
     * A filled button has no border, so the fill itself is the control's
     * boundary and has to clear 3.0 against what is behind it.
     */
    @Test
    fun `filled buttons are distinguishable from their surface`() {
        listOf("dark" to RbDarkColors, "light" to RbLightColors).forEach { (name, colors) ->
            assertContrast(
                colors.brandFill, colors.surface, RbContrast.NON_TEXT,
                "$name primary button edge",
            )
            assertContrast(
                colors.dangerFill, colors.surface, RbContrast.NON_TEXT,
                "$name destructive button edge",
            )
        }
    }

    /** Text drawn on the tinted containers used by chips and the error block. */
    @Test
    fun `tinted containers stay readable`() {
        listOf("dark" to RbDarkColors, "light" to RbLightColors).forEach { (name, colors) ->
            assertContrast(
                colors.textPrimary, colors.brandContainer, RbContrast.AAA,
                "$name textPrimary on brandContainer",
            )
            assertContrast(
                colors.textPrimary, colors.dangerContainer, RbContrast.AAA,
                "$name textPrimary on dangerContainer",
            )
        }
    }

    /** The focus ring has to be visible, or keyboard users are lost. */
    @Test
    fun `focus ring is visible on every surface`() {
        listOf("dark" to RbDarkColors, "light" to RbLightColors).forEach { (name, colors) ->
            surfaces(colors).forEach { (surfaceName, surface) ->
                assertContrast(
                    colors.focusRing, surface, RbContrast.NON_TEXT,
                    "$name focus ring on $surfaceName",
                )
            }
        }
    }

    /**
     * Regression guard. These are the exact values that shipped in
     * `client/src/brand.css` and each one failed a rule; the test asserts they
     * are gone rather than trusting a comment saying so.
     */
    @Test
    fun `the css values that failed are not in the palette`() {
        val rejected = mapOf(
            Color(0xFF97A08C) to "light --rb-txt-3, measured 2.72 on white",
            Color(0xFF5C6880) to "dark --rb-txt-3, measured 2.80 on --rb-raise",
            Color(0xFF5C6657) to "light --rb-txt-2, only AA where body copy needs AAA",
        )
        listOf(RbDarkColors, RbLightColors).forEach { colors ->
            listOf(colors.textSecondary, colors.textTertiary).forEach { token ->
                rejected[token]?.let { why ->
                    throw AssertionError("palette still carries $why")
                }
            }
        }
    }

    /** The light theme in the CSS inherited the dark status ramp; it must not. */
    @Test
    fun `light theme has its own status ramp`() {
        assertTrue(
            "light danger must differ from dark danger",
            RbLightColors.dangerText != RbDarkColors.dangerText,
        )
        assertTrue(
            "light warn must differ from dark warn",
            RbLightColors.warnText != RbDarkColors.warnText,
        )
        assertTrue(
            "light info must differ from dark info",
            RbLightColors.infoText != RbDarkColors.infoText,
        )
    }
}
