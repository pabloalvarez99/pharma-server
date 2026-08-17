package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading
import kotlinx.coroutines.delay

/**
 * La tarjeta que la dueña lee antes de que se escriba nada en el negocio.
 *
 * Es la pieza más importante de esta pantalla y la única defensa contra que el
 * agente entienda mal. Si acá dice algo ambiguo, se registra un gasto que no
 * era o se ajusta un stock que no correspondía. Todo lo de abajo sale de eso:
 *
 * - **Encabezado honesto**, no "Confirmación". Todavía no pasó nada.
 * - **Se lee en voz alta**: en feria suena a alguien repitiendo lo que oyó en
 *   el puesto ("Sí, anotá eso" / "Mejor no"), no a un admin de ERP.
 * - **El resumen del server, grande.** Es la frase armada con datos reales.
 * - **El detalle campo por campo** ([TarjetaDetalleAccion]), con la plata
 *   formateada. El resumen se lee rápido; el detalle pilla un cero de más.
 * - **El vencimiento en palabras.** Un timestamp no le sirve a nadie.
 *
 * A 200% de escala esta tarjeta es el peor caso de toda la app: la que más
 * texto tiene y la que no se puede cortar. Por eso nada acá lleva `maxLines`
 * y ninguna altura es fija.
 */
@Composable
fun TarjetaPropuesta(
    mensaje: Mensaje.Propuesta,
    segundosRestantes: Long?,
    onConfirmar: () -> Unit,
    onCancelar: () -> Unit,
    onVencer: () -> Unit,
    onVolverAPedir: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card
    val feria = esFeria()

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            // brandContainer: se lee como "te lo digo en voz alta", no como
            // panel admin. El borde de marca sigue marcando la decisión.
            .background(colors.brandContainer)
            .border(dimens.focusRing, colors.brandText, shape)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        // Honestidad primero: todavía no se anotó nada. Misma frase en feria
        // y retail; el tono del puesto vive en los botones y el cierre.
        Text(
            text = copyEncabezado(),
            style = RbTheme.typography.heading,
            color = colors.brandText,
            modifier = Modifier.rbHeading(),
        )

        Text(
            text = mensaje.propuesta.summary,
            style = RbTheme.typography.bodyStrong,
            color = colors.textPrimary,
        )

        Detalle(mensaje.propuesta)

        when (val estado = mensaje.estado) {
            EstadoPropuesta.Esperando -> Esperando(
                mensaje = mensaje,
                segundosRestantes = segundosRestantes,
                feria = feria,
                onConfirmar = onConfirmar,
                onCancelar = onCancelar,
                onVencer = onVencer,
            )

            EstadoPropuesta.Confirmando -> RbLoadingState(
                label = copyCargando(feria),
            )

            is EstadoPropuesta.Hecha -> Cierre(
                texto = estado.texto,
                tono = Tono.Bien,
            )

            EstadoPropuesta.Cancelada -> Cierre(
                texto = copyCierreCancelada(feria),
                tono = Tono.Neutro,
            )

            is EstadoPropuesta.YaNoSirve -> Column(
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                Cierre(texto = estado.texto, tono = Tono.Atencion)
                RbButton(
                    label = copyBotonVolverAPedir(feria),
                    onClick = { onVolverAPedir(mensaje.pregunta) },
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                )
            }
        }
    }
}

/**
 * El bloque de decisión: vencimiento, confirmar, cancelar.
 *
 * El contador corre acá y no en el `ViewModel` para que solo exista mientras
 * hay una tarjeta esperando en pantalla. Un `ViewModel` con un tick eterno le
 * cobra batería a un teléfono viejo por una tarjeta que la dueña ya cerró.
 */
@Composable
private fun Esperando(
    mensaje: Mensaje.Propuesta,
    segundosRestantes: Long?,
    feria: Boolean,
    onConfirmar: () -> Unit,
    onCancelar: () -> Unit,
    onVencer: () -> Unit,
) {
    val dimens = RbTheme.dimens
    var restantes by remember(mensaje.id) { mutableStateOf(segundosRestantes) }

    LaunchedEffect(mensaje.id) {
        var quedan = segundosRestantes ?: return@LaunchedEffect
        // Cada 10 segundos: suficiente para que el texto no mienta, y lo
        // bastante espaciado para no despertar la pantalla al pedo.
        while (quedan > 0) {
            delay(10_000)
            quedan -= 10
            restantes = quedan
        }
        onVencer()
    }

    Column(verticalArrangement = Arrangement.spacedBy(dimens.space3)) {
        restantes?.let {
            Text(
                text = vencimientoEnPalabras(it, feria),
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }

        // Un solo verbo primario a ancho completo: se siente como "sí, hazlo",
        // no como un par de botones de chatbot simétricos.
        RbButton(
            label = etiquetaDeConfirmar(mensaje.propuesta.name, feria),
            onClick = onConfirmar,
            variant = RbButtonVariant.Primary,
            fillWidth = true,
        )
        RbButton(
            label = etiquetaDeCancelar(feria),
            onClick = onCancelar,
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )
    }
}

/** El detalle campo por campo — tarjeta legible del puesto. */
@Composable
private fun Detalle(propuesta: PropuestaAccion) {
    val lineas = remember(propuesta.confirmToken) { DetalleAccion.lineas(propuesta.params) }
    TarjetaDetalleAccion(lineas = lineas)
}

private enum class Tono { Bien, Neutro, Atencion }

/** Lo que quedó, una vez que la tarjeta dejó de pedir una decisión. */
@Composable
private fun Cierre(texto: String, tono: Tono) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    // La tarjeta ya es brandContainer; el cierre usa surface / surfaceVariant /
    // dangerContainer para no fundirse con el fondo de la propuesta.
    val fondo = when (tono) {
        Tono.Bien -> colors.surface
        Tono.Neutro -> colors.surfaceVariant
        Tono.Atencion -> colors.dangerContainer
    }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RbTheme.shapes.field)
            .background(fondo)
            .padding(dimens.space3),
    ) {
        Text(
            text = texto,
            style = RbTheme.typography.body,
            color = colors.textPrimary,
        )
    }
}
