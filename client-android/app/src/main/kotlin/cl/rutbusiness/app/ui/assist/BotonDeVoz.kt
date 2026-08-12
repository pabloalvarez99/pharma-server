package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/** Lo que dice el botón cuando está quieto. */
internal const val ETIQUETA_QUIETO = "Hablar"

/** Y mientras escucha, para que se vea que el micrófono está abierto. */
internal const val ETIQUETA_ESCUCHANDO = "Te escucho"

/**
 * El control de voz del agente.
 *
 * **Dibujo y palabra, no sólo el dibujo.** Un micrófono suelto es un ícono que
 * hay que interpretar; con la palabra al lado no hay nada que adivinar, y es lo
 * que ya prometen el primer uso ("anotás una venta con la voz o el teclado") y
 * el tagline del rubro ("Puesto, voz, fiado"). El ícono existe igual porque en
 * un puesto se mira de reojo y una forma se reconoce antes que una palabra.
 *
 * Se construye a mano y no con [cl.rutbusiness.ui.components.RbButton] porque
 * ese componente no tiene ranura para un ícono, pero usa exactamente sus mismas
 * piezas del design system: `brandFill`/`onBrandFill` para el contraste ya
 * medido, `rbClickable` para el anillo de foco y el rol de botón, y
 * `rbTouchTarget` -el piso de 56 dp- **sobre el nodo clickeable**, que es la
 * única forma de que el dedo reciba lo que la captura de pantalla muestra.
 *
 * El estado se dice de dos maneras a la vez, porque una sola falla: la palabra
 * cambia ("Hablar" -> "Te escucho") y el micrófono pasa de contorno a relleno.
 * Quien no lee rápido ve la forma; quien no distingue la forma lee la palabra.
 */
@Composable
internal fun BotonDeVoz(
    escuchando: Boolean,
    habilitado: Boolean,
    onTocar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.field

    val fondo = if (habilitado) colors.brandFill else colors.surfaceVariant
    val tinta = if (habilitado) colors.onBrandFill else colors.textTertiary

    Row(
        modifier = modifier
            .clip(shape)
            .background(fondo)
            .rbClickable(
                onClick = onTocar,
                enabled = habilitado,
                role = Role.Button,
                shape = shape,
            )
            .rbTouchTarget()
            .padding(horizontal = dimens.space3, vertical = dimens.space2),
        horizontalArrangement = Arrangement.spacedBy(dimens.space1),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Decorativo para el lector de pantalla: la etiqueta de al lado dice lo
        // mismo, y anunciarlo dos veces es ruido.
        Canvas(
            modifier = Modifier
                .size(dimens.iconSize)
                .clearAndSetSemantics { },
        ) {
            dibujarMicrofono(color = tinta, lleno = escuchando)
        }

        Text(
            text = if (escuchando) ETIQUETA_ESCUCHANDO else ETIQUETA_QUIETO,
            style = RbTheme.typography.button,
            color = tinta,
            // Sin `maxLines`: al 200% la palabra se envuelve, no se corta. Un
            // verbo cortado es un botón sin significado.
        )
    }
}

/**
 * Un micrófono: cápsula, arco, pie.
 *
 * El grosor del trazo sale del tamaño y nunca baja de 2 dp -la misma regla que
 * los íconos de la barra de navegación-: un trazo de un pixel desaparece en un
 * panel barato bajo el sol, que es la condición de trabajo del puesto.
 */
private fun DrawScope.dibujarMicrofono(color: Color, lleno: Boolean) {
    val trazo = Stroke(width = (size.minDimension / 9f).coerceAtLeast(2.dp.toPx()))

    val anchoCapsula = size.width * 0.40f
    val altoCapsula = size.height * 0.52f
    val izquierda = (size.width - anchoCapsula) / 2f

    drawRoundRect(
        color = color,
        topLeft = Offset(izquierda, 0f),
        size = Size(anchoCapsula, altoCapsula),
        cornerRadius = CornerRadius(anchoCapsula / 2f),
        // Relleno mientras escucha: la señal de "está abierto" que se ve sin
        // leer nada.
        style = if (lleno) Fill else trazo,
    )

    // El arco que abraza la cápsula.
    val radioArco = size.width * 0.34f
    val centroArco = size.height * 0.58f
    drawArc(
        color = color,
        startAngle = 0f,
        sweepAngle = 180f,
        useCenter = false,
        topLeft = Offset(size.width / 2f - radioArco, centroArco - radioArco),
        size = Size(radioArco * 2f, radioArco * 2f),
        style = trazo,
    )

    // El palito hasta el pie, y el pie.
    val abajoDelArco = centroArco + radioArco
    drawLine(
        color = color,
        start = Offset(size.width / 2f, abajoDelArco),
        end = Offset(size.width / 2f, size.height),
        strokeWidth = trazo.width,
    )
    drawLine(
        color = color,
        start = Offset(size.width * 0.28f, size.height),
        end = Offset(size.width * 0.72f, size.height),
        strokeWidth = trazo.width,
    )
}
