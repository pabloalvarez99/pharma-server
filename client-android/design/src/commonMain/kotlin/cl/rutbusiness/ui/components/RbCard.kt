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
import cl.rutbusiness.ui.theme.rbElevated
import cl.rutbusiness.ui.theme.rbHeading

/**
 * A card shell — a piece of the mesa del puesto, not a floating SaaS panel.
 *
 * Ported from `.ui-card` / `.rb-card`. The CSS carried a drop shadow
 * (`--rb-shadow`, a 40px blur); the border was added instead because that
 * blur cost a per-frame GPU pass and vanished against a light surface in
 * daylight. The card now carries **both**: [cl.rutbusiness.ui.theme.rbElevated]
 * draws a shallow native shadow (cheap - see [cl.rutbusiness.ui.theme.RbDimens.elevationCard]
 * for why it is not the same cost as the CSS blur), and the border stays as
 * the cue that still works when the shadow washes out outdoors. Depth is
 * never the *only* signal.
 *
 * The edge uses [cl.rutbusiness.ui.theme.RbColors.outlineStrong] (>= 3.0), not
 * the decorative hairline: under feria sun a 1.27 outline disappears and the
 * card melts into the kraft background. Padding and section gaps stay generous
 * so the content reads as a thick mesa tile, not a dense admin widget.
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
            .rbElevated(dimens.elevationCard, shape)
            .clip(shape)
            .background(colors.surface)
            .border(dimens.border, colors.outlineStrong, shape)
            .padding(dimens.space4),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        if (title != null || actions != null) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(dimens.space2),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (title != null) {
                    Text(
                        text = title,
                        style = RbTheme.typography.heading,
                        color = colors.textPrimary,
                        // weight keeps a long heading from shoving actions off
                        // the row at 200% font scale.
                        modifier = Modifier
                            .weight(1f)
                            .rbHeading(),
                    )
                }
                actions?.invoke()
            }
        }
        content()
    }
}
