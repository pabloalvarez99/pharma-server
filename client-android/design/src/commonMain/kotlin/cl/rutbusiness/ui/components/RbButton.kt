package cl.rutbusiness.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.InfiniteRepeatableSpec
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbElevated
import cl.rutbusiness.ui.theme.rbPolite
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * Button variants, ported one-to-one from `ui.ts`'s `ButtonVariant`.
 *
 * The CSS names were `primary` / `ghost` / `danger`. `ghost` is renamed
 * [Secondary] because the ported control is not a ghost any more: it carries a
 * real 1.5dp border at >= 3.0 contrast. A borderless secondary action is
 * undiscoverable on a washed-out panel in daylight.
 */
enum class RbButtonVariant { Primary, Secondary, Destructive }

/**
 * The product's button — the chunky control on the mesa del puesto.
 *
 * Hardware-floor properties, all of them measured rather than asserted:
 *
 * - at least 56dp tall via [rbTouchTarget], and taller when the font scale
 *   pushes the label past it;
 * - the label is `sp` and wraps to a second line instead of ellipsing, because
 *   a truncated "Confirmar venta" is a button whose meaning is gone at 200%;
 * - **no press transform.** `ui.ts` shipped `translateY(1px)`; a control that
 *   shifts under the finger is harder to hit. Press feedback is a color change
 *   through [cl.rutbusiness.ui.theme.RbMotion.quick], which snaps to the end
 *   state under reduced motion.
 * - **filled variants lift off the mesa.** [RbButtonVariant.Primary] and
 *   [RbButtonVariant.Destructive] have no border, so a shallow
 *   [cl.rutbusiness.ui.theme.rbElevated] shadow is their only depth cue - it
 *   drops to flat the instant [enabled] is false, same as the fill does.
 *   [RbButtonVariant.Secondary] stays flat; its border already reads as a
 *   control edge and does not need the shadow to say so twice.
 *
 * @param fillWidth stretch to the parent's width. The default `false` keeps a
 *   button hugging its label; the showcase and dialogs pass `true`.
 * @param loading shows a spinner beside the label and ignores taps, without
 *   flattening toward [enabled]'s false look. The distinction matters: a
 *   disabled button says "you can't do this," a loading one says "you already
 *   did, hang on" - so it keeps its full colour and only the spinner and the
 *   dropped click say anything changed. Defaults to `false` so no existing
 *   call site has to pass it; screens that today swap their own label text for
 *   "Cifrando / enviando..." while disabling the button (see
 *   `PantallaDeCola.kt`) are exactly the case this replaces.
 */
@Composable
fun RbButton(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    variant: RbButtonVariant = RbButtonVariant.Primary,
    enabled: Boolean = true,
    fillWidth: Boolean = false,
    loading: Boolean = false,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val motion = RbTheme.motion

    val interactionSource = remember { MutableInteractionSource() }
    val pressed by interactionSource.collectIsPressedAsState()

    // A button that is working is not the same state as one that is
    // unavailable: it keeps enabled's full colour (below) and only stops
    // reacting to taps, so the user does not read "disabled" while their
    // action is in flight.
    val tappable = enabled && !loading

    val container: Color
    val content: Color
    val border: BorderStroke?

    when (variant) {
        RbButtonVariant.Primary -> {
            // Flat brand fill — no gradient (gradients make contrast unmeasurable
            // and cost a shader on the reference panel).
            container = colors.brandFill
            content = colors.onBrandFill
            border = null
        }

        RbButtonVariant.Secondary -> {
            // Solid surface + outlineStrong so the secondary reads outdoors as a
            // real control, not a washed-out ghost on kraft paper.
            container = colors.surface
            content = colors.textPrimary
            border = BorderStroke(dimens.border, colors.outlineStrong)
        }

        RbButtonVariant.Destructive -> {
            container = colors.dangerFill
            content = colors.onDangerFill
            border = null
        }
    }

    // Press state darkens or lightens the fill toward the surface it sits on,
    // so the feedback is visible in both themes without a second token and
    // without moving the control under the finger.
    val pressedContainer = if (colors.isDark) {
        container.copy(alpha = 0.82f).compositeOverOpaque(colors.surface)
    } else {
        container.copy(alpha = 0.86f).compositeOverOpaque(colors.background)
    }

    val animatedContainer by animateColorAsState(
        targetValue = if (pressed && tappable) pressedContainer else container,
        animationSpec = motion.quick(),
        label = "RbButton container",
    )

    // Disabled is expressed by flattening toward the surface, never by dropping
    // alpha on the label: a 38%-alpha label is the classic Material grey-on-grey
    // that fails contrast outright. Loading is deliberately excluded from this
    // branch - see the `loading` KDoc above - so it keeps enabled's full colour.
    val effectiveContainer = if (enabled) animatedContainer else colors.surfaceVariant
    val effectiveContent = if (enabled) content else colors.textTertiary
    val effectiveBorder = when {
        !enabled -> BorderStroke(dimens.border, colors.outline)
        else -> border
    }

    // Only the borderless fills (Primary/Destructive) lift with a shadow; a
    // bordered Secondary already has its own edge as the depth cue. Drops to
    // flat on disabled, a static swap like the fill above - never animated,
    // see rbElevated's KDoc.
    val elevation = if (enabled && border == null) dimens.elevationCard else 0.dp

    val shape: RoundedCornerShape = RbTheme.shapes.field

    Box(
        modifier = modifier
            .then(if (fillWidth) Modifier.fillMaxWidth() else Modifier)
            .rbElevated(elevation, shape)
            .clip(shape)
            .background(effectiveContainer)
            .then(
                effectiveBorder?.let { Modifier.border(it, shape) } ?: Modifier,
            )
            .rbClickable(
                onClick = onClick,
                enabled = tappable,
                role = Role.Button,
                interactionSource = interactionSource,
                shape = shape,
            )
            .rbTouchTarget()
            .then(if (loading) Modifier.rbPolite() else Modifier)
            .padding(
                // Wider horizontal air than a SaaS pill: the finger lands on a
                // real mesa control, not a 48dp Material stub.
                PaddingValues(
                    horizontal = dimens.space4,
                    vertical = dimens.space2,
                ),
            ),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(dimens.space2),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (loading) {
                RbButtonSpinner(color = effectiveContent)
            }
            Text(
                text = label,
                style = RbTheme.typography.button,
                color = effectiveContent,
                textAlign = TextAlign.Center,
                // No maxLines and no overflow: the label wraps. At 200% scale a
                // clipped verb is a button nobody can act on.
            )
        }
    }
}

/**
 * The spinner [RbButton] shows next to its label when its `loading` parameter
 * is true.
 *
 * Same construction as [RbLoadingState]'s ring - a static track plus a moving
 * arc, [color] on both so it always passes contrast on whatever fill it sits
 * on rather than owning a token of its own - but sized for a label instead of
 * a full-width row, and undecorated for TalkBack: the button's own label
 * already says what is happening, and [cl.rutbusiness.ui.theme.rbPolite] on
 * the button re-announces it, so a second description here would just repeat.
 */
@Composable
private fun RbButtonSpinner(color: Color) {
    val motion = RbTheme.motion
    val dimens = RbTheme.dimens
    val sweep = motion.spinnerSweep()
    val spinnerSize = 18.dp

    val rotation: Float = if (sweep == null) {
        0f
    } else {
        val infinite = sweep as InfiniteRepeatableSpec<Float>
        val transition = rememberInfiniteTransition(label = "RbButtonSpinner")
        val animated by transition.animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = infinite,
            label = "RbButtonSpinner angle",
        )
        animated
    }

    Canvas(
        modifier = Modifier
            .size(spinnerSize)
            .clearAndSetSemantics { },
    ) {
        val stroke = dimens.border.toPx()
        val inset = stroke / 2f
        val arcSize = Size(size.width - stroke, size.height - stroke)
        drawArc(
            color = color.copy(alpha = 0.35f),
            startAngle = 0f,
            sweepAngle = 360f,
            useCenter = false,
            topLeft = Offset(inset, inset),
            size = arcSize,
            style = Stroke(width = stroke),
        )
        drawArc(
            color = color,
            startAngle = rotation,
            sweepAngle = 90f,
            useCenter = false,
            topLeft = Offset(inset, inset),
            size = arcSize,
            style = Stroke(width = stroke),
        )
    }
}

/**
 * Flatten a translucent color onto an opaque one.
 *
 * Kept explicit because every color that reaches a contrast assertion has to be
 * opaque - a ratio against a half-transparent fill is not a real number.
 */
private fun Color.compositeOverOpaque(background: Color): Color {
    val a = alpha
    return Color(
        red = red * a + background.red * (1 - a),
        green = green * a + background.green * (1 - a),
        blue = blue * a + background.blue * (1 - a),
        alpha = 1f,
    )
}
