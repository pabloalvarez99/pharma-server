package cl.rutbusiness.app.ui.caja

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 2: la caja está abierta.
 *
 * Contesta una sola pregunta arriba de todo — **cuánta plata debería haber
 * ahora** — y debajo muestra de dónde salió ese número. Ninguno de los cinco
 * montos se suma en el teléfono: el server los manda ya resueltos por
 * `compute_summary`, y el desglose está justamente para que el número grande no
 * parezca sacado de la nada.
 *
 * `LazyColumn` y no una columna que scrollea: los movimientos de un día ajetreado
 * son decenas y el piso de hardware no tolera montarlos todos a la vez.
 */
@Composable
fun PasoCajaAbierta(vm: CajaViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val arqueo = vm.arqueo

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        item("deberia-haber") { DeberiaHaber(vm) }

        item("desglose") {
            if (arqueo != null) Desglose(moneda = vm.moneda, arqueo = arqueo)
        }

        item("acciones") { Acciones(vm) }

        item("titulo-movimientos") {
            Text(
                text = "Lo que entró y salió a mano",
                style = RbTheme.typography.heading,
                color = RbTheme.colors.textPrimary,
            )
        }

        if (vm.movimientos.isEmpty()) {
            item("sin-movimientos") {
                Text(
                    text = "Todavía no sacaste ni metiste plata a mano. Las ventas en efectivo " +
                        "no aparecen acá: entran solas y ya están contadas arriba.",
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textSecondary,
                )
            }
        } else {
            items(items = vm.movimientos, key = { it.id }) { movimiento ->
                FilaDeMovimiento(movimiento = movimiento, moneda = vm.moneda)
            }
        }
    }
}

@Composable
private fun DeberiaHaber(vm: CajaViewModel) {
    val colors = RbTheme.colors
    val esperado = vm.arqueo?.session?.esperado
    val feria = vm.esFeria || esFeria()
    val copy = copyCajaAbierta(feria)

    RbCard(title = copy.tituloEsperado) {
        if (esperado == null) {
            Text(
                text = if (feria) {
                    "No pudimos traer la cuenta del puesto. El día sigue abierto y puedes " +
                        "seguir vendiendo; toca «Actualizar» arriba para volver a pedirla."
                } else {
                    "No pudimos traer la cuenta de la caja. La caja sigue abierta y puedes " +
                        "seguir vendiendo; toca «Actualizar» arriba para volver a pedirla."
                },
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )
            return@RbCard
        }

        RbAmount(
            amount = vm.moneda.formatear(esperado),
            emphasis = RbAmountEmphasis.Headline,
        )
        Text(
            text = copyEsperadoEnPuesto(feria),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )
    }
}

@Composable
private fun Desglose(moneda: Moneda, arqueo: ArqueoDeCajaDto) {
    RbCard(title = "De dónde sale") {
        LineaDeDesglose("Con lo que abriste", moneda.formatear(arqueo.session.apertura))
        RbDivider()
        LineaDeDesglose("Vendido en efectivo", moneda.formatear(arqueo.ventasEnEfectivo))
        RbDivider()
        LineaDeDesglose("Metiste a mano", moneda.formatear(arqueo.entradas))
        RbDivider()
        LineaDeDesglose("Sacaste a mano", moneda.formatear(arqueo.salidas))
    }
}

/**
 * Una línea del desglose.
 *
 * [RbReflowRow] y no una `Row` con pesos: al 200% "Vendido en efectivo" y un
 * monto de siete dígitos no comparten renglón, y con pesos el texto se parte
 * letra por letra. Acá el monto baja a su propia línea y las dos cosas se leen.
 */
@Composable
private fun LineaDeDesglose(concepto: String, monto: String) {
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
private fun Acciones(vm: CajaViewModel) {
    val dimens = RbTheme.dimens
    val feria = vm.esFeria || esFeria()
    val copy = copyCajaAbierta(feria)

    Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
        vm.errorDeAccion?.let { error ->
            RbErrorState(
                title = error.title,
                message = error.message,
                retryLabel = error.retryLabel,
                onRetry = if (error.retryLabel != null) vm::cargar else null,
            )
        }

        // Uno debajo del otro y a todo el ancho: tres botones en fila quedan de
        // 100dp cada uno en un panel de 360, y al 200% sus etiquetas se cortan.
        RbButton(
            label = "Sacar plata",
            onClick = vm::irASacarPlata,
            variant = RbButtonVariant.Secondary,
            enabled = !vm.guardando,
            fillWidth = true,
        )
        RbButton(
            label = "Meter plata",
            onClick = vm::irAMeterPlata,
            variant = RbButtonVariant.Secondary,
            enabled = !vm.guardando,
            fillWidth = true,
        )
        RbButton(
            label = copy.ctaCerrar,
            onClick = vm::irAlArqueo,
            enabled = !vm.guardando,
            fillWidth = true,
        )
    }
}

@Composable
private fun FilaDeMovimiento(movimiento: MovimientoDeCajaDto, moneda: Moneda) {
    val colors = RbTheme.colors

    RbCard {
        RbReflowRow(
            spacing = RbTheme.dimens.space2,
            modifier = Modifier.fillMaxWidth(),
            content = {
                Text(
                    // La palabra dice el signo, no el color: quien no distingue
                    // el rojo del verde lee exactamente lo mismo.
                    text = if (movimiento.esRetiro) "Sacaste" else "Metiste",
                    style = RbTheme.typography.bodyStrong,
                    color = colors.textPrimary,
                )
            },
            trailing = { RbAmount(amount = moneda.formatear(movimiento.amount)) },
        )
        Text(
            text = movimiento.reason,
            style = RbTheme.typography.body,
            color = colors.textSecondary,
        )
    }
}
