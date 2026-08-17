package cl.rutbusiness.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbElevated
import cl.rutbusiness.ui.theme.rbTouchTarget

/** What a chip is saying. Ported from `.rb-pill` (`ok` / `rx` / `ctrl`). */
enum class RbChipTone { Neutral, Brand, Warn, Danger, Info }

/**
 * The container chips belong in.
 *
 * A plain `Row` of chips is a trap, and the 200% screenshots caught it: three
 * filter chips that fit comfortably at 100% overflowed the screen at 200%, the
 * last one collapsing to a single letter per line and the one after it cut to
 * "Me". Chips are content-sized, so their row has to wrap.
 *
 * Always put chips in this rather than in a `Row`.
 *
 * `content` is a plain composable lambda and not a `FlowRowScope` one on
 * purpose: `FlowRowScope` is experimental, and exposing it would force every
 * call site in the app to carry an `@OptIn`. The opt-in stops here, and chips
 * have no use for the scope's `weight` anyway.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun RbChipRow(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    FlowRow(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(RbTheme.dimens.space2),
        verticalArrangement = Arrangement.spacedBy(RbTheme.dimens.space2),
    ) {
        content()
    }
}

/**
 * A chip — a paper label on the mesa, not a Material filter.
 *
 * Two shapes in one component, because they differ only in whether they react:
 * a **status pill** ([onClick] null) that labels a row, and a **filter chip**
 * ([onClick] set) the user taps.
 *
 * The CSS `.rb-pill` was 11px monospace in a 2px-tall padding box. Both parts
 * broke a rule: 11px is below the readable floor, and the box was nowhere near
 * tappable. Here the text is on the scaling ramp, and a chip that reacts gets
 * the full 56dp target - a non-interactive pill correctly does not, since
 * padding a label to 56dp would wreck the row rhythm for no gain.
 *
 * Status pills carry a soft filled body (existing containers / surfaceVariant)
 * so they read under feria sun; selected filters thicken the brand edge rather
 * than relying on hue alone. Press feedback is color only — never translateY —
 * and runs through [cl.rutbusiness.ui.theme.RbMotion.quick] so reduced motion
 * snaps. A selected, interactive chip also picks up a shallow
 * [cl.rutbusiness.ui.theme.rbElevated] shadow — a third, independent cue past
 * the thicker edge and the filled body, so "which filter is active" survives
 * for a colour-blind reader even if the brand tint reads flat to them.
 *
 * @param selected only meaningful when [onClick] is set; announced to TalkBack.
 */
@Composable
fun RbChip(
    label: String,
    modifier: Modifier = Modifier,
    tone: RbChipTone = RbChipTone.Neutral,
    selected: Boolean = false,
    onClick: (() -> Unit)? = null,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val motion = RbTheme.motion
    val shape = RbTheme.shapes.pill

    val contentColor = when (tone) {
        RbChipTone.Neutral -> colors.textSecondary
        RbChipTone.Brand -> colors.brandText
        RbChipTone.Warn -> colors.warnText
        RbChipTone.Danger -> colors.dangerText
        RbChipTone.Info -> colors.infoText
    }

    // Soft body per tone: a paper label on the mesa, not a hairline ghost.
    // Only brand/danger have dedicated containers; the rest sit on surfaceVariant
    // so warn/info still have body without inventing tokens.
    val toneBody = when (tone) {
        RbChipTone.Neutral -> colors.surface
        RbChipTone.Brand -> colors.brandContainer
        RbChipTone.Danger -> colors.dangerContainer
        RbChipTone.Warn, RbChipTone.Info -> colors.surfaceVariant
    }

    // Selection is carried by a filled brand body AND a thicker border, not by
    // hue alone - the same reason the text field thickens its edge on error.
    val restBackground = if (selected) colors.brandContainer else toneBody
    val borderColor = when {
        selected -> colors.brandText
        // A toned chip borders in its own tone. The neutral outline is a warm
        // grey, and ringing a green "Al día" pill in brown reads as dirt. Every
        // tone colour clears 3.0 on all four surfaces, so this stays compliant.
        tone != RbChipTone.Neutral -> contentColor
        else -> colors.outlineStrong
    }
    val borderWidth = if (selected) dimens.focusRing else dimens.border
    val labelColor = if (selected) colors.textPrimary else contentColor

    // Always own an interaction source so press color is a single code path;
    // a non-clickable chip never receives presses, so the source stays idle.
    val interactionSource = remember { MutableInteractionSource() }
    val pressed by interactionSource.collectIsPressedAsState()

    val pressedBackground = if (colors.isDark) {
        restBackground.copy(alpha = 0.82f).compositeOverOpaque(colors.surface)
    } else {
        restBackground.copy(alpha = 0.86f).compositeOverOpaque(colors.background)
    }

    val animatedBackground by animateColorAsState(
        targetValue = if (pressed && onClick != null) pressedBackground else restBackground,
        animationSpec = motion.quick(),
        label = "RbChip background",
    )

    // Only a selected, tappable chip lifts - a status pill never moves and an
    // unselected filter has nothing to say is "up" yet.
    val elevation = if (selected && onClick != null) dimens.elevationCard else 0.dp

    val base = modifier
        .rbElevated(elevation, shape)
        .clip(shape)
        .background(animatedBackground)
        .border(borderWidth, borderColor, shape)

    val interactive = if (onClick != null) {
        base
            .rbClickable(
                onClick = onClick,
                role = Role.Tab,
                shape = shape,
                interactionSource = interactionSource,
            )
            .rbTouchTarget()
            .semantics { this.selected = selected }
    } else {
        base
    }

    Box(
        modifier = interactive.padding(
            // Horizontal air so a short filter ("Hoy") still feels like a
            // real finger target once rbTouchTarget lifts it to 56dp.
            horizontal = dimens.space3,
            vertical = dimens.space2,
        ),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            style = RbTheme.typography.chip,
            color = labelColor,
        )
    }
}

/** Flatten a translucent color onto an opaque one (same rule as RbButton). */
private fun Color.compositeOverOpaque(background: Color): Color {
    val a = alpha
    return Color(
        red = red * a + background.red * (1 - a),
        green = green * a + background.green * (1 - a),
        blue = blue * a + background.blue * (1 - a),
        alpha = 1f,
    )
}
