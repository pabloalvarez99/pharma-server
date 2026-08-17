package cl.rutbusiness.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbAssertive
import cl.rutbusiness.ui.theme.rbElevated
import cl.rutbusiness.ui.theme.rbHeading

/**
 * A confirmation dialog — a recado on the mesa, not a Material alert.
 *
 * Ported from `.modal` / `.modal-confirm` in `styles.css`, with the parts that
 * would break on the reference device removed:
 *
 * - **No `backdrop-filter: blur(4px)`.** A full-screen blur is a per-frame GPU
 *   pass, and this is a 2 GB device. A plain scrim separates the layers just as
 *   well, and it does not animate — reduced motion never has a half-blur frame.
 * - **No enter/exit motion.** No scale-in, no fade of the card, no translateY.
 *   The dialog is either there or not; [cl.rutbusiness.ui.theme.RbMotion] is not
 *   on the path so "Quitar animaciones" cannot leave a stuck 0-alpha sheet.
 * - **No `width: min(420px, 100%)` equivalent as a fixed size.** The dialog
 *   fills the available width minus a gutter, so at 200% scale it has room.
 * - **The deepest shadow in the system**, via
 *   [cl.rutbusiness.ui.theme.RbDimens.elevationRaised] on
 *   [cl.rutbusiness.ui.theme.RbShapes.dialog] - a bigger radius than
 *   [cl.rutbusiness.ui.theme.RbShapes.card] too, so the sheet reads as one
 *   more step off the mesa than the card that opened it, by shape and by
 *   depth together, not by the scrim alone.
 * - **The body scrolls.** This is the one place a font scale can genuinely run
 *   out of screen: a two-line question plus two 56dp buttons at 200% is taller
 *   than a 720p phone. Scrolling the body keeps the buttons reachable instead
 *   of pushing them off the bottom, which is the "botón inalcanzable" failure
 *   the acceptance criteria name.
 * - **Buttons stack vertically**, full width, each at least
 *   [cl.rutbusiness.ui.theme.RbDimens.touchTarget] (56dp) via [RbButton]. A
 *   side-by-side pair at 200% gives each label about eight characters before it
 *   wraps into a mess, and stacked full-width targets are easier to hit anyway.
 *
 * Hierarchy: title (heading) → message (body secondary) → cancel → confirm.
 * Air between the copy block and the actions is [cl.rutbusiness.ui.theme.RbDimens.space4]
 * so the irreversible verb does not sit under the paragraph like a footnote.
 *
 * The destructive action is the *second* button, below cancel. Putting the
 * irreversible one under the thumb's resting position is how accidents happen.
 * When [destructive] is true the title is also [rbAssertive] so TalkBack leads
 * with the risk, not with the chrome.
 *
 * @param destructive paints the confirm button as [RbButtonVariant.Destructive].
 *   Use it whenever the action cannot be undone - anulando una venta, borrando
 *   un producto.
 */
@Composable
fun RbConfirmDialog(
    title: String,
    message: String,
    confirmLabel: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    cancelLabel: String = "Cancelar",
    destructive: Boolean = false,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.dialog

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                // Static scrim — no blur, no fade. Reduced motion and a 60 Hz
                // panel both get the same solid separation.
                .background(colors.scrim)
                .padding(dimens.space3),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                modifier = modifier
                    .fillMaxWidth()
                    .rbElevated(dimens.elevationRaised, shape)
                    .clip(shape)
                    .background(colors.surfaceRaised)
                    // outlineStrong so the card edge survives feria sun the same
                    // way RbCard does — a soft outline melts into the scrim.
                    .border(dimens.border, colors.outlineStrong, shape)
                    .verticalScroll(rememberScrollState())
                    .padding(dimens.space4),
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                Text(
                    text = title,
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                    modifier = Modifier
                        .rbHeading()
                        .then(if (destructive) Modifier.rbAssertive() else Modifier),
                )
                Text(
                    text = message,
                    style = RbTheme.typography.body,
                    color = colors.textSecondary,
                )

                // Cancel first, confirm under it. space2 keeps the pair as one
                // action block; the space3 from the parent already separates
                // them from the copy above without another nested spacer.
                RbButton(
                    label = cancelLabel,
                    onClick = onDismiss,
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                )
                RbButton(
                    label = confirmLabel,
                    onClick = onConfirm,
                    variant = if (destructive) {
                        RbButtonVariant.Destructive
                    } else {
                        RbButtonVariant.Primary
                    },
                    fillWidth = true,
                )
            }
        }
    }
}
