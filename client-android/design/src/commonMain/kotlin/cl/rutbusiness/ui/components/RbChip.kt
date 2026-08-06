package cl.rutbusiness.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
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
 * A chip.
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
    val shape = RbTheme.shapes.pill

    val contentColor = when (tone) {
        RbChipTone.Neutral -> colors.textSecondary
        RbChipTone.Brand -> colors.brandText
        RbChipTone.Warn -> colors.warnText
        RbChipTone.Danger -> colors.dangerText
        RbChipTone.Info -> colors.infoText
    }

    // Selection is carried by a filled background AND a thicker border, not by
    // hue alone - the same reason the text field thickens its edge on error.
    val background = if (selected) colors.brandContainer else colors.surface
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

    val base = modifier
        .clip(shape)
        .background(background)
        .border(borderWidth, borderColor, shape)

    val interactive = if (onClick != null) {
        base
            .rbClickable(
                onClick = onClick,
                role = Role.Tab,
                shape = shape,
            )
            .rbTouchTarget()
            .semantics { this.selected = selected }
    } else {
        base
    }

    Box(
        modifier = interactive.padding(
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
