package cl.rutbusiness.app.ui.nav

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * Los lugares a los que se puede ir estando adentro.
 *
 * Tres y no más. Cada destino que se agrega angosta a todos los demás, y el
 * peor caso de esta barra —tres etiquetas completas, con la letra al 200%, en
 * un panel de 360dp— ya es el límite de lo que entra sin cortar una palabra.
 *
 * El catálogo **no** es un destino: buscar un producto vive dentro de Cobrar,
 * que es donde la cajera lo necesita. Una pestaña de catálogo suelta sería una
 * segunda lista de productos que hace lo mismo con un toque más.
 */
enum class Destino(val etiqueta: String) {
    /**
     * El default. Directiva del founder: la dueña no navega menús, le habla al
     * negocio. Todo lo demás existe para cuando hablar no alcanza.
     */
    Agente("Agente"),

    /** La pantalla que la cajera usa doscientas veces al día. */
    Cobrar("Cobrar"),

    /** "¿Cuánto vendí hoy?", la pregunta de todos los días. */
    Resumen("Resumen"),
}

/**
 * La barra de abajo.
 *
 * Reglas que no son estéticas sino de piso de hardware, y por qué:
 *
 * - **El ícono nunca va solo.** Un pictograma sin palabra es una adivinanza, y
 *   el usuario objetivo es una persona mayor que no creció con esta gramática.
 *   La etiqueta está siempre, en las tres pestañas, elegida o no.
 * - **Nada distingue sólo por color.** La pestaña activa se marca por tres vías
 *   a la vez: fondo detrás del ícono, color de marca y negrita. Quien no
 *   distingue el verde igual ve cuál está elegida.
 * - **La barra no tiene alto fijo.** Al 200% la etiqueta ocupa dos líneas y la
 *   barra crece; 56dp es el piso, no la medida.
 * - Los íconos se dibujan a mano y se miden contra la tipografía, así que
 *   **crecen con la escala de letra** como el resto. Un vector en `dp` se
 *   quedaría chico justo para quien subió la letra porque ve poco.
 *
 * Está en `app/` y no en `:design` a propósito: la barra conoce los tres
 * destinos de **este** producto, y el design system no debería.
 */
@Composable
fun BarraDeNavegacion(
    actual: Destino,
    onElegir: (Destino) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.surface)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            ),
    ) {
        // La misma hairline que cierra el `RbTopBar`, arriba en vez de abajo:
        // sin ella la barra se funde con el contenido en el tema claro.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(colors.outline),
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.Top,
        ) {
            Destino.entries.forEach { destino ->
                Pestana(
                    destino = destino,
                    elegida = destino == actual,
                    onClick = { onElegir(destino) },
                    // `weight` reparte el ancho en partes iguales: la pestaña
                    // elegida no puede ser más ancha, o la barra se movería
                    // debajo del dedo cada vez que se cambia de pantalla.
                    modifier = Modifier.weight(1f),
                )
            }
        }

        // Un respiro bajo las etiquetas para que la última línea no quede
        // pegada al borde inferior del aparato.
        Box(modifier = Modifier.height(dimens.space1))
    }
}

@Composable
private fun Pestana(
    destino: Destino,
    elegida: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    val color = if (elegida) colors.brandText else colors.textSecondary

    // El ícono se mide contra el cuerpo de texto, no en `dp` sueltos: así sube
    // con la escala de letra del sistema igual que la etiqueta que lo explica.
    val tamanoIcono: Dp = with(LocalDensity.current) {
        RbTheme.typography.body.fontSize.toDp()
    }

    Column(
        modifier = modifier
            // `selected` va antes del clickable para que TalkBack anuncie
            // "pestaña, seleccionada" y no sólo "botón".
            .semantics { selected = elegida }
            .rbClickable(
                onClick = onClick,
                role = Role.Tab,
                shape = RbTheme.shapes.field,
            )
            .rbTouchTarget()
            .padding(horizontal = dimens.space1, vertical = dimens.space1),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(dimens.space1, Alignment.CenterVertically),
    ) {
        Box(
            modifier = Modifier
                .clip(RbTheme.shapes.pill)
                // El fondo cambia; el padding no. Si sólo la elegida tuviera
                // relleno, la fila entera saltaría al cambiar de pestaña.
                .background(if (elegida) colors.brandContainer else Color.Transparent)
                .padding(horizontal = dimens.space2, vertical = dimens.space1),
        ) {
            IconoDestino(destino = destino, color = color, tamano = tamanoIcono)
        }

        Text(
            text = destino.etiqueta,
            style = RbTheme.typography.label,
            color = color,
            textAlign = TextAlign.Center,
            // Sin `maxLines` a propósito: al 200% "Resumen" no entra en un
            // tercio de 360dp y tiene que poder envolver. Cortar la palabra
            // sería dejar la pestaña sin nombre justo para quien más lo lee.
            fontWeight = if (elegida) FontWeight.Bold else FontWeight.Medium,
        )
    }
}

/**
 * Los tres pictogramas, dibujados y no importados.
 *
 * Sin dependencia de íconos: `material-icons-extended` pesa megas en un aparato
 * que no los tiene, y traer tres glifos no justifica la librería entera.
 * Formas gruesas y simples, que es lo único que se lee en un panel barato a la
 * luz del día — regla del piso de hardware contra los trazos finos.
 *
 * Decorativos para el lector de pantalla: el nombre ya lo dice la etiqueta de
 * abajo, y anunciarlo dos veces es ruido.
 */
@Composable
private fun IconoDestino(destino: Destino, color: Color, tamano: Dp) {
    Canvas(
        modifier = Modifier
            .size(tamano)
            .clearAndSetSemantics { },
    ) {
        when (destino) {
            Destino.Agente -> dibujarGlobo(color)
            Destino.Cobrar -> dibujarBillete(color)
            Destino.Resumen -> dibujarBarras(color)
        }
    }
}

/** Grosor de trazo proporcional al ícono, con un piso para que no desaparezca. */
private fun DrawScope.trazo(): Stroke =
    Stroke(width = (size.minDimension / 9f).coerceAtLeast(2.dp.toPx()))

/** Hablarle al negocio: un globo de diálogo. */
private fun DrawScope.dibujarGlobo(color: Color) {
    val alto = size.height * 0.72f
    drawRoundRect(
        color = color,
        topLeft = Offset.Zero,
        size = Size(size.width, alto),
        cornerRadius = CornerRadius(alto * 0.30f),
        style = trazo(),
    )
    // La cola, llena: un triángulo con contorno a este tamaño se ve sucio.
    drawPath(
        path = Path().apply {
            moveTo(size.width * 0.26f, alto * 0.92f)
            lineTo(size.width * 0.26f, size.height)
            lineTo(size.width * 0.54f, alto * 0.92f)
            close()
        },
        color = color,
    )
}

/** Cobrar: un billete. */
private fun DrawScope.dibujarBillete(color: Color) {
    val alto = size.height * 0.64f
    val arriba = (size.height - alto) / 2f
    drawRoundRect(
        color = color,
        topLeft = Offset(0f, arriba),
        size = Size(size.width, alto),
        cornerRadius = CornerRadius(alto * 0.24f),
        style = trazo(),
    )
    drawCircle(
        color = color,
        radius = alto * 0.20f,
        center = Offset(size.width / 2f, arriba + alto / 2f),
        style = trazo(),
    )
}

/** El resumen del día: tres barras que suben. */
private fun DrawScope.dibujarBarras(color: Color) {
    val ancho = size.width / 5f
    listOf(0.45f, 0.72f, 1.0f).forEachIndexed { indice, fraccion ->
        val alto = size.height * fraccion * 0.92f
        drawRoundRect(
            color = color,
            topLeft = Offset(ancho * (0.5f + indice * 1.6f), size.height - alto),
            size = Size(ancho, alto),
            cornerRadius = CornerRadius(ancho * 0.35f),
        )
    }
}
