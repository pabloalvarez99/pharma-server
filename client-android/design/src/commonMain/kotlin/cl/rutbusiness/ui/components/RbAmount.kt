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
import cl.rutbusiness.ui.theme.RbTypography

/** How loud an [RbAmount] is. */
enum class RbAmountEmphasis {
    /**
     * The one figure the screen exists to show - the day's takings, the arqueo
     * difference. One per screen; two headlines are none.
     */
    Headline,

    /** A figure inside a card or a row. Tabular figures and bold, but not louder. */
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
 * - [RbAmountEmphasis.Headline] renders at [RbTypography.display] - the
 *   product's biggest, boldest register, reserved for the one figure a screen
 *   exists to show - until the system font scale goes large, where it drops to
 *   [RbTypography.title] instead: by then the device is enlarging everything
 *   anyway, and a smaller whole figure beats a bigger halved one. That rule
 *   lived inside one screen and had to be re-derived by the next one; here it
 *   is written once.
 * - Bold with `tnum` tabular figures ([RbTypography.amount]) is the scannable
 *   column of a cuaderno: digits claim the same width and line up, weight
 *   carries the eye to the plata before the surrounding copy - without the
 *   terminal-script look a monospace face gave every amount before this.
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

/**
 * The text style [RbAmount] uses, exposed for callers that must build on it.
 *
 * [RbAmountEmphasis.Headline] borrows its size, line height and tracking from
 * [RbTypography.display] (or [RbTypography.title] once the font scale is
 * large enough that the device is already doing the enlarging) rather than
 * from [RbTypography.amount] directly - a headline figure is still the
 * biggest thing on the screen, and [RbTypography.display] is where that
 * register lives. Only the family and the `tnum` tabular-figure feature come
 * from [RbTypography.amount], so both emphases keep the same digit treatment.
 */
@Composable
fun rbAmountStyle(emphasis: RbAmountEmphasis): TextStyle {
    val typography = RbTheme.typography
    return when (emphasis) {
        RbAmountEmphasis.Body -> typography.amount.copy(fontWeight = FontWeight.Bold)

        RbAmountEmphasis.Headline -> {
            val enlarged = LocalDensity.current.fontScale >= LARGE_FONT_SCALE
            val scale = if (enlarged) typography.title else typography.display
            scale.copy(
                fontFamily = typography.amount.fontFamily,
                fontFeatureSettings = typography.amount.fontFeatureSettings,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

/**
 * Where the headline register drops from [RbTypography.display] back down to
 * [RbTypography.title].
 *
 * 1.5x is the point at which Android's own font-scale curve stops being a
 * courtesy and starts being an accessibility setting the user needs.
 */
private const val LARGE_FONT_SCALE = 1.5f
