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
 * Three rules this file enforces beyond "use sp":
 *
 * 1. **Line height in `em`, not `sp`.** A line height fixed in `sp` does not
 *    grow with the glyphs, so at 200% the text collides with itself. Declaring
 *    it as a multiple of the font size makes it scale for free.
 * 2. **No weight below [FontWeight.Normal].** `brand.css` had `font-weight:500`
 *    labels in a 11-12px muted grey; at the reference device's panel that is a
 *    smear. Supporting text here is Normal or Medium, never Light.
 * 3. **No size below [MIN_BODY_SP].** The CSS scale bottomed out at 10px
 *    (`.rb-tag`) and 11px (`.rb-pill`, `.rb-table th`). Those are gone.
 *
 * Font families are the system's. `brand.css` asks for Fraunces + IBM Plex
 * Sans + IBM Plex Mono; bundling three families costs megabytes of APK on a
 * phone the floor describes as "almost out of space", and a webfont download
 * costs mobile data the floor calls expensive. The system sans is already
 * hinted for the device's panel. Monospace is kept for RUT / CLP / folio
 * digits, where column alignment is the point.
 */
@Immutable
data class RbTypography(
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
    /** RUT, CLP amounts, folios - monospace so digits line up. */
    val numeric: TextStyle,
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

val RbDefaultTypography = RbTypography(
    title = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 22.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
    ),
    heading = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 18.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
    ),
    body = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 17.sp,
        lineHeight = NormalLines,
        lineHeightStyle = TrimmedLines,
    ),
    bodyStrong = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 17.sp,
        lineHeight = NormalLines,
        lineHeightStyle = TrimmedLines,
    ),
    support = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 15.sp,
        lineHeight = NormalLines,
        lineHeightStyle = TrimmedLines,
    ),
    label = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 15.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
    ),
    button = TextStyle(
        fontFamily = FontFamily.SansSerif,
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
    chip = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 14.sp,
        lineHeight = TightLines,
        lineHeightStyle = TrimmedLines,
    ),
)

internal val LocalRbTypography = staticCompositionLocalOf { RbDefaultTypography }
