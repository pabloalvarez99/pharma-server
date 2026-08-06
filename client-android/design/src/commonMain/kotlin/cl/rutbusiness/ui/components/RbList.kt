package cl.rutbusiness.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * One row of a list.
 *
 * @param title the row's subject - a product, a sale, a customer.
 * @param subtitle supporting line. Optional.
 * @param value trailing figure: a price, a stock count, a folio. Rendered
 *   monospace so a column of amounts lines up.
 * @param trailing arbitrary trailing slot - typically an [RbChip].
 */
@Composable
fun RbListRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    value: String? = null,
    trailing: (@Composable () -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    val clickable = if (onClick != null) {
        Modifier
            .rbClickable(
                onClick = onClick,
                role = Role.Button,
                shape = androidx.compose.ui.graphics.RectangleShape,
            )
            .rbTouchTarget()
    } else {
        Modifier.rbTouchTarget()
    }

    // RbReflowRow, not a weighted Row. A `weight(1f)` name column shares the
    // width with the price and the chip, and at 200% that left so little room
    // that "Paracetamol" rendered as "Paracetam / ol" and "Ibuprofeno" as one
    // letter per line. Here the price and the chip drop to their own line
    // rather than squeeze the product name below its longest word.
    RbReflowRow(
        spacing = dimens.space3,
        modifier = modifier
            .fillMaxWidth()
            .then(clickable)
            .padding(horizontal = dimens.space3, vertical = dimens.space2),
        content = {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
                Text(
                    text = title,
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
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
        trailing = {
            if (value != null || trailing != null) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(dimens.space2),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (value != null) {
                        Text(
                            text = value,
                            style = RbTheme.typography.numeric,
                            color = colors.textPrimary,
                        )
                    }
                    trailing?.invoke()
                }
            }
        },
    )
}

/** A hairline between rows. Decorative, so it is not announced. */
@Composable
fun RbDivider(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = RbTheme.dimens.space3)
            .height(1.dp)
            .background(RbTheme.colors.outline),
    )
}

/**
 * A virtualised list.
 *
 * `LazyColumn`, never a `Column` in a scroller: the hardware floor budgets
 * 1-2 GB of RAM total and forbids holding a whole result set in memory. This is
 * the piece the WebView could not give the product - the audit found zero
 * virtualisation across 37 views.
 *
 * The three states are part of the component rather than the caller's problem,
 * which is what stops a screen from shipping without them.
 */
@Composable
fun <T> RbList(
    items: List<T>,
    modifier: Modifier = Modifier,
    loading: Boolean = false,
    error: RbErrorCopy? = null,
    onRetry: (() -> Unit)? = null,
    emptyTitle: String = "Todavía no hay nada acá",
    emptyHint: String? = null,
    emptyActionLabel: String? = null,
    onEmptyAction: (() -> Unit)? = null,
    key: ((T) -> Any)? = null,
    row: @Composable (T) -> Unit,
) {
    when {
        error != null -> RbErrorState(
            title = error.title,
            message = error.message,
            modifier = modifier.padding(RbTheme.dimens.space3),
            retryLabel = error.retryLabel,
            onRetry = onRetry,
        )

        loading -> Column(modifier = modifier) {
            RbLoadingState()
            RbSkeletonLines(lines = 5, modifier = Modifier.padding(RbTheme.dimens.space3))
        }

        items.isEmpty() -> RbEmptyState(
            title = emptyTitle,
            modifier = modifier,
            hint = emptyHint,
            actionLabel = emptyActionLabel,
            onAction = onEmptyAction,
        )

        else -> LazyColumn(modifier = modifier.fillMaxWidth()) {
            items(items = items, key = key) { item ->
                row(item)
                RbDivider()
            }
        }
    }
}
