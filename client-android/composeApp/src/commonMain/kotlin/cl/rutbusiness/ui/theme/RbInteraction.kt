package cl.rutbusiness.ui.theme

import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.Role

/**
 * The one way anything in this design system becomes tappable.
 *
 * Centralised so three properties cannot be forgotten one component at a time:
 *
 * 1. **A visible focus ring.** Keyboard and switch-access users need to see
 *    where they are. `styles.css` drew it as a 3px soft glow
 *    (`--focus-ring: 0 0 0 3px rgba(...,.32)`); a 32%-alpha halo is not visible
 *    on a cheap panel outdoors, so this draws a **solid** stroke in
 *    [RbColors.focusRing].
 * 2. **The platform's own press indication**, via [LocalIndication], rather
 *    than a hand-rolled highlight. On Android that is the system ripple, which
 *    the vendor already tuned for the device.
 * 3. **A declared [Role]**, so TalkBack announces "botón" instead of reading a
 *    bare label.
 *
 * @param shape the same shape the caller clipped with, so the ring hugs the
 *   control instead of boxing it.
 */
fun Modifier.rbClickable(
    onClick: () -> Unit,
    enabled: Boolean = true,
    role: Role = Role.Button,
    onClickLabel: String? = null,
    shape: Shape,
    interactionSource: MutableInteractionSource? = null,
): Modifier = composed {
    val source = interactionSource ?: remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    this
        .then(
            if (focused && enabled) {
                Modifier.border(dimens.focusRing, colors.focusRing, shape)
            } else {
                Modifier
            },
        )
        .clickable(
            interactionSource = source,
            indication = LocalIndication.current,
            enabled = enabled,
            role = role,
            onClickLabel = onClickLabel,
            onClick = onClick,
        )
}
