package cl.rutbusiness.app.ui.fiado

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import cl.rutbusiness.app.ui.gente.RecadoParaGente
import cl.rutbusiness.app.ui.gente.etiquetaCompartirDeuda
import cl.rutbusiness.app.ui.gente.mensajeDeuda
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Lo que una persona te debe: qué se llevó y qué fue pagando.
 *
 * Arriba lo único que se pregunta en el mostrador — **cuánto debe ahora** — y
 * abajo las líneas que lo explican, del más nuevo al más viejo, tal como
 * las ordena el server. En feria se lee «Te debe $X», no «saldo de cuenta».
 *
 * `LazyColumn` porque una persona frecuente son cientos de líneas, y montarlas
 * todas es justamente lo que el aparato de referencia no puede pagar.
 */
@Composable
fun DetalleDeCuenta(vm: FiadoViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val feria = esFeria()
    val cuenta = vm.cuenta

    // El mismo texto que dibuja la tarjeta de arriba, y los pesos detrás de ese
    // texto para saber si hay algo que recordar. Se leen una sola vez acá para
    // que el recado y la tarjeta no puedan discrepar.
    val montoEnPantalla = cuenta?.let { vm.moneda.formatear(it.balance) }
    val pesosQueDebe = cuenta?.let { Dinero.deTextoDeServidor(it.balance)?.unidades }

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        vm.avisoDeAbono?.let { aviso ->
            item("aviso") {
                RbCard {
                    RbChip(label = chipQuedoAnotado(), tone = RbChipTone.Brand)
                    Text(
                        text = aviso,
                        style = RbTheme.typography.body,
                        color = colors.textPrimary,
                    )
                }
            }
        }

        vm.errorDeAccion?.let { error ->
            item("error") {
                RbErrorState(
                    title = error.title,
                    message = error.message,
                    retryLabel = error.retryLabel,
                    onRetry = null,
                )
            }
        }

        item("debe") { DebeAhora(vm, feria = feria) }

        item("cobrar") {
            RbButton(
                label = ctaMeEstaPagando(),
                onClick = vm::irAlAbono,
                // Sin conexión el botón queda apagado, y el motivo va debajo.
                // Dejarlo tocable para que el formulario falle al final sería
                // hacerle escribir el monto a alguien que no lo va a poder
                // guardar, con el cliente esperando.
                enabled = !vm.guardando && cuenta != null && vm.motivoParaNoAbonar == null,
                fillWidth = true,
            )
        }

        // Recordarle a alguien lo que debe es la otra mitad de esta pantalla: la
        // dueña ya está mirando el monto y el nombre, que es exactamente lo que
        // el recado necesita. Va acá, pegado al monto, y no en un menú.
        //
        // Sólo con deuda viva. Mandar "me debes $0" a quien terminó de pagar es
        // cobrarle de nuevo, y esta persona se abre igual —es la única forma de
        // ver lo que se llevó y pagó alguien que ya se puso al día. Sin puerto el
        // recado no se dibuja.
        if (montoEnPantalla != null && pesosQueDebe != null && pesosQueDebe > 0L) {
            item("compartir") {
                RecadoParaGente(
                    mensaje = mensajeDeuda(
                        nombre = vm.elegido?.name.orEmpty(),
                        monto = montoEnPantalla,
                    ),
                    etiqueta = etiquetaCompartirDeuda(feria),
                )
            }
        }

        vm.motivoParaNoAbonar?.let { motivo ->
            item("sin-abono") {
                Text(
                    text = motivo,
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        }

        item("titulo-movimientos") {
            Text(
                text = tituloMovimientos(),
                style = RbTheme.typography.heading,
                color = colors.textPrimary,
            )
        }

        when {
            cuenta == null && vm.cargandoCuenta -> item("cargando") {
                RbLoadingState(label = cargandoDetalle(feria))
            }

            // Sin detalle no hay lista que mostrar, pero dejar el título
            // seguido de nada se lee como "esta persona nunca compró", que es
            // una respuesta distinta.
            cuenta == null -> item("sin-cuenta") {
                Text(
                    text = sinMovimientosCargados(feria),
                    style = RbTheme.typography.body,
                    color = colors.textSecondary,
                )
            }

            cuenta.entries.isEmpty() -> item("sin-movimientos") {
                Text(
                    // En feria no se manda a «Cobrar»: esa pestaña se llama
                    // "Vender" y fiar se hace hablando. Mandar a alguien a una
                    // pestaña que no existe es peor que no decirle nada.
                    text = vacioMovimientos(feria),
                    style = RbTheme.typography.body,
                    color = colors.textSecondary,
                )
            }

            else -> items(items = cuenta.entries, key = { it.id }) { movimiento ->
                FilaDeMovimiento(
                    movimiento = movimiento,
                    moneda = vm.moneda,
                    feria = feria,
                )
            }
        }
    }
}

@Composable
private fun DebeAhora(vm: FiadoViewModel, feria: Boolean) {
    val colors = RbTheme.colors
    val cuenta = vm.cuenta

    RbCard(title = tituloDebeAhora(feria)) {
        if (cuenta == null) {
            Text(
                text = if (vm.cargandoCuenta) {
                    cargandoDetalle(feria)
                } else {
                    errorSinCuenta(feria = feria)
                },
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
            return@RbCard
        }

        // El monto viene del server (`cuenta.balance`); acá no se calcula.
        RbAmount(
            amount = vm.moneda.formatear(cuenta.balance),
            emphasis = RbAmountEmphasis.Headline,
        )

        RbDivider()

        LineaDeResumen(totalSeLlevo(), vm.moneda.formatear(cuenta.totalFiado))
        LineaDeResumen(totalTePago(), vm.moneda.formatear(cuenta.totalAbonado))
    }
}

@Composable
private fun LineaDeResumen(concepto: String, monto: String) {
    RbReflowRow(
        spacing = RbTheme.dimens.space2,
        modifier = Modifier.fillMaxWidth(),
        content = {
            Text(
                text = concepto,
                style = RbTheme.typography.body,
                color = RbTheme.colors.textSecondary,
            )
        },
        trailing = { RbAmount(amount = monto) },
    )
}

@Composable
private fun FilaDeMovimiento(
    movimiento: MovimientoDeCuentaDto,
    moneda: Moneda,
    feria: Boolean,
) {
    val colors = RbTheme.colors

    RbCard {
        RbReflowRow(
            spacing = RbTheme.dimens.space2,
            modifier = Modifier.fillMaxWidth(),
            content = {
                Text(
                    // La palabra dice el signo. Nada distingue sólo por color:
                    // quien no ve la diferencia entre el verde y el gris lee
                    // exactamente lo mismo.
                    text = etiquetaMovimiento(
                        esAbono = movimiento.esAbono,
                        feria = feria,
                    ),
                    style = RbTheme.typography.bodyStrong,
                    color = colors.textPrimary,
                )
            },
            trailing = { RbAmount(amount = moneda.formatear(movimiento.amount)) },
        )

        val detalle = listOfNotNull(
            fechaCorta(movimiento.creadoEn),
            movimiento.note?.takeIf { it.isNotBlank() },
        ).joinToString(" · ")

        if (detalle.isNotBlank()) {
            Text(
                text = detalle,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }
    }
}
