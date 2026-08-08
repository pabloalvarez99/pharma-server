package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import cl.rutbusiness.app.ui.impresora.TarjetaDeImpresion
import cl.rutbusiness.app.ui.impresora.impresoraViewModel
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.pos.ComprobanteDto
import cl.rutbusiness.core.pos.MedioDePago
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbAssertive

/**
 * Paso 3: la venta quedó.
 *
 * Todo lo que se muestra acá lo calculó el server -incluido el vuelto, que
 * viene en `change` del comprobante-. La app no suma ni resta un peso en esta
 * pantalla; si lo hiciera, el papel y la caja dirían cosas distintas.
 *
 * El vuelto va primero y grande porque es lo único que la cajera necesita en
 * los tres segundos siguientes: tiene la mano en el cajón.
 */
@Composable
fun PasoComprobante(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val comprobante = vm.comprobante

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Vuelto(
            comprobante = comprobante,
            totalCobrado = vm.totalCobrado,
            moneda = vm.moneda,
        )

        if (comprobante != null) {
            DetalleDelComprobante(comprobante, vm.moneda)
        } else {
            // La venta se cobró pero el comprobante no llegó. Decirlo, no
            // esconderlo: el cajero tiene que saber que el cobro sí pasó.
            RbCard(title = "La venta quedó registrada") {
                Text(
                    text = "No alcanzamos a traer el detalle del comprobante, pero el cobro ya " +
                        "está hecho. Lo ves en «Tu día», que ya cuenta esta venta.",
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textSecondary,
                )
            }
        }

        // La boleta va **antes** de "Cobrar otra venta" y nunca lo tapa: la
        // impresora es lo siguiente que se hace, pero la venta ya está cobrada
        // y ningún problema de papel puede dejar a la cajera sin poder seguir.
        // Si nadie proveyó la impresora, esto simplemente no se dibuja.
        impresoraViewModel()?.let { impresora ->
            TarjetaDeImpresion(
                vm = impresora,
                ordenId = comprobante?.orderId,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        if (vm.puntosGanados > 0) {
            Text(
                text = "Sumó ${vm.puntosGanados} ${if (vm.puntosGanados == 1L) "punto" else "puntos"} de fidelidad.",
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }

        RbButton(
            label = "Cobrar otra venta",
            onClick = vm::nuevaVenta,
            fillWidth = true,
        )
    }
}

@Composable
private fun Vuelto(
    comprobante: ComprobanteDto?,
    totalCobrado: String?,
    moneda: Moneda,
) {
    val colors = RbTheme.colors
    val vuelto = comprobante?.change
    val total = comprobante?.total ?: totalCobrado

    RbCard(modifier = Modifier.rbAssertive()) {
        if (vuelto != null) {
            Text(
                text = "Vuelto",
                style = RbTheme.typography.label,
                color = colors.textSecondary,
            )
            Text(
                text = moneda.formatear(vuelto),
                style = RbTheme.typography.title,
                color = colors.brandText,
            )
        } else {
            Text(
                text = "Cobrado",
                style = RbTheme.typography.label,
                color = colors.textSecondary,
            )
        }

        if (total != null) {
            Text(
                text = if (vuelto != null) {
                    "Total de la venta: ${moneda.formatear(total)}"
                } else {
                    moneda.formatear(total)
                },
                style = if (vuelto != null) RbTheme.typography.body else RbTheme.typography.title,
                color = colors.textPrimary,
            )
        }

        comprobante?.paymentMethod?.let { codigo ->
            Text(
                text = etiquetaDeMedio(codigo),
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
}

@Composable
private fun DetalleDelComprobante(comprobante: ComprobanteDto, moneda: Moneda) {
    val dimens = RbTheme.dimens

    RbCard(title = comprobante.tenantName) {
        Text(
            text = "Comprobante ${comprobante.folio.take(8)}",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )

        RbDivider()

        comprobante.items.forEach { linea ->
            RbReflowRow(
                spacing = dimens.space2,
                modifier = Modifier.fillMaxWidth(),
                content = {
                    Text(
                        text = "${linea.qty} × ${linea.name}",
                        style = RbTheme.typography.body,
                        color = RbTheme.colors.textPrimary,
                    )
                },
                trailing = {
                    Text(
                        text = moneda.formatear(linea.lineTotal),
                        style = RbTheme.typography.numeric,
                        color = RbTheme.colors.textPrimary,
                    )
                },
            )
        }

        RbDivider()

        FilaDeMonto("Subtotal", moneda.formatear(comprobante.subtotal))
        if (comprobante.discount != "0") {
            FilaDeMonto("Descuento", moneda.formatear(comprobante.discount))
        }
        FilaDeMonto("Total", moneda.formatear(comprobante.total), destacado = true)
        comprobante.cashAmount?.let { FilaDeMonto("Pagó con", moneda.formatear(it)) }
        comprobante.change?.let { FilaDeMonto("Vuelto", moneda.formatear(it)) }

        if (comprobante.footerNote.isNotBlank()) {
            Text(
                text = comprobante.footerNote,
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }
    }
}

@Composable
private fun FilaDeMonto(etiqueta: String, monto: String, destacado: Boolean = false) {
    RbReflowRow(
        spacing = RbTheme.dimens.space2,
        modifier = Modifier.fillMaxWidth(),
        content = {
            Text(
                text = etiqueta,
                style = if (destacado) RbTheme.typography.bodyStrong else RbTheme.typography.body,
                color = RbTheme.colors.textPrimary,
            )
        },
        trailing = {
            Text(
                text = monto,
                style = RbTheme.typography.numeric,
                color = RbTheme.colors.textPrimary,
            )
        },
    )
}

/** El código del server, dicho como lo diría una persona. */
private fun etiquetaDeMedio(codigo: String): String =
    MedioDePago.entries.firstOrNull { it.codigo == codigo }?.etiqueta
        ?: when (codigo) {
            "pos_debit" -> "Tarjeta de débito"
            "pos_credit" -> "Tarjeta de crédito"
            "pos_mixed" -> "Pago mixto"
            else -> codigo
        }
