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
import androidx.compose.ui.text.style.TextAlign
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
 * Visual de mesa: cada línea del carrito es una tarjeta gruesa; +/− y el CTA
 * de cobro son ≥56dp (rbTouchTarget vía [RbButton]). Feria habla de mesa de
 * puesto, no de caja de mall.
 */
@Composable
fun PasoPago(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val feria = esFeria()
    val carritoVacio = vm.carrito.items.isEmpty()

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = copyTituloCarrito(feria)) {
            if (carritoVacio) {
                // Honestidad: sin líneas no hay total que inventar. El empty
                // manda a sumar cosas; el CTA queda deshabilitado por el VM.
                RbEmptyState(
                    title = copyCarritoVacioTitulo(feria),
                    hint = copyCarritoVacioPista(feria),
                )
            } else {
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
                            // Con el cobro en vuelo el +/- queda mudo: tocarlo cambia
                            // el carrito que el server ya está procesando y, de paso,
                            // invalida la clave de idempotencia justo cuando un
                            // reintento la necesita intacta.
                            habilitado = !vm.cobrando,
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
                            text = copyEtiquetaTotal(feria),
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
                            !vm.hayConexion || vm.carrito.total == null -> Text(
                                text = copyTotalPendiente(
                                    feria = feria,
                                    hayConexion = vm.hayConexion,
                                ),
                                style = RbTheme.typography.body,
                                color = RbTheme.colors.textSecondary,
                            )
                            else -> RbAmount(
                                amount = vm.moneda.formatear(vm.carrito.total!!),
                                emphasis = RbAmountEmphasis.Body,
                            )
                        }
                    },
                )
            }
        }

        RbCard(title = copyComoPaga(feria)) {
            RbChipRow {
                MedioDePago.entries.forEach { opcion ->
                    val bloqueado = vm.motivoParaNoUsar(opcion) != null
                    RbChip(
                        label = copyEtiquetaMedio(opcion, feria),
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
                        //
                        // `vm.cobrando` se suma al mismo candado: cambiar de
                        // medio mientras la venta ya salió hacia el server deja
                        // la pantalla diciendo una cosa y el cobro haciendo
                        // otra, y de paso invalida la clave de idempotencia de
                        // un cobro que todavía no supimos si llegó.
                        onClick = if (bloqueado || vm.cobrando) null else ({ vm.cambiarMedio(opcion) }),
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
                    label = copyLabelMontoEntregado(feria),
                    placeholder = "0",
                    supportingText = copyAyudaVuelto(feria),
                    numeric = true,
                    keyboardType = KeyboardType.Number,
                    // Mismo candado que el medio de pago: el vuelto que calcula
                    // el server es sobre el monto que viajó, no sobre el que la
                    // cajera siga escribiendo mientras el cobro va en camino.
                    enabled = !vm.cobrando,
                )
            }

            if (vm.medio.exigeCliente && vm.hayConexion) {
                SelectorDeCliente(
                    clientes = vm.clientes,
                    elegido = vm.cliente,
                    feria = feria,
                    habilitado = !vm.cobrando,
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
            // Verbo de mesa: Cobrar / Anotar fiado / Anotar venta. Sin señal
            // no se "confirmó" nada: la palabra dice lo que de verdad pasa.
            label = copyCtaPago(
                feria = feria,
                cobrando = vm.cobrando,
                hayConexion = vm.hayConexion,
                medio = vm.medio,
            ),
            onClick = vm::cobrar,
            // Deshabilitado mientras se manda: el primer candado contra el
            // doble toque. El segundo, el que de verdad importa cuando la red
            // se corta, es la clave de idempotencia del ViewModel.
            // Brand fill Primary + fillWidth + ≥56dp (rbTouchTarget).
            enabled = !vm.cobrando && vm.impedimentoParaCobrar() == null,
            fillWidth = true,
        )

        RbButton(
            label = copySeguirAgregando(feria),
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
 * pide no inventar. Los +/− ya miden ≥56dp vía rbTouchTarget; a 200% crecen
 * con la letra. La plata va en [RbReflowRow] para no partir el monto a la mitad.
 */
@Composable
private fun FilaDeCarrito(
    item: ItemCarrito,
    precioUnitario: String,
    subtotal: String?,
    mostrarSubtotal: Boolean,
    habilitado: Boolean,
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
                if (mostrarSubtotal && subtotal != null) {
                    // Solo el monto que mandó el server. Sin subtotal no se
                    // inventa ni se pone "-": el reflow deja el nombre entero.
                    RbAmount(
                        amount = subtotal,
                        emphasis = RbAmountEmphasis.Body,
                    )
                }
            },
        )

        Text(
            text = copyPrecioUnitario(precioUnitario),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )

        // +/− a todo el ancho: en mesa de feria con una mano y sol, un
        // stepper chiquito al borde se pierde. Cada botón ≥56dp (RbButton).
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(dimens.space2),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RbButton(
                label = "−",
                onClick = onMenos,
                variant = RbButtonVariant.Secondary,
                enabled = habilitado,
                modifier = Modifier.weight(1f),
                fillWidth = true,
            )
            Text(
                text = "${item.cantidad}",
                style = RbTheme.typography.title,
                color = colors.textPrimary,
                textAlign = TextAlign.Center,
                modifier = Modifier.weight(1f),
            )
            RbButton(
                label = "+",
                onClick = onMas,
                variant = RbButtonVariant.Secondary,
                enabled = habilitado,
                modifier = Modifier.weight(1f),
                fillWidth = true,
            )
        }
    }
}

@Composable
private fun SelectorDeCliente(
    clientes: List<ClienteDto>,
    elegido: ClienteDto?,
    feria: Boolean,
    habilitado: Boolean,
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
            title = copyClientesVaciosTitulo(feria),
            hint = copyClientesVaciosPista(feria),
        )
        return
    }

    Text(
        text = copyLabelClienteFiado(feria),
        style = RbTheme.typography.label,
        color = RbTheme.colors.textPrimary,
    )
    RbChipRow {
        clientes.forEach { cliente ->
            RbChip(
                label = cliente.name,
                tone = if (elegido?.id == cliente.id) RbChipTone.Brand else RbChipTone.Neutral,
                selected = elegido?.id == cliente.id,
                onClick = if (!habilitado) {
                    null
                } else {
                    { onElegir(if (elegido?.id == cliente.id) null else cliente) }
                },
            )
        }
    }
}
