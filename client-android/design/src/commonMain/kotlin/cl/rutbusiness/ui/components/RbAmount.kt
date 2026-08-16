package cl.rutbusiness.ui.components

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import cl.rutbusiness.ui.theme.RbTheme

/** How loud an [RbAmount] is. */
enum class RbAmountEmphasis {
    /**
     * The one figure the screen exists to show - the day's takings, the arqueo
     * difference. One per screen; two headlines are none.
     */
    Headline,

    /** A figure inside a card or a row. Monospace and bold, but not louder. */
    Body,
}

/**
 * A money figure — the plata that has to read at a glance on the mesa.
 *
 * Takes text that is **already formatted** - the caller ran the tenant's
 * currency over the server's decimal string. This component never sees a number
 * and so cannot round, truncate or re-scale one.
 *
 * Why it is a component rather than a `Text` with a style:
 *
 * - An amount is a single word with no spaces, so when it does not fit Compose
 *   breaks it **in the middle**. "$1.234.567" renders as "$1.234." over "567"
 *   and the reader cannot tell a million from a thousand. Here the figure is
 *   forced onto **one line** (`softWrap = false`, `maxLines = 1`) so it never
 *   splits mid-digit.
 * - The [RbAmountEmphasis.Headline] multiplier is released once the system font
 *   scale goes large - by then the device is enlarging everything anyway, and a
 *   smaller whole figure beats a bigger halved one. That rule lived inside one
 *   screen and had to be re-derived by the next one; here it is written once.
 * - Bold monospace is the scannable column of a cuaderno: digits line up, weight
 *   carries the eye to the plata before the surrounding copy.
 *
 * No motion. A figure is static type, never a count-up or scale-in: both would
 * fight reduced motion and cost frames on the reference panel.
 */
@Composable
fun RbAmount(
    amount: String,
    modifier: Modifier = Modifier,
    emphasis: RbAmountEmphasis = RbAmountEmphasis.Body,
    color: Color = RbTheme.colors.textPrimary,
    textAlign: TextAlign? = null,
) {
    Text(
        text = amount,
        style = rbAmountStyle(emphasis),
        color = color,
        textAlign = textAlign,
        // Never wrap mid-figure. Overflow is the caller's layout problem (reflow
        // the row, drop the headline multiplier); a split amount is a wrong amount.
        softWrap = false,
        maxLines = 1,
        overflow = TextOverflow.Clip,
        modifier = modifier,
    )
}

/** The text style [RbAmount] uses, exposed for callers that must build on it. */
@Composable
fun rbAmountStyle(emphasis: RbAmountEmphasis): TextStyle {
    val typography = RbTheme.typography
    return when (emphasis) {
        RbAmountEmphasis.Body -> typography.numeric.copy(fontWeight = FontWeight.Bold)

        RbAmountEmphasis.Headline -> {
            val enlarged = LocalDensity.current.fontScale >= LARGE_FONT_SCALE
            typography.numeric.copy(
                fontSize = typography.title.fontSize * (if (enlarged) 1.0f else 1.5f),
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

/**
 * Where the headline multiplier is dropped.
 *
 * 1.5x is the point at which Android's own font-scale curve stops being a
 * courtesy and starts being an accessibility setting the user needs.
 */
private const val LARGE_FONT_SCALE = 1.5f
