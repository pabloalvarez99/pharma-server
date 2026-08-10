package cl.rutbusiness.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Spacing, touch targets and shapes.
 *
 * These stay in `dp`. A `dp` is a *physical* size, and a finger does not get
 * bigger when the font scale does - so growing the touch target with the text
 * would be wrong. What must grow is the control's *height*, and it does:
 * components use [RbDimens.touchTarget] as a **minimum**, then let the scaled
 * text push them taller. Never a fixed height.
 *
 * Ported from the `--ui-space-*` ramp in `brand.css` (6/10/16/24/40px), rounded
 * to the 4dp grid Compose lays out on.
 */
@Immutable
data class RbDimens(
    /** 6px in the CSS ramp. Gap inside a tight cluster (icon to its label). */
    val space1: Dp,
    /** 10px. Gap between related rows. */
    val space2: Dp,
    /** 16px. Default padding inside a card or screen gutter. */
    val space3: Dp,
    /** 24px. Gap between sections. */
    val space4: Dp,
    /** 40px. Vertical breathing room around a full-screen state. */
    val space5: Dp,

    /**
     * Minimum height and width of anything that reacts to a tap.
     *
     * 56dp, not Material's 48dp. The hardware floor raises it because the user
     * may be an older person whose aim is not a 25-year-old cashier's. Every
     * interactive component in this package applies it via
     * `Modifier.rbTouchTarget()`, and `RbTouchTargetTest` measures the rendered
     * bounds to prove it - at 100% and at 200% font scale.
     */
    val touchTarget: Dp,

    /** Border width of an unfilled control. 1dp reads as a hairline on a 720p
     *  panel; 1.5dp survives it. */
    val border: Dp,
    /** Focus ring stroke. Solid and thick enough to see outdoors. */
    val focusRing: Dp,
    /** Card corner. `--rb-radius`. */
    val radiusCard: Dp,
    /** Field and button corner. `--rb-radius-sm`. */
    val radiusField: Dp,
    /** Chip corner - a pill. */
    val radiusPill: Dp,
    /** Icon inside a button or list row. */
    val iconSize: Dp,
    /** The soft mark at the top of an empty state. */
    val markSize: Dp,
)

val RbDefaultDimens = RbDimens(
    space1 = 4.dp,
    space2 = 8.dp,
    space3 = 16.dp,
    space4 = 24.dp,
    space5 = 40.dp,

    touchTarget = 56.dp,

    border = 1.5.dp,
    focusRing = 3.dp,
    radiusCard = 14.dp,
    radiusField = 10.dp,
    radiusPill = 999.dp,
    iconSize = 24.dp,
    markSize = 56.dp,
)

/** Corner shapes derived from [RbDimens], so a component asks for a shape and
 *  not for a number. */
@Immutable
data class RbShapes(
    val card: RoundedCornerShape,
    val field: RoundedCornerShape,
    val pill: RoundedCornerShape,
)

val RbDefaultShapes = RbShapes(
    card = RoundedCornerShape(RbDefaultDimens.radiusCard),
    field = RoundedCornerShape(RbDefaultDimens.radiusField),
    pill = RoundedCornerShape(percent = 50),
)

internal val LocalRbDimens = staticCompositionLocalOf { RbDefaultDimens }
internal val LocalRbShapes = staticCompositionLocalOf { RbDefaultShapes }
