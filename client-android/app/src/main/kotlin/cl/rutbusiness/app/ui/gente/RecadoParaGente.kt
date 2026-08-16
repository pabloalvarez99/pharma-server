package cl.rutbusiness.app.ui.gente

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
 * No es un export: se ve la frase tal cual saldrá (como una nota de papel),
 * con aire para leer al sol y al 200%, una pista humana y un botón que habla
 * de mandar o recordar — no de "compartir el resumen". Si no hay puerto de
 * plataforma, no se dibuja nada (mismo contrato que [LocalCompartirConGente]).
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
        // Aire entre la nota y el botón: se lee como dos gestos, no como un sheet.
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        // La nota: borde de papel, relleno holgado, tipografía de cuerpo.
        // Tokens del tema (contraste AAA / outline >= 3.0); no se inventan colores.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(shape)
                .background(colors.surfaceRaised)
                .border(dimens.border, colors.outlineStrong, shape)
                .padding(dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            Text(
                text = pista,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
            Text(
                text = mensaje,
                style = RbTheme.typography.bodyStrong,
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
