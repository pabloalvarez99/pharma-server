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
import cl.rutbusiness.app.ui.gente.compartirConGente
import cl.rutbusiness.app.ui.impresora.TarjetaDeImpresion
import cl.rutbusiness.app.ui.impresora.impresoraViewModel
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.pos.ComprobanteDto
import cl.rutbusiness.core.pos.MedioDePago
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbAssertive

/**
 * Paso 3: la venta quedó.
 *
 * Se siente un **papel del puesto**, no un ticket fiscal: cifra hero arriba
 * (vuelto o cobrado), cuerpo calmado con lo que se llevó, y un CTA de 56dp
 * para la siguiente venta. Todo lo que se muestra acá lo calculó el server
 * —incluido el vuelto, que viene en `change`—. La app no suma ni resta un peso.
 *
 * El vuelto va primero y grande porque es lo único que la cajera necesita en
 * los tres segundos siguientes: tiene la mano en el cajón / en el bolsillo.
 */
@Composable
fun PasoComprobante(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val comprobante = vm.comprobante
    val feria = esFeria()

    // La venta que quedó en la cola tiene su propia pantalla y no reusa ésta.
    // No es un comprobante: no hay folio, no hay total y no hay vuelto, porque
    // todavía no la vio el sistema. Mostrarla con la misma cara que una venta
    // cobrada sería el peor malentendido posible de esta app.
    vm.ventaEncolada?.let { encolada ->
        VentaGuardada(
            vm = vm,
            unidades = encolada.solicitud.items.sumOf { it.quantity },
            feria = feria,
        )
        return
    }

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        // space4 entre bloques: se leen de a uno, como un papel, no un panel.
        verticalArrangement = Arrangement.spacedBy(dimens.space4),
    ) {
        Vuelto(
            comprobante = comprobante,
            totalCobrado = vm.totalCobrado,
            moneda = vm.moneda,
        )

        if (comprobante != null) {
            DetalleDelComprobante(comprobante, vm.moneda, feria = feria)
        } else {
            // La venta se cobró pero el papel no llegó. Decirlo, no esconderlo.
            RbCard(title = copyTituloSinDetalle()) {
                Text(
                    text = copySinDetalleComprobante(feria),
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

        // Mandar el mismo papel por chat: en feria muchas veces no hay
        // impresora en el puesto y esto es la salida de todos los días, no
        // un premio de consuelo. Sin culpa si no se imprime ni se manda:
        // la venta ya quedó cobrada arriba. Si no hay detalle o no hay
        // puerto de plataforma (previews, iOS sin implementar), no se dibuja.
        if (comprobante != null) {
            BotonMandarPorChat(comprobante, vm.moneda, feria = feria)
        }

        if (vm.puntosGanados > 0) {
            Text(
                text = copyPuntosGanados(vm.puntosGanados),
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }

        // Brand fill Primary + fillWidth + ≥56dp (rbTouchTarget).
        RbButton(
            label = copyBotonCobrarOtraVenta(),
            onClick = vm::nuevaVenta,
            fillWidth = true,
        )
    }
}

/**
 * Botón secundario para mandar el comprobante por el chat de siempre.
 *
 * Reusa el puerto [compartirConGente] (ADR-0021, `cl.rutbusiness.app.ui.gente`):
 * mismo `ACTION_SEND` genérico que ya usa el recado del día, sin abrir una
 * segunda vía para lo mismo. Si no hay puerto de plataforma detrás, no se
 * dibuja nada —mismo contrato que el resto de esa serie—.
 */
@Composable
private fun BotonMandarPorChat(comprobante: ComprobanteDto, moneda: Moneda, feria: Boolean) {
    val compartir = compartirConGente() ?: return
    val titulo = comprobante.tenantName.takeIf { it.isNotBlank() } ?: copyTituloCarrito(feria)
    val lineas = comprobante.items.map { linea ->
        "${linea.qty} × ${linea.name}  ${moneda.formatear(linea.lineTotal)}"
    }
    val mensaje = copyMensajeParaCompartir(
        titulo = titulo,
        referencia = copyRefPapel(comprobante.folio),
        lineas = lineas,
        total = moneda.formatear(comprobante.total),
        vuelto = comprobante.change?.let { moneda.formatear(it) },
    )

    RbButton(
        label = copyBotonMandarPorChat(),
        onClick = { compartir(mensaje) },
        variant = RbButtonVariant.Secondary,
        fillWidth = true,
    )
}

/**
 * La venta se cobró en el mostrador pero todavía no llegó al sistema.
 *
 * **Cero plata en esta pantalla.** Ni total, ni vuelto, ni subtotal: no hay
 * ningún monto que el server haya confirmado, y el único que el teléfono podría
 * poner sería uno que calculó solo. La cajera ya tiene los billetes en la mano
 * y sabe cuánto le dieron; lo que necesita saber acá es otra cosa, y es que la
 * venta no se perdió.
 *
 * Tampoco se ofrece imprimir: una boleta necesita folio y el folio lo asigna el
 * server. Un papel con los productos y sin folio no es una boleta.
 */
@Composable
private fun VentaGuardada(vm: CobrarViewModel, unidades: Int, feria: Boolean) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space4),
    ) {
        RbCard(modifier = Modifier.rbAssertive()) {
            Text(
                text = copyTituloVentaGuardada(),
                style = RbTheme.typography.heading,
                color = colors.brandText,
            )
            Text(
                text = copyDetalleVentaGuardada(),
                style = RbTheme.typography.body,
                color = colors.textPrimary,
            )
        }

        RbCard(title = copyTituloCarrito(feria)) {
            vm.carrito.items.forEach { item ->
                Text(
                    text = "${item.cantidad} × ${item.nombre}",
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
                )
            }
            Text(
                text = copyPieVentaEncolada(feria = feria, unidades = unidades),
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }

        RbButton(
            label = copyBotonCobrarOtraVenta(),
            onClick = vm::nuevaVenta,
            fillWidth = true,
        )
    }
}

/**
 * La cifra que se mira de lejos: vuelto si hubo efectivo, cobrado si no.
 *
 * Misma lectura que TarjetaDelDía / caja abierta: etiqueta callada + hero con
 * aire. Un solo Headline por pantalla.
 */
@Composable
private fun Vuelto(
    comprobante: ComprobanteDto?,
    totalCobrado: String?,
    moneda: Moneda,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val vuelto = comprobante?.change
    val total = comprobante?.total ?: totalCobrado

    RbCard(modifier = Modifier.rbAssertive()) {
        Text(
            text = if (vuelto != null) copyEtiquetaVuelto() else copyEtiquetaCobrado(),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )

        when {
            vuelto != null -> RbAmount(
                amount = moneda.formatear(vuelto),
                emphasis = RbAmountEmphasis.Headline,
                color = colors.brandText,
                modifier = Modifier.padding(vertical = dimens.space2),
            )
            total != null -> RbAmount(
                amount = moneda.formatear(total),
                emphasis = RbAmountEmphasis.Headline,
                color = colors.brandText,
                modifier = Modifier.padding(vertical = dimens.space2),
            )
        }

        if (vuelto != null && total != null) {
            Text(
                text = copyTotalDeLaVenta(moneda.formatear(total)),
                style = RbTheme.typography.body,
                color = colors.textSecondary,
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

/**
 * El cuerpo del papel: nombre del puesto, ref quieta, líneas y total.
 *
 * No grita "Comprobante" ni se lee como extracto fiscal: es lo que se llevó y
 * cuánto salió, con aire entre filas.
 */
@Composable
private fun DetalleDelComprobante(
    comprobante: ComprobanteDto,
    moneda: Moneda,
    feria: Boolean,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val titulo = comprobante.tenantName.takeIf { it.isNotBlank() }
        ?: copyTituloCarrito(feria)

    RbCard(title = titulo) {
        Text(
            text = copyRefPapel(comprobante.folio),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )

        RbDivider()

        Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
            comprobante.items.forEach { linea ->
                RbReflowRow(
                    spacing = dimens.space2,
                    modifier = Modifier.fillMaxWidth(),
                    content = {
                        Text(
                            text = "${linea.qty} × ${linea.name}",
                            style = RbTheme.typography.body,
                            color = colors.textPrimary,
                        )
                    },
                    trailing = {
                        RbAmount(
                            amount = moneda.formatear(linea.lineTotal),
                            emphasis = RbAmountEmphasis.Body,
                        )
                    },
                )
            }
        }

        RbDivider()

        FilaDeMonto(copyEtiquetaSubtotal(), moneda.formatear(comprobante.subtotal))
        if (comprobante.discount != "0") {
            FilaDeMonto(copyEtiquetaDescuento(), moneda.formatear(comprobante.discount))
        }
        FilaDeMonto(copyEtiquetaTotalLinea(), moneda.formatear(comprobante.total), destacado = true)
        comprobante.cashAmount?.let { FilaDeMonto(copyEtiquetaPagoCon(), moneda.formatear(it)) }
        comprobante.change?.let { FilaDeMonto(copyEtiquetaVueltoLinea(), moneda.formatear(it)) }

        if (comprobante.footerNote.isNotBlank()) {
            Text(
                text = comprobante.footerNote,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
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
                color = if (destacado) {
                    RbTheme.colors.textPrimary
                } else {
                    RbTheme.colors.textSecondary
                },
            )
        },
        trailing = {
            RbAmount(
                amount = monto,
                emphasis = RbAmountEmphasis.Body,
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
