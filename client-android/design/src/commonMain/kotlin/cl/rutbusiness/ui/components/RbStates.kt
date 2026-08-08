package cl.rutbusiness.ui.components

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbAssertive
import cl.rutbusiness.ui.theme.rbHeading
import cl.rutbusiness.ui.theme.rbPolite

/**
 * The three states no list or panel is allowed to skip: loading, empty, error.
 *
 * Ported from `loadingState` / `emptyState` / `errorState` in
 * `client/src/views/ui.ts`, which existed for exactly this reason - so a fresh
 * install never shows a bare paragraph that reads "de dev".
 */

/**
 * Loading.
 *
 * Announced politely to TalkBack, matching the web helper's
 * `role="status" aria-live="polite"`.
 *
 * When the user asked the system for reduced animation, the ring is drawn
 * **static** rather than removed: a still ring next to "Cargando..." still says
 * the app is working. This is the port of the CSS's
 * `@media (prefers-reduced-motion: reduce) { .ui-spinner { animation: none } }`.
 */
@Composable
fun RbLoadingState(
    modifier: Modifier = Modifier,
    label: String = "Cargando...",
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val sweep = RbTheme.motion.spinnerSweep()

    val rotation: Float = if (sweep == null) {
        // Reduced motion: a fixed angle, never a moving one.
        0f
    } else {
        val transition = rememberInfiniteTransition(label = "RbLoadingState spinner")
        val animated by transition.animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = 900, easing = androidx.compose.animation.core.LinearEasing),
                repeatMode = RepeatMode.Restart,
            ),
            label = "RbLoadingState angle",
        )
        animated
    }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(dimens.space3)
            .rbPolite(),
        horizontalArrangement = Arrangement.spacedBy(dimens.space2),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Canvas(
            modifier = Modifier
                .size(dimens.iconSize)
                .clearAndSetSemantics { },
        ) {
            val stroke = 3.dp.toPx()
            val inset = stroke / 2f
            val arcSize = Size(size.width - stroke, size.height - stroke)
            // The full ring at low contrast, then the bright arc on top: the
            // gap between them is what reads as progress.
            drawArc(
                color = colors.outline,
                startAngle = 0f,
                sweepAngle = 360f,
                useCenter = false,
                topLeft = androidx.compose.ui.geometry.Offset(inset, inset),
                size = arcSize,
                style = Stroke(width = stroke),
            )
            drawArc(
                color = colors.brandText,
                startAngle = rotation,
                sweepAngle = 90f,
                useCenter = false,
                topLeft = androidx.compose.ui.geometry.Offset(inset, inset),
                size = arcSize,
                style = Stroke(width = stroke),
            )
        }
        Text(
            text = label,
            style = RbTheme.typography.body,
            color = colors.textSecondary,
        )
    }
}

/**
 * Empty.
 *
 * The rule from `ui.ts`: an empty state **teaches the next step**. `title` says
 * what is missing, `hint` says how to make the first one, and the optional
 * action takes the user there. "No hay datos" alone is a dead end and is not
 * allowed in this product.
 */
@Composable
fun RbEmptyState(
    title: String,
    modifier: Modifier = Modifier,
    hint: String? = null,
    actionLabel: String? = null,
    onAction: (() -> Unit)? = null,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = dimens.space4, vertical = dimens.space5),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        // The soft mark from `.ui-empty-mark`. Decorative, so it is hidden from
        // TalkBack rather than read as a shape with no meaning.
        Box(
            modifier = Modifier
                .size(dimens.markSize)
                .clip(RoundedCornerShape(dimens.radiusCard))
                .background(colors.brandContainer)
                // Brand-toned edge, not the neutral one: a warm grey outline
                // around a mint tile reads muddy in the light theme, and
                // brandText clears 3.0 on the container either way.
                .border(dimens.border, colors.brandText, RoundedCornerShape(dimens.radiusCard))
                .clearAndSetSemantics { },
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = "+",
                style = RbTheme.typography.title,
                color = colors.brandText,
            )
        }

        Text(
            text = title,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            textAlign = TextAlign.Center,
            modifier = Modifier.rbHeading(),
        )

        if (hint != null) {
            Text(
                text = hint,
                style = RbTheme.typography.body,
                color = colors.textSecondary,
                textAlign = TextAlign.Center,
            )
        }

        if (actionLabel != null && onAction != null) {
            Box(modifier = Modifier.padding(top = dimens.space2)) {
                RbButton(label = actionLabel, onClick = onAction)
            }
        }
    }
}

/**
 * Error.
 *
 * Announced assertively, matching the web helper's `role="alert"`.
 *
 * The message must say **what to do**, in plain Chilean Spanish, with no
 * technical jargon - see [RbErrorCopy], which is the ported and rewritten
 * version of `classifyFetchError`.
 */
@Composable
fun RbErrorState(
    title: String,
    message: String,
    modifier: Modifier = Modifier,
    retryLabel: String? = "Reintentar",
    onRetry: (() -> Unit)? = null,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.dangerContainer)
            .border(dimens.border, colors.dangerText, shape)
            .padding(dimens.space3)
            .rbAssertive(),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = title,
            style = RbTheme.typography.heading,
            // On the tinted danger container, primary text is the AAA choice;
            // the danger hue is carried by the border and the mark instead.
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        Text(
            text = message,
            style = RbTheme.typography.body,
            color = colors.textPrimary,
        )
        if (retryLabel != null && onRetry != null) {
            RbButton(
                label = retryLabel,
                onClick = onRetry,
                variant = RbButtonVariant.Secondary,
            )
        }
    }
}

/**
 * Skeleton placeholder lines, ported from `skeletonLines` / `.ui-sk-line`.
 *
 * The CSS animated a moving gradient. That is a repainting shader every frame,
 * which the reference device pays for, so this is a **static** block at rest -
 * and under reduced motion it stays static by construction rather than by an
 * override. Hidden from TalkBack: the announcement is
 * [RbLoadingState]'s job.
 */
@Composable
fun RbSkeletonLines(
    lines: Int,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    // Each bar is as tall as a line of body text, so the placeholder occupies
    // the space the real content will and grows with the system font scale.
    // A fixed dp height would make the skeleton and the text that replaces it
    // two different shapes at 200%, and the screen would jump on load.
    val lineHeight = with(LocalDensity.current) {
        RbTheme.typography.body.fontSize.toDp()
    }

    // Ragged ends, so the block reads as text rather than as a table.
    val widths = listOf(1f, 0.82f, 0.94f, 0.7f)

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clearAndSetSemantics { },
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        repeat(lines.coerceAtLeast(1)) { index ->
            Box(
                modifier = Modifier
                    .fillMaxWidth(widths[index % widths.size])
                    .height(lineHeight)
                    // Filled, not outlined. An outlined bar reads as an empty
                    // input field waiting for typing, which is the wrong
                    // message while data is on its way.
                    .clip(RoundedCornerShape(dimens.space1))
                    .background(colors.surfaceVariant),
            )
        }
    }
}
