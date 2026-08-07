package cl.rutbusiness.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.VisualTransformation
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * A labelled text field.
 *
 * Ported from `.rb-input` + `.rb-label` in `brand.css`, with three fixes:
 *
 * - **The border is a real border.** The CSS used `--rb-line` (#232c40 on
 *   #121826 = 1.27, and #e6e0d4 on #fff = 1.31). Under WCAG 1.4.11 the edge of
 *   an input has to clear 3.0, and at 1.3 the field is invisible on a
 *   sun-washed screen. Uses [cl.rutbusiness.ui.theme.RbColors.outlineStrong].
 * - **The label is not 12px muted.** `.rb-label` was `font-size:12px` in
 *   `--rb-txt-2`. The label ramp here is 15sp at AAA, and it scales.
 * - **The label sits above the field, never inside it.** A floating label that
 *   shrinks into the border has nowhere to go at 200% font scale.
 *
 * The field holds a single line - the values it carries are a RUT, an amount, a
 * folio - but its **height is a minimum**, so scaled text grows the box instead
 * of clipping.
 */
@Composable
fun RbTextField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    /** Human help under the field. Shown when [errorMessage] is null. */
    supportingText: String? = null,
    /**
     * What went wrong, in plain Chilean Spanish. Non-null turns the field into
     * its error state and is announced by TalkBack through
     * [androidx.compose.ui.semantics.error].
     */
    errorMessage: String? = null,
    enabled: Boolean = true,
    /** Monospace for RUT / CLP / folios, so digits line up column to column. */
    numeric: Boolean = false,
    keyboardType: KeyboardType = KeyboardType.Text,
    /**
     * Qué dice la tecla de acción del teclado. `Send` cuando escribir y
     * apretar esa tecla es la forma natural de terminar - un campo de chat,
     * una búsqueda - para no obligar a bajar la vista hasta un botón.
     */
    imeAction: ImeAction = ImeAction.Default,
    /**
     * Qué hacer cuando se aprieta esa tecla. Nunca es la *única* forma de
     * enviar: en un teclado de teléfono viejo esa tecla puede venir sin
     * etiqueta clara, así que la pantalla siempre ofrece además un botón.
     */
    onImeAction: (() -> Unit)? = null,
    visualTransformation: VisualTransformation = VisualTransformation.None,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.field

    val interactionSource = remember { MutableInteractionSource() }
    val focused by interactionSource.collectIsFocusedAsState()
    val hasError = errorMessage != null

    val borderColor = when {
        !enabled -> colors.outline
        hasError -> colors.dangerText
        focused -> colors.brandText
        else -> colors.outlineStrong
    }
    // A focused or failing field gets a thicker edge, so the state is carried by
    // width as well as hue - colour alone excludes a colour-blind user.
    val borderWidth = if (focused || hasError) dimens.focusRing else dimens.border

    val textStyle = if (numeric) RbTheme.typography.numeric else RbTheme.typography.body

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(dimens.space1),
    ) {
        Text(
            text = label,
            style = RbTheme.typography.label,
            color = colors.textSecondary,
            // Read as part of the field below, so TalkBack does not say it twice.
            modifier = Modifier.clearAndSetSemantics { },
        )

        // The 56dp floor and the chrome both sit on the BasicTextField itself,
        // never on a wrapper. A padded Box around a 21dp input measures 56dp in
        // a screenshot and still gives the finger a 21dp target - which is
        // exactly what the first version of this component did, and what
        // `RbFontScaleTest` caught.
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            enabled = enabled,
            singleLine = true,
            textStyle = textStyle.copy(
                color = if (enabled) colors.textPrimary else colors.textTertiary,
            ),
            cursorBrush = SolidColor(colors.brandText),
            interactionSource = interactionSource,
            visualTransformation = visualTransformation,
            keyboardOptions = KeyboardOptions(
                keyboardType = keyboardType,
                imeAction = imeAction,
            ),
            keyboardActions = onImeAction?.let { accion ->
                // Las cuatro, no solo `onSend`: qué callback dispara el teclado
                // depende del `imeAction`, y un teclado de fabricante en un
                // aparato viejo no siempre respeta el que se pidió.
                KeyboardActions(
                    onSend = { accion() },
                    onDone = { accion() },
                    onGo = { accion() },
                    onSearch = { accion() },
                )
            } ?: KeyboardActions.Default,
            modifier = Modifier
                .fillMaxWidth()
                .clip(shape)
                .background(if (enabled) colors.surface else colors.surfaceVariant)
                .border(borderWidth, borderColor, shape)
                .rbTouchTarget()
                .semantics {
                    contentDescription = label
                    if (errorMessage != null) error(errorMessage)
                },
            decorationBox = { innerTextField ->
                Box(
                    // `fillMaxWidth`, no `fillMaxSize`. Con `fillMaxSize` el
                    // campo se queda con TODA la altura que le ofrezcan: dentro
                    // de un `LazyColumn` no se nota, porque ahí la altura viene
                    // ajustada al contenido, pero puesto en un `Column` junto a
                    // un hermano con `weight(1f)` el campo se comía la pantalla
                    // entera. El piso de 56dp ya lo pone `rbTouchTarget` más
                    // arriba; acá la altura tiene que salir del texto.
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = dimens.space3, vertical = dimens.space2),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    if (value.isEmpty() && placeholder != null) {
                        Text(
                            text = placeholder,
                            style = textStyle,
                            // Tertiary is AA here. The CSS used `--rb-txt-3`,
                            // which measured 2.72 on white - a placeholder
                            // nobody could read.
                            color = colors.textTertiary,
                        )
                    }
                    innerTextField()
                }
            },
        )

        val helper = errorMessage ?: supportingText
        if (helper != null) {
            Text(
                text = helper,
                style = RbTheme.typography.support,
                color = if (hasError) colors.dangerText else colors.textSecondary,
            )
        }
    }
}
