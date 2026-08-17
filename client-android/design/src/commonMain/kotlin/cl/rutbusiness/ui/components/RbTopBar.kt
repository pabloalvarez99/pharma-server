package cl.rutbusiness.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.graphics.RectangleShape
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbElevated
import cl.rutbusiness.ui.theme.rbHeading
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * The screen's top bar — the header of the puesto, not a floating Material app bar.
 *
 * Two things it does that the WebView client did not, both from the ADR's list
 * of what was broken on a real phone ("el header pisa la barra de estado",
 * "cero ocurrencias de safe-area-inset"):
 *
 * - it insets itself below the status bar with [WindowInsets.Companion.safeDrawing],
 *   so it never sits under the clock or a punch-hole camera;
 * - it has **no fixed height**. A `56.dp`-tall bar clips its own title the
 *   moment the user raises the font scale. The bar is as tall as its content
 *   needs, with 56dp only as a floor on the back affordance via [rbTouchTarget].
 *
 * The bottom edge uses [cl.rutbusiness.ui.theme.RbColors.outlineStrong] at the
 * real border token, not a decorative hairline: under feria sun a 1.27 line
 * vanishes and the title melts into the body. Padding is generous so title +
 * subtitle read as a thick mesa header at 100% and still reflow cleanly at 200%.
 * No translateY, no motion on the bar itself.
 *
 * A shallow [cl.rutbusiness.ui.theme.rbElevated] shadow sits under the whole
 * bar, so the header reads as a plane the content scrolls under rather than a
 * flat strip painted at the same depth as everything below it. The border
 * stays because it is the cue that survives when the shadow does not.
 *
 * @param subtitle second line - the business name, the branch. Optional.
 * @param onBack renders the back affordance when set.
 * @param actions trailing slot, typically an [RbButton] or an [RbChip].
 */
@Composable
fun RbTopBar(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    onBack: (() -> Unit)? = null,
    actions: (@Composable () -> Unit)? = null,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Column(
        modifier = modifier
            .fillMaxWidth()
            // Rectangular shadow: the bar has no rounded corners, so the same
            // clip=false shadow used by rounded surfaces just draws a flat
            // strip under the edge, which is exactly what a header wants.
            .rbElevated(dimens.elevationCard, RectangleShape)
            .background(colors.surface)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Top + WindowInsetsSides.Horizontal,
                ),
            ),
    ) {
        RbReflowRow(
            spacing = dimens.space2,
            modifier = Modifier
                .fillMaxWidth()
                // Horizontal space3 + vertical space3: air like the mesa cards,
                // so a long title wrapping at 200% still has room around it.
                .padding(horizontal = dimens.space3, vertical = dimens.space3),
            leading = {
                if (onBack != null) {
                    Box(
                        modifier = Modifier
                            .rbClickable(
                                onClick = onBack,
                                role = Role.Button,
                                onClickLabel = "Volver",
                                shape = RbTheme.shapes.field,
                            )
                            .rbTouchTarget(),
                        contentAlignment = Alignment.Center,
                    ) {
                        // A text glyph rather than a vector asset: it inherits
                        // the font scale, so the affordance grows with
                        // everything else. Bold, because rule five of the
                        // hardware floor rules out thin strokes.
                        Text(
                            text = "‹",
                            style = RbTheme.typography.title.copy(fontWeight = FontWeight.Bold),
                            color = colors.textPrimary,
                        )
                    }
                }
            },
            content = {
                Column {
                    Text(
                        text = title,
                        style = RbTheme.typography.title,
                        color = colors.textPrimary,
                        modifier = Modifier.rbHeading(),
                        // Deliberately no maxLines: at 200% a long screen title
                        // wraps to two lines and the bar grows to fit it.
                    )
                    if (subtitle != null) {
                        Text(
                            text = subtitle,
                            style = RbTheme.typography.support,
                            color = colors.textSecondary,
                        )
                    }
                }
            },
            trailing = { actions?.invoke() },
        )

        // Strong edge, token width: same outdoor rule as RbCard — a hairline
        // outline disappears in daylight and the header stops reading as a bar.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(dimens.border)
                .background(colors.outlineStrong),
        )
    }
}
