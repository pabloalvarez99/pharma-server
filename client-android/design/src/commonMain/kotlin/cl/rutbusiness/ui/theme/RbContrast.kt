package cl.rutbusiness.ui.theme

import androidx.compose.ui.graphics.Color
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow

/**
 * WCAG 2.1 contrast maths, in common code so the palette can be *asserted*
 * rather than assumed. `RbContrastTest` walks every pair the components
 * actually use and fails the build when one drops below its floor.
 */

/** WCAG floors. Body copy aims at [AAA]; everything else must clear [AA]. */
object RbContrast {
    /** Normal-size text and icons. */
    const val AA = 4.5

    /** Body copy target - the hardware floor asks for it wherever it is free. */
    const val AAA = 7.0

    /** Control boundaries and input borders (WCAG 1.4.11 non-text contrast). */
    const val NON_TEXT = 3.0
}

/** Relative luminance of an opaque sRGB color, per WCAG 2.1. */
fun relativeLuminance(color: Color): Double {
    fun channel(v: Float): Double {
        val c = v.toDouble()
        return if (c <= 0.04045) c / 12.92 else ((c + 0.055) / 1.055).pow(2.4)
    }
    return 0.2126 * channel(color.red) +
        0.7152 * channel(color.green) +
        0.0722 * channel(color.blue)
}

/**
 * Contrast ratio between two opaque colors, 1.0 to 21.0.
 *
 * Both arguments must be opaque. A translucent token has no single ratio, so
 * the palette carries pre-blended container colors instead of alphas.
 */
fun contrastRatio(a: Color, b: Color): Double {
    val la = relativeLuminance(a)
    val lb = relativeLuminance(b)
    return (max(la, lb) + 0.05) / (min(la, lb) + 0.05)
}
