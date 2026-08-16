package cl.rutbusiness.app.ui.scanner

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme

/**
 * El bloque de abajo del escáner.
 *
 * Va **debajo** del visor, nunca encima. Flotar la respuesta sobre el video
 * obliga a componer texto contra una imagen que se mueve -ilegible justo cuando
 * hay que leerla rápido- y en varios teléfonos fuerza a `PreviewView` a su modo
 * `TextureView`, que copia cada frame otra vez en un aparato que ya va justo.
 *
 * Los botones quedan siempre pegados al mismo borde: cuando hay más que decir,
 * lo que se achica es el visor, no la fila que el dedo va a tocar.
 *
 * Se siente el **mango de la linterna**: superficie elevada, borde fuerte al
 * sol, aire generoso. El color de estado va adentro, en el [Cartel], y no en
 * este fondo: los botones tienen que seguir parados sobre la superficie para
 * la que el design system midió su contraste.
 */
@Composable
internal fun PanelInferior(
    modifier: Modifier = Modifier,
    contenido: @Composable ColumnScope.() -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.surfaceRaised)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            )
            .imePadding(),
    ) {
        // Hairline: el visor se corta acá; sin ella, al 200% el texto se lee
        // como una frase que "termina mal" en vez de panel que sigue.
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(dimens.border)
                .background(colors.outlineStrong),
        )
        Column(
            modifier = Modifier.padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
            content = contenido,
        )
    }
}

/**
 * La respuesta a la última lectura, en un bloque de color.
 *
 * El color es la mitad del trabajo: la cajera está mirando el producto, no la
 * pantalla, y tiene que poder confirmar de reojo. Verde de marca = entró; rojo =
 * ese código no está. Las dos combinaciones que usa -`textPrimary` sobre
 * `brandContainer` y sobre `dangerContainer`- son las que `RbContrastTest` mide
 * contra WCAG AAA en los dos temas.
 *
 * Borde fuerte + padding de mesa: se lee un recado del puesto, no un chip de
 * laboratorio. La otra mitad la hace la vibración, porque el color no sirve si
 * no se está mirando y hay quien no lo distingue.
 */
@Composable
internal fun Cartel(
    fondo: Color,
    titulo: String,
    detalle: String? = null,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(fondo)
            .border(dimens.border, colors.outlineStrong, shape)
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space1),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.bodyStrong,
            color = colors.textPrimary,
        )
        detalle?.let {
            Text(text = it, style = RbTheme.typography.support, color = colors.textPrimary)
        }
    }
}

/**
 * Marco quieto del visor: cuatro esquinas, como el cono de una linterna.
 *
 * **Sólo pintura.** No toca la cámara ni el analizador; vive encima del video
 * para que el ojo sepa dónde apoyar el código sin leer un manual.
 */
@Composable
internal fun MarcoDeLinterna(modifier: Modifier = Modifier) {
    val colors = RbTheme.colors
    // Esquinas claras sobre el video oscuro; el cono se lee al sol del local.
    val tinta = Color.White.copy(alpha = 0.92f)
    val sombra = Color.Black.copy(alpha = 0.32f)
    Canvas(
        modifier = modifier
            .fillMaxSize()
            .clearAndSetSemantics { },
    ) {
        val margen = 28.dp.toPx()
        val largo = 28.dp.toPx()
        val grosor = 3.dp.toPx()
        val anchoUtil = size.width - margen * 2
        val altoUtil = size.height - margen * 2

        // Velo suave fuera del "cono": el centro queda limpio para el código.
        drawRect(color = sombra, size = Size(size.width, margen))
        drawRect(
            color = sombra,
            topLeft = Offset(0f, size.height - margen),
            size = Size(size.width, margen),
        )
        drawRect(
            color = sombra,
            topLeft = Offset(0f, margen),
            size = Size(margen, altoUtil),
        )
        drawRect(
            color = sombra,
            topLeft = Offset(size.width - margen, margen),
            size = Size(margen, altoUtil),
        )

        fun esquina(x: Float, y: Float, dx: Float, dy: Float) {
            drawLine(
                color = tinta,
                start = Offset(x, y),
                end = Offset(x + dx, y),
                strokeWidth = grosor,
                cap = StrokeCap.Round,
            )
            drawLine(
                color = tinta,
                start = Offset(x, y),
                end = Offset(x, y + dy),
                strokeWidth = grosor,
                cap = StrokeCap.Round,
            )
        }

        val izq = margen
        val der = margen + anchoUtil
        val arr = margen
        val aba = margen + altoUtil
        esquina(izq, arr, largo, largo)
        esquina(der, arr, -largo, largo)
        esquina(izq, aba, largo, -largo)
        esquina(der, aba, -largo, -largo)

        // Aro suave: el cono de la linterna, no un rectángulo de laboratorio.
        drawRoundRect(
            color = tinta.copy(alpha = 0.35f),
            topLeft = Offset(izq, arr),
            size = Size(anchoUtil, altoUtil),
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(16.dp.toPx(), 16.dp.toPx()),
            style = Stroke(width = 1.5.dp.toPx()),
        )
    }
}

/**
 * Lo que se explica antes -y en vez- del diálogo del sistema.
 *
 * Tres reglas que esta pantalla existe para cumplir:
 *
 * 1. **Se explica antes de que Android pregunte.** El cuadro del sistema dice
 *    "¿Permitir que RutAgent tome fotos y grabe videos?", que en un mostrador
 *    suena a que la app va a guardar fotos del local. Acá se dice para qué es y
 *    qué **no** hacemos con la imagen, antes de que decidir sea urgente.
 * 2. **Nunca es un callejón sin salida.** Escribir el código a mano está
 *    siempre, con el mismo tamaño de toque que el botón de permitir.
 * 3. **Si Android ya no va a preguntar**, se explica el camino de Ajustes en
 *    palabras y en orden, no "habilite el permiso en la configuración".
 *
 * Se siente una **linterna del puesto**, no un lab: se prende un rato, mira el
 * código y se apaga. Cero jerga de cámara.
 */
@Composable
internal fun ExplicacionDelPermiso(estado: EstadoDelPermiso, modifier: Modifier = Modifier) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    val titulo: String
    val cuerpo: String
    when (estado) {
        EstadoDelPermiso.NegadoParaSiempre -> {
            titulo = "La cámara quedó bloqueada"
            cuerpo = "Android no nos va a volver a preguntar. Para desbloquearla: toca " +
                "\"Abrir ajustes\", entra a Permisos, prende Cámara y vuelve acá con el botón de " +
                "atrás del teléfono. Si prefieres, sigue cobrando y escribe el código a mano."
        }

        EstadoDelPermiso.Negado -> {
            titulo = "Sin la cámara no leemos el código"
            cuerpo = "La usamos sólo un ratito, como una linterna: apunta al código de barras " +
                "del producto y lo suma a la venta. Puedes darle permiso ahora, o seguir " +
                "escribiendo el código a mano."
        }

        else -> {
            titulo = "La cámara es la linterna"
            cuerpo = "Sólo para leer el código de barras del producto y sumarlo a la venta. La " +
                "imagen se mira en este mismo teléfono: no se guarda ninguna foto y no sale de acá."
        }
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = dimens.space3, vertical = dimens.space4),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(shape)
                .background(colors.surfaceRaised)
                .border(dimens.focusRing, colors.outlineStrong, shape)
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            Text(text = titulo, style = RbTheme.typography.heading, color = colors.textPrimary)
            Text(text = cuerpo, style = RbTheme.typography.body, color = colors.textPrimary)
            Text(
                text = "Cuando termines, la apagas y listo. No es un laboratorio: es una linterna.",
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
}

/**
 * Los botones del permiso.
 *
 * `RbReflowRow` los deja lado a lado mientras entren y apila el segundo cuando
 * la letra al 200% partiría una palabra. Ninguno de los dos deja de estar.
 * Cada uno ya mide ≥ 56 dp por el design system.
 */
@Composable
internal fun AccionesDePermiso(
    estado: EstadoDelPermiso,
    onPermitir: () -> Unit,
    onAjustes: () -> Unit,
    onEscribirAMano: () -> Unit,
) {
    RbReflowRow(
        spacing = RbTheme.dimens.space2,
        content = {
            if (estado == EstadoDelPermiso.NegadoParaSiempre) {
                RbButton(label = "Abrir ajustes", onClick = onAjustes)
            } else {
                RbButton(label = "Permitir cámara", onClick = onPermitir)
            }
        },
        trailing = {
            RbButton(
                label = "Escribir a mano",
                onClick = onEscribirAMano,
                variant = RbButtonVariant.Secondary,
            )
        },
    )
}

/** Cartel de "acá no hay cámara": una tablet, un emulador, un lente roto. */
@Composable
internal fun SinCamara(onEscribirAMano: () -> Unit, modifier: Modifier = Modifier) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = dimens.space3, vertical = dimens.space4),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(shape)
                .background(colors.surfaceRaised)
                .border(dimens.focusRing, colors.outlineStrong, shape)
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            Text(
                text = "Este teléfono no tiene cámara que podamos usar",
                style = RbTheme.typography.heading,
                color = colors.textPrimary,
            )
            Text(
                text = "Se cobra igual: escribe el código de barras o busca el producto por nombre. " +
                    "La linterna no hace falta para anotar la venta.",
                style = RbTheme.typography.body,
                color = colors.textPrimary,
            )
        }
        RbButton(label = "Escribir a mano", onClick = onEscribirAMano, fillWidth = true)
    }
}
