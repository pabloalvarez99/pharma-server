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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * One row of a list — a line of the cuaderno, scannable under feria sun.
 *
 * Hierarchy is fixed so the eye finds plata and product without hunting:
 *
 * - **Title** in [cl.rutbusiness.ui.theme.RbTypography.bodyStrong]: the subject
 *   (product, customer, sale) carries weight, not just size.
 * - **Subtitle** in support / secondary: context, never competition.
 * - **Value** via [RbAmount]: bold monospace, one line, never split mid-digit.
 *   A column of prices has to line up and read at a glance.
 *
 * Interactive rows are at least [cl.rutbusiness.ui.theme.RbDimens.touchTarget]
 * (56dp) tall via [rbTouchTarget]. Press feedback lives in [rbClickable] — color
 * only, no translateY — so reduced motion stays a snap, never a shift under the
 * finger.
 *
 * @param title the row's subject - a product, a sale, a customer.
 * @param subtitle supporting line. Optional.
 * @param value trailing figure: a price, a stock count, a folio. Routed through
 *   [RbAmount] so a column of amounts lines up and never wraps mid-figure.
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
        // Still min 56dp tall: a non-clickable row sits next to clickable ones and
        // a thin row next to a fat one reads as a different list.
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
            // space3 vertical: mesa air so title + subtitle + 56dp floor never
            // feel like a dense SaaS table under a fat finger.
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        content = {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
                Text(
                    text = title,
                    style = RbTheme.typography.bodyStrong,
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
                        RbAmount(
                            amount = value,
                            emphasis = RbAmountEmphasis.Body,
                            color = colors.textPrimary,
                            textAlign = TextAlign.End,
                        )
                    }
                    trailing?.invoke()
                }
            }
        },
    )
}

/**
 * A hairline between rows.
 *
 * Decorative only (not announced). Stroke width comes from
 * [cl.rutbusiness.ui.theme.RbDimens.border] so the list edge and the row
 * separators share one token — never a raw `1.dp`.
 */
@Composable
fun RbDivider(modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = dimens.space3)
            .height(dimens.border)
            .background(RbTheme.colors.outline),
    )
}

/**
 * A virtualised list — the cuaderno pages, not a Column dumped into RAM.
 *
 * `LazyColumn`, never a `Column` in a scroller: the hardware floor budgets
 * 1-2 GB of RAM total and forbids holding a whole result set in memory. This is
 * the piece the WebView could not give the product - the audit found zero
 * virtualisation across 37 views.
 *
 * The three states are part of the component rather than the caller's problem,
 * which is what stops a screen from shipping without them. Empty / error sit in
 * a vertical scroller so a 200% font scale still reaches the hint and the retry
 * button on a 640dp-tall panel.
 *
 * No list-level motion (no staggered row fades, no translateY on appear): the
 * rows are content, and reduced motion must not leave a blank frame waiting for
 * an entrance animation that never runs.
 */
@Composable
fun <T> RbList(
    items: List<T>,
    modifier: Modifier = Modifier,
    loading: Boolean = false,
    /**
     * Qué está trayendo, dicho como lo diría una persona: "Buscando productos".
     *
     * Tiene default para no romper a nadie, pero pasarlo es lo correcto. El
     * resto de las pantallas del producto anuncian qué esperan —"Viendo cómo
     * está la caja", "Viendo quién te debe"— y una lista que dice "Cargando" a
     * secas se siente de otra app.
     */
    loadingLabel: String = "Cargando...",
    error: RbErrorCopy? = null,
    onRetry: (() -> Unit)? = null,
    emptyTitle: String = "Todavía no hay nada acá",
    emptyHint: String? = null,
    emptyActionLabel: String? = null,
    onEmptyAction: (() -> Unit)? = null,
    key: ((T) -> Any)? = null,
    row: @Composable (T) -> Unit,
) {
    // Los tres estados que no son lista van adentro de un scroller.
    //
    // Este componente ocupa el hueco que la lista habría llenado, y ese hueco
    // tiene altura fija — los que lo usan le pasan un `weight(1f)`. Al 200% de
    // escala el vacío no entra ahí: el título se veía y la **pista** —la parte
    // que enseña qué hacer, o sea la razón de ser del estado vacío— quedaba
    // debajo del borde, invisible justo para quien subió la letra porque ve
    // poco. Lo mismo el mensaje de una falla, que es aún más largo.
    //
    // El scroller va acá y no adentro de `RbEmptyState`: ese componente también
    // se usa dentro de pantallas que ya scrollean —`PasoPago`, el showcase— y
    // dos scrollers verticales anidados no son un detalle de estilo, revientan
    // con "measured with an infinity maximum height constraints".
    val enUnScroller: @Composable (@Composable () -> Unit) -> Unit = { contenido ->
        Column(
            modifier = modifier.verticalScroll(rememberScrollState()),
        ) { contenido() }
    }

    when {
        error != null -> enUnScroller {
            RbErrorState(
                title = error.title,
                message = error.message,
                modifier = Modifier.padding(RbTheme.dimens.space3),
                retryLabel = error.retryLabel,
                onRetry = onRetry,
            )
        }

        loading -> Column(modifier = modifier) {
            RbLoadingState(label = loadingLabel)
            RbSkeletonLines(lines = 5, modifier = Modifier.padding(RbTheme.dimens.space3))
        }

        items.isEmpty() -> enUnScroller {
            RbEmptyState(
                title = emptyTitle,
                hint = emptyHint,
                actionLabel = emptyActionLabel,
                onAction = onEmptyAction,
            )
        }

        else -> LazyColumn(modifier = modifier.fillMaxWidth()) {
            items(items = items, key = key) { item ->
                row(item)
                RbDivider()
            }
        }
    }
}
