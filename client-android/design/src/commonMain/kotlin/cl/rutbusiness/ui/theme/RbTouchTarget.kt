package cl.rutbusiness.ui.theme

import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.unit.Dp

/**
 * Guarantees the receiver is at least [RbDimens.touchTarget] (56dp) in both
 * axes, and lets it grow past that.
 *
 * `defaultMinSize` and not `size`, deliberately. A fixed `size` would be a trap
 * at 200% font scale: the label grows, the box does not, and the text clips -
 * which is precisely the failure the hardware floor calls "no esta listo".
 * A minimum lets the control stay 56dp when the text is small and become 70dp+
 * when the user scaled it up.
 *
 * Apply it to the **clickable** node, not to a wrapper. A 56dp box around a
 * 30dp hit area passes a screenshot and fails a finger.
 *
 * @param minWidth override for a control that is legitimately wider than tall
 *   (a full-width button); still never below 56dp.
 */
fun Modifier.rbTouchTarget(minWidth: Dp? = null): Modifier = composed {
    val target = RbTheme.dimens.touchTarget
    defaultMinSize(
        minWidth = minWidth ?: target,
        minHeight = target,
    )
}
