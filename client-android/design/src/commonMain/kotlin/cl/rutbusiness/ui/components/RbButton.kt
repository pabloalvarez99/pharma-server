package cl.rutbusiness.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.BorderStroke
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
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
 * The product's button.
 *
 * Hardware-floor properties, all of them measured rather than asserted:
 *
 * - at least 56dp tall via [rbTouchTarget], and taller when the font scale
 *   pushes the label past it;
 * - the label is `sp` and wraps to a second line instead of ellipsing, because
 *   a truncated "Confirmar venta" is a button whose meaning is gone at 200%;
 * - no press transform. `ui.ts` shipped `translateY(1px)`; a control that
 *   shifts under the finger is harder to hit. Press feedback is a color change,
 *   which costs nothing on a 60 Hz panel and survives reduced-motion.
 *
 * @param fillWidth stretch to the parent's width. The default `false` keeps a
 *   button hugging its label; the showcase and dialogs pass `true`.
 */
@Composable
fun RbButton(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    variant: RbButtonVariant = RbButtonVariant.Primary,
    enabled: Boolean = true,
    fillWidth: Boolean = false,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val motion = RbTheme.motion

    val interactionSource = remember { MutableInteractionSource() }
    val pressed by interactionSource.collectIsPressedAsState()

    val container: Color
    val content: Color
    val border: BorderStroke?

    when (variant) {
        RbButtonVariant.Primary -> {
            container = colors.brandFill
            content = colors.onBrandFill
            border = null
        }

        RbButtonVariant.Secondary -> {
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
    // so the feedback is visible in both themes without a second token.
    val pressedContainer = if (colors.isDark) {
        container.copy(alpha = 0.82f).compositeOverOpaque(colors.surface)
    } else {
        container.copy(alpha = 0.86f).compositeOverOpaque(colors.background)
    }

    val animatedContainer by animateColorAsState(
        targetValue = if (pressed && enabled) pressedContainer else container,
        animationSpec = motion.quick(),
        label = "RbButton container",
    )

    // Disabled is expressed by flattening toward the surface, never by dropping
    // alpha on the label: a 38%-alpha label is the classic Material grey-on-grey
    // that fails contrast outright.
    val effectiveContainer = if (enabled) animatedContainer else colors.surfaceVariant
    val effectiveContent = if (enabled) content else colors.textTertiary
    val effectiveBorder = when {
        !enabled -> BorderStroke(dimens.border, colors.outline)
        else -> border
    }

    val shape: RoundedCornerShape = RbTheme.shapes.field

    Box(
        modifier = modifier
            .then(if (fillWidth) Modifier.fillMaxWidth() else Modifier)
            .clip(shape)
            .background(effectiveContainer)
            .then(
                effectiveBorder?.let { Modifier.border(it, shape) } ?: Modifier,
            )
            .rbClickable(
                onClick = onClick,
                enabled = enabled,
                role = Role.Button,
                interactionSource = interactionSource,
                shape = shape,
            )
            .rbTouchTarget()
            .padding(
                PaddingValues(
                    horizontal = dimens.space3,
                    vertical = dimens.space2,
                ),
            ),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(RbTheme.dimens.space2),
            verticalAlignment = Alignment.CenterVertically,
        ) {
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
