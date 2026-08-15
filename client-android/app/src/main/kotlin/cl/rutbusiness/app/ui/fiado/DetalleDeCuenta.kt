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
import cl.rutbusiness.app.ui.agente.ASI_SE_FIA
import cl.rutbusiness.app.ui.gente.compartirConGente
import cl.rutbusiness.app.ui.gente.mensajeDeuda
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Dinero
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
 * La cuenta corriente de una persona: qué se llevó fiado y qué fue pagando.
 *
 * Arriba lo único que se pregunta en el mostrador — **cuánto debe ahora** — y
 * abajo los movimientos que lo explican, del más nuevo al más viejo, tal como
 * los ordena el server.
 *
 * `LazyColumn` porque una cuenta vieja de un cliente frecuente son cientos de
 * líneas, y montarlas todas es justamente lo que el aparato de referencia no
 * puede pagar.
 */
@Composable
fun DetalleDeCuenta(vm: FiadoViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val cuenta = vm.cuenta

    val compartir = compartirConGente()
    // El mismo texto que dibuja la tarjeta de arriba, y los pesos detrás de ese
    // texto para saber si hay algo que recordar. Se leen una sola vez acá para
    // que el botón y la tarjeta no puedan discrepar.
    val saldoEnPantalla = cuenta?.let { vm.moneda.formatear(it.balance) }
    val pesosQueDebe = cuenta?.let { Dinero.deTextoDeServidor(it.balance)?.unidades }

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        vm.avisoDeAbono?.let { aviso ->
            item("aviso") {
                RbCard {
                    RbChip(label = "Quedó anotado", tone = RbChipTone.Brand)
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

        item("saldo") { SaldoDelCliente(vm) }

        item("cobrar") {
            RbButton(
                label = "Me está pagando",
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
        // dueña ya está mirando el saldo y el nombre, que es exactamente lo que
        // el mensaje necesita. Va acá, pegado al saldo, y no en un menú.
        //
        // Sólo con deuda viva. Mandar "me debes $0" a quien terminó de pagar es
        // cobrarle de nuevo, y esa cuenta se abre igual —es la única forma de
        // ver los movimientos de un cliente que ya se puso al día.
        if (compartir != null && saldoEnPantalla != null && pesosQueDebe != null && pesosQueDebe > 0L) {
            item("compartir") {
                RbButton(
                    label = if (esFeria()) "Mandarle lo que debe" else "Compartir el saldo",
                    onClick = {
                        compartir(
                            mensajeDeuda(
                                nombre = vm.elegido?.name.orEmpty(),
                                monto = saldoEnPantalla,
                            ),
                        )
                    },
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
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
                text = "Lo que se llevó y lo que pagó",
                style = RbTheme.typography.heading,
                color = colors.textPrimary,
            )
        }

        when {
            cuenta == null && vm.cargandoCuenta -> item("cargando") {
                RbLoadingState(label = "Trayendo la cuenta...")
            }

            // Sin la cuenta no hay lista que mostrar, pero dejar el título
            // "Lo que se llevó y lo que pagó" seguido de nada se lee como
            // "esta persona nunca compró", que es una respuesta distinta.
            cuenta == null -> item("sin-cuenta") {
                Text(
                    text = "No alcanzamos a traer los movimientos. No quiere decir que no " +
                        "haya: vuelve a abrir esta cuenta en un momento.",
                    style = RbTheme.typography.body,
                    color = colors.textSecondary,
                )
            }

            cuenta.entries.isEmpty() -> item("sin-movimientos") {
                Text(
                    // Antes decía "Esta cuenta no tiene movimientos todavía" y
                    // se quedaba ahí: informaba en vez de enseñar. Ahora dice de
                    // dónde salen las líneas, que es lo que hace falta saber la
                    // primera vez que se abre una cuenta recién creada.
                    //
                    // En feria no se manda a «Cobrar»: esa pestaña se llama
                    // "Vender" y fiar se hace hablando. Mandar a alguien a una
                    // pestaña que no existe es peor que no decirle nada.
                    text = if (esFeria()) {
                        "Todavía no hay movimientos en esta cuenta. Aparecen solos: cuando le " +
                            "fíes algo — «$ASI_SE_FIA» — y cuando anotes que te pagó."
                    } else {
                        "Todavía no hay movimientos en esta cuenta. Aparecen solos: cuando " +
                            "le fíes una venta desde «Cobrar», y cuando anotes que te pagó."
                    },
                    style = RbTheme.typography.body,
                    color = colors.textSecondary,
                )
            }

            else -> items(items = cuenta.entries, key = { it.id }) { movimiento ->
                FilaDeCuenta(movimiento = movimiento, moneda = vm.moneda)
            }
        }
    }
}

@Composable
private fun SaldoDelCliente(vm: FiadoViewModel) {
    val colors = RbTheme.colors
    val cuenta = vm.cuenta

    RbCard(title = "Debe ahora") {
        if (cuenta == null) {
            Text(
                text = if (vm.cargandoCuenta) {
                    "Trayendo la cuenta..."
                } else {
                    "No pudimos traer la cuenta de esta persona. Toca atrás y vuelve a " +
                        "abrirla; si sigue igual, revisa que el computador del negocio esté " +
                        "encendido."
                },
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
            return@RbCard
        }

        RbAmount(
            amount = vm.moneda.formatear(cuenta.balance),
            emphasis = RbAmountEmphasis.Headline,
        )

        RbDivider()

        LineaDeCuenta("Se llevó fiado en total", vm.moneda.formatear(cuenta.totalFiado))
        LineaDeCuenta("Te ha pagado en total", vm.moneda.formatear(cuenta.totalAbonado))
    }
}

@Composable
private fun LineaDeCuenta(concepto: String, monto: String) {
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
private fun FilaDeCuenta(movimiento: MovimientoDeCuentaDto, moneda: Moneda) {
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
                    text = if (movimiento.esAbono) "Te pagó" else "Se llevó fiado",
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
