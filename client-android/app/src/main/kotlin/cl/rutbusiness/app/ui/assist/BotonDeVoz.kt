package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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

/**
 * El control de voz del agente.
 *
 * Las palabras del botón ("Hablar" / "Te escucho") están en `CopyVoz.kt`, no
 * acá: mismo patrón que el resto del copy de feria, puro JVM y con test propio.
 *
 * **No es un micrófono chico en la esquina.** En el puesto se toca con sol,
 * a veces con una mano ocupada y sin mirar el ícono: el control tiene que
 * verse como botón de verdad (marca, 56 dp de piso, palabra legible) y no
 * como un glyph de toolbar de 24 dp. El micrófono va grande al lado de la
 * palabra porque la forma se reconoce de reojo y la etiqueta saca la duda.
 *
 * Se construye a mano y no con [cl.rutbusiness.ui.components.RbButton] porque
 * ese componente no tiene ranura para un ícono, pero usa las mismas piezas
 * del design system: `brandFill`/`onBrandFill`, `rbClickable` y
 * `rbTouchTarget` sobre el nodo clickeable.
 *
 * El estado se dice de dos maneras a la vez: la palabra cambia
 * ("Hablar" → "Te escucho") y el micrófono pasa de contorno a relleno. Quien
 * no lee rápido ve la forma; quien no distingue la forma lee la palabra.
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
    // Píldora gruesa: se lee como control del puesto, no como ícono suelto.
    val shape = RbTheme.shapes.pill

    val fondo = when {
        !habilitado -> colors.surfaceVariant
        escuchando -> colors.brandFill
        else -> colors.brandFill
    }
    val tinta = if (habilitado) colors.onBrandFill else colors.textTertiary
    // Borde que sobrevive al sol: sin él, brandFill se funde con el cielo o
    // con un fondo claro barato. Al escuchar el borde se engrosa (focusRing)
    // para que "está abierto" se vea sin leer la etiqueta.
    val trazoBorde = if (escuchando && habilitado) dimens.focusRing else dimens.border
    val colorBorde = when {
        !habilitado -> colors.outline
        escuchando -> colors.onBrandFill
        else -> colors.onBrandFill.copy(alpha = 0.35f)
    }

    // El micrófono va más grande que `iconSize` (24): un trazo de toolbar se
    // pierde al sol y se siente "app de retail". 32 dp se ve al costado del
    // campo sin pelear con la etiqueta a 200%.
    val tamanoMicro = 32.dp

    Row(
        modifier = modifier
            .clip(shape)
            .background(fondo)
            .border(trazoBorde, colorBorde, shape)
            .rbClickable(
                onClick = onTocar,
                enabled = habilitado,
                role = Role.Button,
                shape = shape,
            )
            // 56 dp de piso en ambos ejes; el ancho mínimo evita un botón
            // cuadradito al lado del campo cuando la etiqueta es corta.
            .rbTouchTarget(minWidth = 120.dp)
            .padding(horizontal = dimens.space3, vertical = dimens.space2),
        horizontalArrangement = Arrangement.spacedBy(dimens.space2),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Decorativo para el lector de pantalla: la etiqueta de al lado dice
        // lo mismo, y anunciarlo dos veces es ruido.
        Canvas(
            modifier = Modifier
                .size(tamanoMicro)
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
 * El grosor del trazo sale del tamaño y nunca baja de 2.5 dp -más grueso que
 * un ícono de barra-: un trazo de un pixel desaparece en un panel barato bajo
 * el sol, que es la condición de trabajo del puesto.
 */
private fun DrawScope.dibujarMicrofono(color: Color, lleno: Boolean) {
    val trazo = Stroke(width = (size.minDimension / 8f).coerceAtLeast(2.5.dp.toPx()))

    val anchoCapsula = size.width * 0.42f
    val altoCapsula = size.height * 0.54f
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
    val radioArco = size.width * 0.36f
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
        end = Offset(size.width / 2f, size.height * 0.96f),
        strokeWidth = trazo.width,
    )
    drawLine(
        color = color,
        start = Offset(size.width * 0.26f, size.height * 0.96f),
        end = Offset(size.width * 0.74f, size.height * 0.96f),
        strokeWidth = trazo.width,
    )
}
