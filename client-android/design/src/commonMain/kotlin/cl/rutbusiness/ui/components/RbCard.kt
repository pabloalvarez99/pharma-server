package cl.rutbusiness.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * A card shell.
 *
 * Ported from `.ui-card` / `.rb-card`. The CSS carried a drop shadow
 * (`--rb-shadow`, a 40px blur); this draws a 1.5dp border instead. Two reasons,
 * and both come from the hardware floor: a large-radius shadow is a real GPU
 * cost per frame on the reference device, and a shadow is invisible against a
 * light surface in daylight while a border is not.
 *
 * @param title optional heading. Rendered as a heading for TalkBack.
 * @param actions optional trailing slot in the header - put an [RbButton] here.
 */
@Composable
fun RbCard(
    modifier: Modifier = Modifier,
    title: String? = null,
    actions: (@Composable () -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surface)
            .border(dimens.border, colors.outline, shape)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        if (title != null || actions != null) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (title != null) {
                    Text(
                        text = title,
                        style = RbTheme.typography.heading,
                        color = colors.textPrimary,
                        modifier = Modifier.rbHeading(),
                    )
                }
                actions?.invoke()
            }
        }
        content()
    }
}
