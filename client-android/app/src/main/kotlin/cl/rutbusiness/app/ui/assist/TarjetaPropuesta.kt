package cl.rutbusiness.app.ui.assist

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
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
 * - **Encabezado que dice qué es esto**, no "Confirmación". La dueña tiene que
 *   saber en un vistazo que todavía no pasó nada.
 * - **El resumen del server, grande.** Es la frase que el server armó con los
 *   datos reales; va en el cuerpo más grande de la pantalla.
 * - **El detalle campo por campo**, con la plata formateada. El resumen puede
 *   leerse rápido; el detalle es donde se pilla un cero de más.
 * - **Botones con verbo**, no "Aceptar". "Sí, registrar el gasto" se entiende
 *   incluso si alguien no leyó el resto.
 * - **El vencimiento en palabras.** Un timestamp no le sirve a nadie.
 *
 * A 200% de escala esta tarjeta es el peor caso de toda la app: la que más
 * texto tiene y la que no se puede cortar. Por eso nada acá lleva `maxLines`,
 * ninguna altura es fija, y el detalle usa [RbReflowRow], que baja el valor a
 * su propia línea antes de dejar que se parta una palabra.
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
        Text(
            text = "Antes de hacerlo, revisa",
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
                onConfirmar = onConfirmar,
                onCancelar = onCancelar,
                onVencer = onVencer,
            )

            EstadoPropuesta.Confirmando -> RbLoadingState(label = "Guardándolo…")

            is EstadoPropuesta.Hecha -> Cierre(
                texto = estado.texto,
                tono = Tono.Bien,
            )

            EstadoPropuesta.Cancelada -> Cierre(
                texto = if (esFeria()) {
                    "No lo hice. No cambió nada en tu puesto."
                } else {
                    "No lo hice. No cambió nada en tu negocio."
                },
                tono = Tono.Neutro,
            )

            is EstadoPropuesta.YaNoSirve -> Column(
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                Cierre(texto = estado.texto, tono = Tono.Atencion)
                RbButton(
                    label = "Pedirlo de nuevo",
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
                text = Vencimiento.enPalabras(it),
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }

        // Un solo verbo primario a ancho completo: se siente como "sí, hazlo",
        // no como un par de botones de chatbot simétricos.
        RbButton(
            label = etiquetaDeConfirmar(mensaje.propuesta.name),
            onClick = onConfirmar,
            variant = RbButtonVariant.Primary,
            fillWidth = true,
        )
        RbButton(
            label = "No, déjalo así",
            onClick = onCancelar,
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )
    }
}

/**
 * El verbo exacto de lo que va a pasar.
 *
 * "Confirmar" no dice nada. Una etiqueta que nombra la acción es lo único que
 * alguien lee seguro, incluso apurado, así que ahí es donde tiene que estar la
 * información. Las etiquetas salen de `Action::label` en
 * `crates/assist/src/actions.rs`; una que no conozcamos cae en una frase
 * genérica pero honesta en vez de inventar un verbo equivocado.
 */
internal fun etiquetaDeConfirmar(nombreDeAccion: String): String = when (nombreDeAccion) {
    "registrar_gasto" -> "Sí, registrar el gasto"
    "registrar_abono" -> "Sí, registrar el abono"
    "crear_cliente" -> "Sí, crear el cliente"
    "crear_proveedor" -> "Sí, crear el proveedor"
    "crear_producto_rapido" -> "Sí, crear el producto"
    "ajustar_precio" -> "Sí, cambiar el precio"
    "ajustar_stock" -> "Sí, ajustar el stock"
    "abrir_caja" -> "Sí, abrir la caja"
    "cerrar_caja" -> "Sí, cerrar la caja"
    "crear_orden_compra_draft" -> "Sí, crear la orden de compra"
    "dispensar_receta" -> "Sí, dispensar la receta"
    "registrar_venta" -> "Sí, registrar la venta"
    "registrar_fiado" -> "Sí, registrar el fiado"
    else -> "Sí, hazlo"
}

/** El detalle campo por campo. */
@Composable
private fun Detalle(propuesta: PropuestaAccion) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val lineas = remember(propuesta.confirmToken) { DetalleAccion.lineas(propuesta.params) }

    if (lineas.isEmpty()) return

    // Detalle sobre surface dentro del brandContainer de la tarjeta: se lee
    // el desglose sin competir con el resumen hablado de arriba.
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RbTheme.shapes.field)
            .background(colors.surface)
            .border(dimens.border, colors.outline, RbTheme.shapes.field)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = "Esto es lo que voy a guardar",
            style = RbTheme.typography.label,
            color = colors.textSecondary,
        )

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(colors.outline)
                .clearAndSetSemantics { },
        )

        lineas.forEach { linea ->
            RbReflowRow(
                spacing = dimens.space3,
                modifier = Modifier.fillMaxWidth(),
                content = {
                    Text(
                        text = linea.etiqueta,
                        style = RbTheme.typography.support,
                        color = colors.textSecondary,
                    )
                },
                trailing = {
                    // El valor en el cuerpo fuerte: es el número que hay que
                    // mirar. La etiqueta es contexto, el valor es la decisión.
                    Text(
                        text = linea.valor,
                        style = RbTheme.typography.bodyStrong,
                        color = colors.textPrimary,
                    )
                },
            )
        }
    }
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
