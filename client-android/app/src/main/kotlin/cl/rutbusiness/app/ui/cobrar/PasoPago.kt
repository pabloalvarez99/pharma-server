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
                    // El subtotal de la línea es una multiplicación, y sin
                    // server esa multiplicación la haría el teléfono. Se calla:
                    // el precio unitario sí se muestra porque lo mandó el
                    // server tal cual, y la cantidad la puso la cajera.
                    subtotal = if (vm.hayConexion) {
                        item.subtotal?.let { vm.moneda.formatear(it) }
                    } else {
                        null
                    },
                    mostrarSubtotal = vm.hayConexion,
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
                        // Sin conexión **no va ningún número acá**. El total lo
                        // calcula el server, y sin server no hay a quién
                        // preguntarle: un monto armado en el teléfono, en la
                        // pantalla donde se cobra, es peor que no mostrar nada.
                        // Con conexión sigue el adelanto del mostrador, que el
                        // cobro reemplaza por el total real.
                        text = when {
                            !vm.hayConexion -> "se confirma al enviarse"
                            else -> vm.carrito.total?.let { vm.moneda.formatear(it) }
                                ?: "lo confirma el sistema"
                        },
                        style = if (vm.hayConexion) {
                            RbTheme.typography.title
                        } else {
                            RbTheme.typography.body
                        },
                        color = if (vm.hayConexion) {
                            RbTheme.colors.textPrimary
                        } else {
                            RbTheme.colors.textSecondary
                        },
                    )
                },
            )
        }

        RbCard(title = "Cómo paga") {
            RbChipRow {
                MedioDePago.entries.forEach { opcion ->
                    val bloqueado = vm.motivoParaNoUsar(opcion) != null
                    RbChip(
                        label = opcion.etiqueta,
                        tone = when {
                            bloqueado -> RbChipTone.Neutral
                            vm.medio == opcion -> RbChipTone.Brand
                            else -> RbChipTone.Neutral
                        },
                        selected = vm.medio == opcion && !bloqueado,
                        // Sin `onClick` el chip deja de ser tocable. Es a
                        // propósito que quede a la vista y no que desaparezca:
                        // la dueña sabe que el fiado existe, y un botón que se
                        // esfumó la deja buscándolo. Abajo dice por qué no.
                        onClick = if (bloqueado) null else ({ vm.cambiarMedio(opcion) }),
                    )
                }
            }

            // Lo que no se puede, dicho **antes** de que lo intente. Un cartel
            // después del toque llega tarde: el cliente ya está esperando.
            MedioDePago.entries
                .mapNotNull { vm.motivoParaNoUsar(it) }
                .distinct()
                .forEach { motivo ->
                    Text(
                        text = motivo,
                        style = RbTheme.typography.support,
                        color = RbTheme.colors.textSecondary,
                    )
                }

            if (vm.medio.pideMontoEntregado && vm.hayConexion) {
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

            if (vm.medio.exigeCliente && vm.hayConexion) {
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

        if (!vm.hayConexion) {
            Text(
                text = "Sin conexión con el sistema del negocio. La venta se guarda en el " +
                    "teléfono y se envía sola cuando vuelva la señal. El total se confirma " +
                    "cuando se envíe.",
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }

        RbButton(
            // "Guardar venta" y no "Confirmar venta": sin señal no se confirmó
            // nada todavía, y la palabra tiene que decir lo que de verdad pasa.
            label = when {
                vm.cobrando -> "Cobrando..."
                !vm.hayConexion -> "Guardar venta"
                else -> "Confirmar venta"
            },
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
    mostrarSubtotal: Boolean,
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
                if (mostrarSubtotal) {
                    Text(
                        text = subtotal ?: "-",
                        style = RbTheme.typography.numeric,
                        color = RbTheme.colors.textPrimary,
                    )
                }
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
        RbEmptyState(
            title = "No hay clientes cargados",
            hint = "El fiado queda en la cuenta de alguien. Crea el cliente en el sistema del " +
                "negocio y va a aparecer acá.",
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
