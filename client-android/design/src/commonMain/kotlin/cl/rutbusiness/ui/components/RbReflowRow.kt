package cl.rutbusiness.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import kotlin.math.max

/**
 * A leading / content / trailing row that **stacks the trailing slot onto a
 * second line** when the content's longest word would otherwise be split.
 *
 * This is the load-bearing layout of the design system at large font scales,
 * and it exists because of two defects the 200% screenshots showed:
 *
 * - the top bar rendered "compone / ntes", because a back arrow and a chip
 *   left the title column narrower than the word;
 * - a list row rendered "Paracetam / ol", and the next row collapsed to "I /
 *   b" - one letter per line - because a price and a status chip had taken
 *   almost the whole width.
 *
 * Wrapping between words is fine. Splitting a word in half is not, and for a
 * reader who already needed to set 200% type it is the difference between a
 * usable screen and an unreadable one.
 *
 * The decision uses the content's **minimum intrinsic width**, which for text
 * is the width of its longest word. That makes the rule exact and
 * self-adjusting: the row reflows when, and only when, a word would otherwise
 * be cut. There is no font-scale threshold to guess at, and nothing to
 * re-tune when someone writes a longer product name.
 *
 * @param spacing gap between slots, and between the two rows when stacked.
 * @param leading optional slot pinned to the start - a back arrow, an avatar.
 * @param content the slot that must never have a word broken.
 * @param trailing optional slot pinned to the end - a price, a chip, an action.
 */
@Composable
fun RbReflowRow(
    spacing: Dp,
    modifier: Modifier = Modifier,
    leading: @Composable () -> Unit = {},
    content: @Composable () -> Unit,
    trailing: @Composable () -> Unit = {},
) {
    Layout(
        modifier = modifier,
        contents = listOf(leading, content, trailing),
    ) { (leadingMeasurables, contentMeasurables, trailingMeasurables), constraints ->
        val gap = spacing.roundToPx()
        val available = constraints.maxWidth
        val loose = Constraints(maxWidth = available, maxHeight = Constraints.Infinity)

        val lead = leadingMeasurables.firstOrNull()?.measure(loose)
        val leadWidth = lead?.width ?: 0
        val leadGap = if (lead != null) gap else 0

        val trailMeasurable = trailingMeasurables.firstOrNull()
        val trailIntrinsic = trailMeasurable?.maxIntrinsicWidth(constraints.maxHeight) ?: 0
        val trailGap = if (trailMeasurable != null) gap else 0

        val contentMeasurable = contentMeasurables.first()
        // For text this is the longest word: the narrowest the content can get
        // before the layout has to break a word in half.
        val contentFloor = contentMeasurable.minIntrinsicWidth(constraints.maxHeight)

        val inlineWidth = available - leadWidth - leadGap - trailIntrinsic - trailGap
        val stacked = trailMeasurable != null && trailIntrinsic > 0 && inlineWidth < contentFloor

        val contentWidth = if (stacked) available - leadWidth - leadGap else inlineWidth

        val contentPlaceable = contentMeasurable.measure(
            Constraints(
                minWidth = 0,
                maxWidth = contentWidth.coerceAtLeast(0),
                maxHeight = Constraints.Infinity,
            ),
        )
        val trailPlaceable = trailMeasurable?.measure(loose)

        var firstRowHeight = max(lead?.height ?: 0, contentPlaceable.height)
        if (!stacked && trailPlaceable != null) {
            firstRowHeight = max(firstRowHeight, trailPlaceable.height)
        }
        val secondRowHeight = if (stacked && trailPlaceable != null) trailPlaceable.height + gap else 0

        layout(available, firstRowHeight + secondRowHeight) {
            lead?.placeRelative(x = 0, y = (firstRowHeight - lead.height) / 2)
            contentPlaceable.placeRelative(
                x = leadWidth + leadGap,
                y = (firstRowHeight - contentPlaceable.height) / 2,
            )
            trailPlaceable?.let {
                if (stacked) {
                    // Trailing edge of the second row, so the eye still finds
                    // the price or the action where it expects them.
                    it.placeRelative(x = available - it.width, y = firstRowHeight + gap)
                } else {
                    it.placeRelative(
                        x = available - it.width,
                        y = (firstRowHeight - it.height) / 2,
                    )
                }
            }
        }
    }
}
