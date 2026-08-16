package cl.rutbusiness.app.ui.caja

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 2: la caja / el puesto está abierto.
 *
 * Contesta una sola pregunta arriba de todo — **cuánta plata debería haber
 * ahora** — y debajo muestra de dónde salió ese número. Ninguno de los cinco
 * montos se suma en el teléfono: el server los manda ya resueltos por
 * `compute_summary`, y el desglose está justamente para que el número grande no
 * parezca sacado de la nada.
 *
 * Visual de puesto: chip de estado, cifra hero con aire, secciones con
 * `space4` (se leen de a una, no como grilla de KPI).
 *
 * `LazyColumn` y no una columna que scrollea: los movimientos de un día ajetreado
 * son decenas y el piso de hardware no tolera montarlos todos a la vez.
 */
@Composable
fun PasoCajaAbierta(vm: CajaViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val arqueo = vm.arqueo
    val feria = vm.esFeria || esFeria()
    val copy = copyCajaAbierta(feria)

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        // space4 entre tarjetas: se leen de a una, no como grilla.
        contentPadding = PaddingValues(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space4),
    ) {
        item("estado") {
            RbChip(
                label = copy.chipEstado,
                tone = RbChipTone.Brand,
            )
        }

        item("deberia-haber") { DeberiaHaber(vm, copy, feria) }

        item("desglose") {
            if (arqueo != null) Desglose(moneda = vm.moneda, arqueo = arqueo)
        }

        item("acciones") { Acciones(vm, copy) }

        item("titulo-movimientos") {
            Text(
                text = copy.tituloMovimientos,
                style = RbTheme.typography.heading,
                color = RbTheme.colors.textPrimary,
            )
        }

        if (vm.movimientos.isEmpty()) {
            item("sin-movimientos") {
                Text(
                    text = copy.vacioMovimientos,
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
private fun DeberiaHaber(vm: CajaViewModel, copy: CopyCajaAbierta, feria: Boolean) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val esperado = vm.arqueo?.session?.esperado

    // Misma lectura que TarjetaDelDia: etiqueta callada + cifra hero con aire.
    RbCard {
        Text(
            text = copy.tituloEsperado,
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )

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
                modifier = Modifier.padding(top = dimens.space2),
            )
            return@RbCard
        }

        RbAmount(
            amount = vm.moneda.formatear(esperado),
            emphasis = RbAmountEmphasis.Headline,
            modifier = Modifier.padding(vertical = dimens.space2),
        )
        Text(
            text = copyEsperadoEnPuesto(feria),
            style = RbTheme.typography.body,
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
private fun Acciones(vm: CajaViewModel, copy: CopyCajaAbierta) {
    val dimens = RbTheme.dimens

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
            label = copy.ctaSacar,
            onClick = vm::irASacarPlata,
            variant = RbButtonVariant.Secondary,
            enabled = !vm.guardando,
            fillWidth = true,
        )
        RbButton(
            label = copy.ctaMeter,
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
