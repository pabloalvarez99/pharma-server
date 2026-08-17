package cl.rutbusiness.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * RutBusiness type scale.
 *
 * Every size is `sp`, so the system font-scale multiplies it. There is not one
 * `dp` text size in this file, and there must never be: a `dp` size is a size
 * the older person cannot change from Ajustes, which is the exact failure the
 * hardware floor forbids.
 *
 * Four rules this file enforces beyond "use sp":
 *
 * 1. **Line height in `em`, not `sp`.** A line height fixed in `sp` does not
 *    grow with the glyphs, so at 200% the text collides with itself. Declaring
 *    it as a multiple of the font size makes it scale for free.
 * 2. **No weight below [FontWeight.Normal].** `brand.css` had `font-weight:500`
 *    labels in a 11-12px muted grey; at the reference device's panel that is a
 *    smear. Supporting text here is Normal or Medium, never Light.
 * 3. **No size below [MIN_BODY_SP].** The CSS scale bottomed out at 10px
 *    (`.rb-tag`) and 11px (`.rb-pill`, `.rb-table th`). Those are gone.
 * 4. **Tracking in `em`, like line height, not `sp`.** [TextStyle.letterSpacing]
 *    takes a [TextUnit] same as font size, and the same argument applies: a
 *    fixed-sp tracking value stops being proportional to the letterforms the
 *    instant the system font scale multiplies the glyphs but not the gap
 *    between them. Only the styles that need it declare it - a big semibold
 *    number reads tighter with a touch of negative tracking, a small label
 *    reads calmer with a touch of positive tracking - everything else is left
 *    at the font's own default rather than tracked "just because".
 *
 * Type is Public Sans (SIL OFL 1.1), bundled - see [RbBrandFont] for the
 * measured byte cost and why it ships as four static weight files instead
 * of one variable font. [FontFamily.Monospace] is kept only where it is
 * earning its keep: RUT and folios, where a letter (the RUT check digit) has
 * to sit the same width as every digit around it. CLP amounts used to share
 * that monospace style too - the plata is the most important thing this app
 * shows, and a terminal typeface made it look like a script's stdout. [amount]
 * is Public Sans instead, with `tnum` tabular figures so the digits still
 * line up without the typewriter texture.
 */
@Immutable
data class RbTypography(
    /**
     * The one figure that has to dominate the screen: the day's takings, the
     * arqueo difference. Larger than [title] on purpose - before this style
     * existed the biggest thing in the scale was a 22sp screen title, and a
     * headline number was just that same title stretched at the call site by
     * a hand-rolled multiplier (see the old `RbAmount.kt` history). One place
     * to be big, instead of every caller improvising its own.
     */
    val display: TextStyle,
    /** Screen title in the top bar. */
    val title: TextStyle,
    /** Section and card heading. */
    val heading: TextStyle,
    /** Body copy - the default for anything the user reads. */
    val body: TextStyle,
    /** Body copy that carries emphasis inside a paragraph. */
    val bodyStrong: TextStyle,
    /** Supporting copy: hints, captions, field help. */
    val support: TextStyle,
    /** Field label above an input. */
    val label: TextStyle,
    /** Label inside a button. */
    val button: TextStyle,
    /**
     * RUT and folios - monospace so every character lines up, digit or the
     * RUT's check letter alike. Not for money: see [amount].
     */
    val numeric: TextStyle,
    /**
     * CLP amounts - Public Sans with `tnum` tabular figures, so the digits
     * still line up column to column without borrowing a terminal typeface's
     * look for the plata. [cl.rutbusiness.ui.components.RbAmount] is the
     * component that consumes this; reach for it there rather than styling a
     * bare `Text` with this directly.
     */
    val amount: TextStyle,
    /** Chip / pill label. */
    val chip: TextStyle,
) {
    companion object {
        /**
         * The smallest text size in the product, in sp.
         *
         * Chosen at 14: Material's `bodySmall` is 12sp and the CSS scale went to
         * 10px. On the reference panel (720p, older eyes) neither is readable,
         * and the difference is invisible in the layout because everything
         * scales together anyway.
         */
        const val MIN_BODY_SP = 14f
    }
}

/** Line height as a multiple of the font size, so it scales with the glyphs. */
private val TightLines: TextUnit = 1.25.em
private val NormalLines: TextUnit = 1.45.em

/** Trim the extra first-line/last-line padding so a scaled line box does not
 *  push a one-line label out of its 56dp control. */
private val TrimmedLines = LineHeightStyle(
    alignment = LineHeightStyle.Alignment.Center,
    trim = LineHeightStyle.Trim.None,
)

/**
 * `"tnum" 1` in CSS `font-feature-settings` syntax - the format
 * [TextStyle.fontFeatureSettings] itself documents. Tabular figures: every
 * digit glyph claims the same advance width, so a column of [amount] values
 * lines up the way monospace used to, without switching the whole word to a
 * monospace face just because it contains digits.
 */
private const val TabularFigures = "\"tnum\" 1"

val RbDefaultTypography = RbTypography(
    display = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.Bold,
        fontSize = 34.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
        // Tight tracking on a big bold headline keeps large digits and
        // letterforms from feeling loose at a size this size range never had
        // before this style existed.
        letterSpacing = (-0.02).em,
    ),
    title = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.SemiBold,
        fontSize = 22.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
        letterSpacing = (-0.01).em,
    ),
    heading = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.SemiBold,
        fontSize = 18.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
        letterSpacing = (-0.005).em,
    ),
    body = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.Normal,
        fontSize = 17.sp,
        lineHeight = NormalLines,
        lineHeightStyle = TrimmedLines,
    ),
    bodyStrong = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.SemiBold,
        fontSize = 17.sp,
        lineHeight = NormalLines,
        lineHeightStyle = TrimmedLines,
    ),
    support = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.Normal,
        fontSize = 15.sp,
        lineHeight = NormalLines,
        lineHeightStyle = TrimmedLines,
    ),
    label = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.Medium,
        fontSize = 15.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
        // Small text reads calmer with a touch of room between letters.
        letterSpacing = 0.01.em,
    ),
    button = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.SemiBold,
        fontSize = 17.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
    ),
    numeric = TextStyle(
        fontFamily = FontFamily.Monospace,
        fontWeight = FontWeight.Medium,
        fontSize = 17.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
    ),
    amount = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.SemiBold,
        fontSize = 17.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
        fontFeatureSettings = TabularFigures,
    ),
    chip = TextStyle(
        fontFamily = RbBrandFont,
        fontWeight = FontWeight.Medium,
        fontSize = 14.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
        letterSpacing = 0.02.em,
    ),
)

internal val LocalRbTypography = staticCompositionLocalOf { RbDefaultTypography }
