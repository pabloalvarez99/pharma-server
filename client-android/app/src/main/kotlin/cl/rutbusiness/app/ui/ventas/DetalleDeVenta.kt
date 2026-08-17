package cl.rutbusiness.app.ui.ventas

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Qué llevaba una venta, cómo pagó, cuándo, y el camino para deshacerla.
 *
 * El paso de confirmar es una pantalla aparte dentro de este mismo composable
 * ([PasoDeVentas.Confirmando]) — mismo criterio que `PasoAbono.kt` en fiado —
 * y no un `AlertDialog`: así la confirmación crece con la letra al 200% igual
 * que el resto de la pantalla.
 */
@Composable
fun DetalleDeVenta(vm: VentasViewModel, feria: Boolean, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val venta = vm.ventaElegida
    val det = vm.detalle

    if (vm.paso == PasoDeVentas.Confirmando) {
        val montoFormateado = venta?.let { vm.moneda.formatear(it.total) } ?: ""
        PasoConfirmarDeshacer(
            feria = feria,
            montoFormateado = montoFormateado,
            deshaciendo = vm.deshaciendo,
            onConfirmar = { vm.deshacer(feria) },
            onCancelar = vm::cancelarDeshacer,
            modifier = modifier,
        )
        return
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        vm.avisoDeDeshecho?.let { aviso ->
            RbCard {
                RbChip(label = etiquetaVentaDevuelta(feria), tone = RbChipTone.Brand)
                Text(text = aviso, style = RbTheme.typography.body, color = colors.textPrimary)
            }
        }

        vm.errorDeAccion?.let { error ->
            RbErrorState(
                title = error.title,
                message = error.message,
                retryLabel = error.retryLabel,
                onRetry = null,
            )
        }

        when {
            det == null && vm.cargandoDetalle -> RbLoadingState(
                label = if (feria) "Buscando lo que llevó..." else "Cargando el detalle...",
            )

            det == null || venta == null -> Text(
                text = if (feria) "No pudimos ver lo que llevó." else "No pudimos cargar el detalle.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            else -> {
                RbCard(title = tituloQueLlevaba(feria)) {
                    det.items.forEachIndexed { indice, item ->
                        FilaDeItem(item = item, moneda = vm.moneda)
                        if (indice != det.items.lastIndex) RbDivider()
                    }
                }

                RbCard {
                    LineaDeDetalle(tituloComoPago(), etiquetaComoPago(venta.metodoDePago, feria))
                    LineaDeDetalle(tituloCuandoFue(), fechaYHora(venta.creadaEn))
                    RbDivider()
                    RbReflowRow(
                        spacing = dimens.space2,
                        modifier = Modifier.fillMaxWidth(),
                        content = {
                            Text(
                                text = if (feria) "Total" else "Total de la venta",
                                style = RbTheme.typography.bodyStrong,
                                color = colors.textPrimary,
                            )
                        },
                        trailing = {
                            RbAmount(
                                amount = vm.moneda.formatear(venta.total),
                                emphasis = RbAmountEmphasis.Headline,
                            )
                        },
                    )
                }

                if (venta.estaDevuelta) {
                    RbCard {
                        RbChip(label = etiquetaVentaDevuelta(feria), tone = RbChipTone.Neutral)
                    }
                } else {
                    BotonDeshacer(
                        feria = feria,
                        enabled = !vm.deshaciendo && det.items.isNotEmpty(),
                        onClick = vm::pedirConfirmarDeshacer,
                    )
                }
            }
        }
    }
}

@Composable
private fun FilaDeItem(item: ItemDeVentaDto, moneda: Moneda) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbReflowRow(
        spacing = dimens.space2,
        modifier = Modifier.fillMaxWidth().padding(vertical = dimens.space1),
        content = {
            Column {
                Text(
                    text = item.nombreDelProducto,
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
                )
                Text(
                    text = "${item.quantity} x ${moneda.formatear(item.precioUnitario)}",
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        },
        trailing = { RbAmount(amount = moneda.formatear(item.subtotal)) },
    )
}

@Composable
private fun LineaDeDetalle(titulo: String, valor: String) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    RbReflowRow(
        spacing = dimens.space2,
        modifier = Modifier.fillMaxWidth(),
        content = {
            Text(text = titulo, style = RbTheme.typography.body, color = colors.textSecondary)
        },
        trailing = {
            Text(text = valor, style = RbTheme.typography.bodyStrong, color = colors.textPrimary)
        },
    )
}

/**
 * El paso de confirmar, dentro de la misma pantalla. Dice qué va a pasar en
 * plata y en mercadería antes de tocar el server — nunca "¿estás seguro?".
 */
@Composable
private fun PasoConfirmarDeshacer(
    feria: Boolean,
    montoFormateado: String,
    deshaciendo: Boolean,
    onConfirmar: () -> Unit,
    onCancelar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = tituloConfirmarDeshacer(feria)) {
            Text(
                text = mensajeConfirmarDeshacer(feria, montoFormateado),
                style = RbTheme.typography.body,
                color = colors.textPrimary,
            )
        }

        RbButton(
            label = if (deshaciendo) ctaDeshaciendo(feria) else botonConfirmarDeshacer(feria),
            onClick = onConfirmar,
            variant = RbButtonVariant.Destructive,
            enabled = !deshaciendo,
            fillWidth = true,
        )

        RbButton(
            label = botonCancelarDeshacer(),
            onClick = onCancelar,
            variant = RbButtonVariant.Secondary,
            enabled = !deshaciendo,
            fillWidth = true,
        )
    }
}

/**
 * `dd-mm-aaaa HH:mm` de un instante ISO-8601, en el orden en que se lee en
 * Chile. Se recorta la cadena, mismo criterio que `fechaCorta` en
 * `FiadoScreen.kt`.
 */
internal fun fechaYHora(iso: String): String {
    val fecha = iso.substringBefore('T')
    val partes = fecha.split('-')
    val fechaCorta = if (partes.size == 3) "${partes[2]}-${partes[1]}-${partes[0]}" else fecha
    return "$fechaCorta ${horaCorta(iso)}"
}
