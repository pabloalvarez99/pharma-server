package cl.rutbusiness.app.ui.scanner

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.graphics.Color
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
 * El color de estado va adentro, en el [Cartel], y no en este fondo: los
 * botones tienen que seguir parados sobre la superficie para la que el design
 * system midió su contraste.
 */
@Composable
internal fun PanelInferior(
    modifier: Modifier = Modifier,
    contenido: @Composable ColumnScope.() -> Unit,
) {
    val dimens = RbTheme.dimens
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(RbTheme.colors.surfaceRaised)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            )
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
        content = contenido,
    )
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
 * La otra mitad la hace la vibración, porque el color no sirve si no se está
 * mirando y hay quien no lo distingue.
 */
@Composable
internal fun Cartel(
    fondo: Color,
    titulo: String,
    detalle: String? = null,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(fondo, RbTheme.shapes.card)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space1),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.bodyStrong,
            color = RbTheme.colors.textPrimary,
        )
        detalle?.let {
            Text(text = it, style = RbTheme.typography.support, color = RbTheme.colors.textPrimary)
        }
    }
}

/**
 * Lo que se explica antes -y en vez- del diálogo del sistema.
 *
 * Tres reglas que esta pantalla existe para cumplir:
 *
 * 1. **Se explica antes de que Android pregunte.** El cuadro del sistema dice
 *    "¿Permitir que RutBusiness tome fotos y grabe videos?", que en un mostrador
 *    suena a que la app va a guardar fotos del local. Acá se dice para qué es y
 *    qué **no** hacemos con la imagen, antes de que decidir sea urgente.
 * 2. **Nunca es un callejón sin salida.** Escribir el código a mano está
 *    siempre, con el mismo tamaño de toque que el botón de permitir.
 * 3. **Si Android ya no va a preguntar**, se explica el camino de Ajustes en
 *    palabras y en orden, no "habilite el permiso en la configuración".
 */
@Composable
internal fun ExplicacionDelPermiso(estado: EstadoDelPermiso, modifier: Modifier = Modifier) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    val titulo: String
    val cuerpo: String
    when (estado) {
        EstadoDelPermiso.NegadoParaSiempre -> {
            titulo = "La cámara quedó bloqueada"
            cuerpo = "Android no nos va a volver a preguntar. Para desbloquearla: toca " +
                "\"Abrir ajustes\", entra a Permisos, prende Cámara y vuelve acá con el botón de " +
                "atrás del teléfono. Si prefieres, sigue cobrando y escribe el código."
        }

        EstadoDelPermiso.Negado -> {
            titulo = "Sin la cámara no podemos leer el código"
            cuerpo = "La usamos sólo para leer el código de barras del producto. Puedes darle " +
                "permiso ahora, o seguir escribiendo el código a mano."
        }

        else -> {
            titulo = "Vamos a usar la cámara"
            cuerpo = "Sólo para leer el código de barras del producto y agregarlo a la venta. La " +
                "imagen se lee en este mismo teléfono: no se guarda ninguna foto y no sale de acá."
        }
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(text = titulo, style = RbTheme.typography.heading, color = colors.textPrimary)
        Text(text = cuerpo, style = RbTheme.typography.body, color = colors.textSecondary)
    }
}

/**
 * Los botones del permiso.
 *
 * `RbReflowRow` los deja lado a lado mientras entren y apila el segundo cuando
 * la letra al 200% partiría una palabra. Ninguno de los dos deja de estar.
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
    val dimens = RbTheme.dimens
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = "Este teléfono no tiene cámara que podamos usar",
            style = RbTheme.typography.heading,
            color = RbTheme.colors.textPrimary,
        )
        Text(
            text = "Se cobra igual: escribe el código de barras o busca el producto por nombre.",
            style = RbTheme.typography.body,
            color = RbTheme.colors.textSecondary,
        )
        RbButton(label = "Escribir a mano", onClick = onEscribirAMano, fillWidth = true)
    }
}
