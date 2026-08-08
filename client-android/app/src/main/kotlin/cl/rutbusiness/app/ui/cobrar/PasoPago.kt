package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import cl.rutbusiness.core.customers.ClienteDto
import cl.rutbusiness.core.pos.ItemCarrito
import cl.rutbusiness.core.pos.MedioDePago
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 2: qué lleva y cómo paga.
 *
 * Todo en una columna que scrollea. A 200% de escala esta pantalla mide más de
 * dos pantallazos de alto, y eso está bien: lo que no puede pasar es que el
 * botón de confirmar quede fuera de alcance o que un texto se corte.
 */
@Composable
fun PasoPago(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = "Lo que lleva") {
            vm.carrito.items.forEachIndexed { indice, item ->
                if (indice > 0) RbDivider()
                FilaDeCarrito(
                    item = item,
                    precioUnitario = vm.moneda.formatear(item.precioUnitario),
                    subtotal = item.subtotal?.let { vm.moneda.formatear(it) },
                    onMenos = { vm.cambiarCantidad(item.productoId, item.cantidad - 1) },
                    onMas = { vm.cambiarCantidad(item.productoId, item.cantidad + 1) },
                )
            }

            RbDivider()

            RbReflowRow(
                spacing = dimens.space2,
                modifier = Modifier.fillMaxWidth(),
                content = {
                    Text(
                        text = "Total",
                        style = RbTheme.typography.bodyStrong,
                        color = RbTheme.colors.textPrimary,
                    )
                },
                trailing = {
                    Text(
                        // El total definitivo lo pone el server al cobrar; éste
                        // es lo que va sumando el mostrador.
                        text = vm.carrito.total?.let { vm.moneda.formatear(it) }
                            ?: "lo confirma el sistema",
                        style = RbTheme.typography.title,
                        color = RbTheme.colors.textPrimary,
                    )
                },
            )
        }

        RbCard(title = "Cómo paga") {
            RbChipRow {
                MedioDePago.entries.forEach { opcion ->
                    RbChip(
                        label = opcion.etiqueta,
                        tone = if (vm.medio == opcion) RbChipTone.Brand else RbChipTone.Neutral,
                        selected = vm.medio == opcion,
                        onClick = { vm.cambiarMedio(opcion) },
                    )
                }
            }

            if (vm.medio.pideMontoEntregado) {
                cl.rutbusiness.ui.components.RbTextField(
                    value = vm.montoEntregado,
                    onValueChange = vm::cambiarMontoEntregado,
                    label = "¿Con cuánto paga?",
                    placeholder = "0",
                    supportingText = "El vuelto lo calcula el sistema, no la app.",
                    numeric = true,
                    keyboardType = KeyboardType.Number,
                )
            }

            if (vm.medio.exigeCliente) {
                SelectorDeCliente(
                    clientes = vm.clientes,
                    elegido = vm.cliente,
                    onElegir = vm::elegirCliente,
                )
            }
        }

        vm.errorPago?.let { error ->
            RbErrorState(
                title = error.title,
                message = error.message,
                retryLabel = error.retryLabel,
                onRetry = if (error.retryLabel != null) vm::cobrar else null,
            )
        }

        vm.impedimentoParaCobrar()?.let { motivo ->
            Text(
                text = motivo,
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }

        RbButton(
            label = if (vm.cobrando) "Cobrando..." else "Confirmar venta",
            onClick = vm::cobrar,
            // Deshabilitado mientras se manda: el primer candado contra el
            // doble toque. El segundo, el que de verdad importa cuando la red
            // se corta, es la clave de idempotencia del ViewModel.
            enabled = !vm.cobrando && vm.impedimentoParaCobrar() == null,
            fillWidth = true,
        )

        RbButton(
            label = "Seguir agregando",
            onClick = vm::volverABuscar,
            variant = RbButtonVariant.Secondary,
            enabled = !vm.cobrando,
            fillWidth = true,
        )
    }
}

/**
 * Una línea del carrito con su control de cantidad.
 *
 * El control es un par de [RbButton] y no un componente propio: sumar un
 * "stepper" suelto acá sería exactamente el botón nuevo que el design system
 * pide no inventar. Queda anotado como hueco del catálogo.
 */
@Composable
private fun FilaDeCarrito(
    item: ItemCarrito,
    precioUnitario: String,
    subtotal: String?,
    onMenos: () -> Unit,
    onMas: () -> Unit,
) {
    val dimens = RbTheme.dimens

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(dimens.space1),
    ) {
        RbReflowRow(
            spacing = dimens.space2,
            modifier = Modifier.fillMaxWidth(),
            content = {
                Text(
                    text = item.nombre,
                    style = RbTheme.typography.bodyStrong,
                    color = RbTheme.colors.textPrimary,
                )
            },
            trailing = {
                Text(
                    text = subtotal ?: "-",
                    style = RbTheme.typography.numeric,
                    color = RbTheme.colors.textPrimary,
                )
            },
        )

        Text(
            text = "$precioUnitario c/u",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )

        Row(
            horizontalArrangement = Arrangement.spacedBy(dimens.space2),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RbButton(
                label = "−",
                onClick = onMenos,
                variant = RbButtonVariant.Secondary,
            )
            Text(
                text = "${item.cantidad}",
                style = RbTheme.typography.title,
                color = RbTheme.colors.textPrimary,
            )
            RbButton(
                label = "+",
                onClick = onMas,
                variant = RbButtonVariant.Secondary,
            )
        }
    }
}

@Composable
private fun SelectorDeCliente(
    clientes: List<ClienteDto>,
    elegido: ClienteDto?,
    onElegir: (ClienteDto?) -> Unit,
) {
    if (clientes.isEmpty()) {
        // El peor vacío que tenía la app: la cajera está a mitad de una venta,
        // ya eligió Fiado, y el cartel le decía que fuera "al sistema del
        // negocio" a crear el cliente. Desde el teléfono no hay tal cosa: era
        // un callejón sin salida en el paso más caro de todos, con alguien
        // esperando al otro lado del mostrador.
        //
        // Ahora dice las dos cosas que sí puede hacer ahora mismo, y en el orden
        // en que sirven: cobrar de otra forma para no frenar la venta, o pedirle
        // el cliente al agente.
        RbEmptyState(
            title = "Todavía no tienes clientes anotados",
            hint = "El fiado queda en la cuenta de una persona, así que primero hay que " +
                "anotarla. Cobra esta venta de otra forma para no hacer esperar, y después " +
                "pídeselo al agente: «agrega el cliente Juan Pérez».",
        )
        return
    }

    Text(
        text = "¿A quién se le fía?",
        style = RbTheme.typography.label,
        color = RbTheme.colors.textPrimary,
    )
    RbChipRow {
        clientes.forEach { cliente ->
            RbChip(
                label = cliente.name,
                tone = if (elegido?.id == cliente.id) RbChipTone.Brand else RbChipTone.Neutral,
                selected = elegido?.id == cliente.id,
                onClick = { onElegir(if (elegido?.id == cliente.id) null else cliente) },
            )
        }
    }
}
