package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.input.KeyboardType
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.customers.ClienteDto
import cl.rutbusiness.core.pos.ItemCarrito
import cl.rutbusiness.core.pos.MedioDePago
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
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
 *
 * Visual de mesa: cada línea del carrito es una tarjeta gruesa; el CTA de
 * cobro es brand fill a 56dp de alto.
 */
@Composable
fun PasoPago(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val feria = esFeria()

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = copyTituloCarrito(feria)) {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
                vm.carrito.items.forEach { item ->
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
            }

            RbReflowRow(
                spacing = dimens.space2,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = dimens.space1),
                content = {
                    Text(
                        text = "Total",
                        style = RbTheme.typography.bodyStrong,
                        color = RbTheme.colors.textPrimary,
                    )
                },
                trailing = {
                    // Sin conexión **no va ningún número acá**. El total lo
                    // calcula el server, y sin server no hay a quién
                    // preguntarle: un monto armado en el teléfono, en la
                    // pantalla donde se cobra, es peor que no mostrar nada.
                    // Con conexión sigue el adelanto del mostrador, que el
                    // cobro reemplaza por el total real.
                    when {
                        !vm.hayConexion -> Text(
                            text = "se confirma al enviarse",
                            style = RbTheme.typography.body,
                            color = RbTheme.colors.textSecondary,
                        )
                        vm.carrito.total != null -> RbAmount(
                            amount = vm.moneda.formatear(vm.carrito.total!!),
                            emphasis = RbAmountEmphasis.Body,
                        )
                        else -> Text(
                            text = "lo confirma el sistema",
                            style = RbTheme.typography.body,
                            color = RbTheme.colors.textSecondary,
                        )
                    }
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
                text = copyOfflinePago(feria),
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
            // Brand fill Primary + fillWidth + ≥56dp (rbTouchTarget).
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
 * Una línea del carrito: tarjeta gruesa con nombre, precio y cantidad táctil.
 *
 * El control es un par de [RbButton] y no un componente propio: sumar un
 * "stepper" suelto acá sería exactamente el botón nuevo que el design system
 * pide no inventar.
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
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surfaceVariant)
            .border(dimens.border, colors.outline, shape)
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        RbReflowRow(
            spacing = dimens.space2,
            modifier = Modifier.fillMaxWidth(),
            content = {
                Text(
                    text = item.nombre,
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                )
            },
            trailing = {
                if (mostrarSubtotal) {
                    RbAmount(
                        amount = subtotal ?: "-",
                        emphasis = RbAmountEmphasis.Body,
                    )
                }
            },
        )

        Text(
            text = "$precioUnitario c/u",
            style = RbTheme.typography.support,
            color = colors.textSecondary,
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
                color = colors.textPrimary,
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
