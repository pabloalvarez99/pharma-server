package cl.rutbusiness.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Un punto a dibujar en [RbChart] — sin plata, sin fecha, sin nada de dominio.
 *
 * [valor] es una magnitud sin unidad, ya resuelta por quien llama: este
 * componente no sabe si viene de un monto, un conteo o cualquier otra cosa, y
 * por eso vive acá y no en `:app` — es un primitivo de dibujo reusable, igual
 * que [RbCard] o [RbChip]. Convertir un monto de servidor a este número (la
 * altura de una barra es geometría, no plata) es trabajo de quien llama; ver
 * `cl.rutbusiness.app.ui.resumen.alturaDeBarra`.
 *
 * @param destacado si esta barra es la que hay que resaltar (p.ej. "hoy" en
 *   una semana). Nunca se resalta sólo con color — ver [RbChart].
 */
data class RbChartDato(
    val etiqueta: String,
    val valor: Float,
    val destacado: Boolean = false,
)

/**
 * Una barra ya calculada en píxeles, lista para que [RbChart] la dibuje sin
 * volver a hacer una sola cuenta.
 */
data class RbChartBarra(
    val etiqueta: String,
    val destacado: Boolean,
    val x: Float,
    val anchoBarra: Float,
    val altoBarraPx: Float,
)

/**
 * La geometría de un gráfico de barras — pura, sin Compose ni Android.
 *
 * Acá viven los bordes que rompen un gráfico de barras sin que ningún test de
 * layout se entere:
 *
 * - **Normalización contra el máximo.** Cada barra es una fracción de
 *   [altoDisponiblePx] proporcional a `valor / máximo`, nunca un valor
 *   absoluto — así una semana de $2.000 y una de $200.000 usan el mismo alto
 *   disponible.
 * - **Un solo día con datos.** Si sólo un [RbChartDato.valor] es mayor que
 *   cero, ése es el máximo y su barra ocupa el 100%; el resto cae en
 *   [minAltoBarraPx], no en cero puro — así siguen siendo barras visibles y
 *   no líneas invisibles pegadas al piso.
 * - **Todos los datos en cero.** El máximo también es cero, y dividir por él
 *   daría `NaN`/`Infinity`. Acá el máximo se trata como "sin datos": todas
 *   las barras caen directo en [minAltoBarraPx], nunca se divide por cero.
 * - **Valores negativos.** Se recortan a cero antes de calcular nada — un
 *   gráfico no dibuja una barra hacia abajo por un dato que no debería
 *   existir.
 *
 * @param anchoTotalPx ancho disponible, en píxeles del `DrawScope` que va a
 *   dibujar el resultado.
 * @param altoDisponiblePx alto máximo que puede ocupar una barra llena.
 * @param espacioEntreBarrasPx separación horizontal entre barras.
 * @param minAltoBarraPx piso visual para una barra en cero, para que un día
 *   sin ventas siga leyéndose como una barra y no como un espacio vacío.
 */
fun calcularBarrasDeGrafico(
    datos: List<RbChartDato>,
    anchoTotalPx: Float,
    altoDisponiblePx: Float,
    espacioEntreBarrasPx: Float = 0f,
    minAltoBarraPx: Float = 0f,
): List<RbChartBarra> {
    if (datos.isEmpty() || anchoTotalPx <= 0f || altoDisponiblePx <= 0f) return emptyList()

    val cantidad = datos.size
    val espacioTotal = espacioEntreBarrasPx * (cantidad - 1).coerceAtLeast(0)
    val anchoBarra = ((anchoTotalPx - espacioTotal) / cantidad).coerceAtLeast(0f)
    val maximo = datos.maxOf { it.valor.coerceAtLeast(0f) }

    return datos.mapIndexed { indice, dato ->
        val valorPositivo = dato.valor.coerceAtLeast(0f)
        val fraccion = if (maximo <= 0f) 0f else (valorPositivo / maximo).coerceIn(0f, 1f)
        val altoLleno = (fraccion * altoDisponiblePx).coerceIn(0f, altoDisponiblePx)
        RbChartBarra(
            etiqueta = dato.etiqueta,
            destacado = dato.destacado,
            x = indice * (anchoBarra + espacioEntreBarrasPx),
            anchoBarra = anchoBarra,
            altoBarraPx = maxOf(altoLleno, minAltoBarraPx).coerceAtMost(altoDisponiblePx),
        )
    }
}

/**
 * Un gráfico de barras delgado — dibuja exactamente lo que
 * [calcularBarrasDeGrafico] calculó, nada más.
 *
 * La barra [RbChartDato.destacado] nunca se distingue sólo por color (piso de
 * accesibilidad): lleva **tres** señales — el color de marca, una etiqueta
 * distinta ("hoy" en vez del día de la semana, en negrita) y un punto encima
 * de la barra. Quien no distingue colores igual la reconoce por la etiqueta y
 * el punto.
 *
 * Sin `contentDescription` un gráfico es una mancha para un lector de
 * pantalla: [descripcion] reemplaza **todo** el bloque (barras y etiquetas)
 * por una sola frase que resume el gráfico, así que quien llama tiene que
 * decir en palabras lo que el dibujo muestra.
 *
 * @param alturaBarras alto fijo del área de dibujo. No es texto — es un
 *   gráfico, del mismo tamaño físico que [cl.rutbusiness.ui.theme.RbDimens.markSize] —
 *   así que un `dp` fijo acá no viola el piso de "nada de alturas fijas donde
 *   vive texto": el texto de las etiquetas vive abajo, en su propia fila, y
 *   crece con la escala de letra sin que este alto lo apriete.
 */
@Composable
fun RbChart(
    datos: List<RbChartDato>,
    descripcion: String,
    modifier: Modifier = Modifier,
    alturaBarras: Dp = 120.dp,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val density = LocalDensity.current

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clearAndSetSemantics { contentDescription = descripcion },
        verticalArrangement = Arrangement.spacedBy(dimens.space1),
    ) {
        val espacioPx = with(density) { dimens.space1.toPx() }
        val minAltoPx = with(density) { dimens.space1.toPx() }
        val marcadorRadioPx = with(density) { (dimens.space1 / 2).toPx() }
        val marcadorGapPx = with(density) { dimens.space2.toPx() }

        Canvas(
            modifier = Modifier
                .fillMaxWidth()
                .height(alturaBarras),
        ) {
            val barras = calcularBarrasDeGrafico(
                datos = datos,
                anchoTotalPx = size.width,
                altoDisponiblePx = size.height - marcadorGapPx,
                espacioEntreBarrasPx = espacioPx,
                minAltoBarraPx = minAltoPx,
            )
            barras.forEach { barra ->
                drawRect(
                    color = if (barra.destacado) colors.brandFill else colors.outlineStrong,
                    topLeft = Offset(barra.x, size.height - barra.altoBarraPx),
                    size = Size(barra.anchoBarra, barra.altoBarraPx),
                )
                if (barra.destacado) {
                    // Segunda señal, sin color: un punto sobre la barra de hoy.
                    // La tercera es la etiqueta en negrita que dibuja la Row de
                    // abajo — ver la etiqueta "Hoy" que arma quien llama.
                    drawCircle(
                        color = colors.brandFill,
                        radius = marcadorRadioPx,
                        center = Offset(
                            barra.x + barra.anchoBarra / 2f,
                            size.height - barra.altoBarraPx - marcadorGapPx / 2f,
                        ),
                    )
                }
            }
        }

        Row(modifier = Modifier.fillMaxWidth()) {
            datos.forEach { dato ->
                Text(
                    text = dato.etiqueta,
                    style = RbTheme.typography.support.let {
                        if (dato.destacado) it.copy(fontWeight = FontWeight.Bold) else it
                    },
                    color = if (dato.destacado) colors.textPrimary else colors.textSecondary,
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                    modifier = Modifier.weight(1f),
                )
            }
        }
    }
}
