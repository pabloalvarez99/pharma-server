package cl.rutbusiness.app.ui.gente

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Un recado listo para mandar por chat.
 *
 * No es un export: se ve la frase tal cual saldrá (como una nota), con una
 * pista humana y un botón que habla de contar o recordar — no de "compartir
 * el resumen". Si no hay puerto de plataforma, no se dibuja nada (mismo
 * contrato que [LocalCompartirConGente]).
 *
 * @param mensaje texto ya armado con [mensajeHoy] o [mensajeDeuda].
 * @param etiqueta verbo del botón; ver [etiquetaCompartirDia] /
 *   [etiquetaCompartirDeuda].
 */
@Composable
fun RecadoParaGente(
    mensaje: String,
    etiqueta: String,
    modifier: Modifier = Modifier,
    pista: String = pistaDelRecado(),
) {
    val compartir = compartirConGente() ?: return
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.field

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        // La nota: superficie callada, aire, la frase completa. Se lee como
        // lo que le mandarías a alguien, no como una fila de un reporte.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(shape)
                .background(colors.surfaceVariant)
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space1),
        ) {
            Text(
                text = pista,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
            Text(
                text = mensaje,
                style = RbTheme.typography.body,
                color = colors.textPrimary,
            )
        }

        RbButton(
            label = etiqueta,
            onClick = { compartir(mensaje) },
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )
    }
}
