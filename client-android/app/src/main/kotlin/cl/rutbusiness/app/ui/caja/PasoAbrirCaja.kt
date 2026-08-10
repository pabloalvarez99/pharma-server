package cl.rutbusiness.app.ui.caja

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 1: abrir la caja.
 *
 * Es lo primero del día y tiene que costar un campo. Todo lo demás — qué caja,
 * qué anotar — es opcional y está debajo, para que quien tiene una sola caja
 * escriba el monto y toque el botón sin leer nada más.
 *
 * El campo del monto va **arriba de todo** y la pantalla scrollea con
 * `imePadding`: cuando el teclado numérico sube, el campo enfocado se lleva a la
 * vista y el botón queda alcanzable scrolleando. Es el error clásico de esta
 * pantalla y el que la arruina entera.
 */
@Composable
fun PasoAbrirCaja(vm: CajaViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = "¿Con cuánta plata partes?") {
            Text(
                text = "Cuenta lo que hay en el cajón para dar vuelto y escríbelo. " +
                    "Desde que abres la caja, todo lo que vendas en efectivo se va sumando " +
                    "solo hasta el cierre.",
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            RbTextField(
                value = vm.montoDeApertura,
                onValueChange = vm::cambiarMontoDeApertura,
                label = "Plata con la que parte el cajón",
                placeholder = "0",
                supportingText = "Si el cajón arranca vacío, escribe 0.",
                numeric = true,
                keyboardType = tecladoDePlata(vm.moneda),
                enabled = !vm.guardando,
            )
        }

        if (vm.cajas.size > 1) {
            RbCard(title = "¿Cuál caja?") {
                Text(
                    text = "El negocio tiene más de una. Elige en cuál estás parada: lo que " +
                        "vendas descuenta del local donde está esa caja.",
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
                RbChipRow {
                    vm.cajas.forEach { caja ->
                        RbChip(
                            label = caja.name,
                            tone = if (vm.cajaElegida?.id == caja.id) {
                                RbChipTone.Brand
                            } else {
                                RbChipTone.Neutral
                            },
                            selected = vm.cajaElegida?.id == caja.id,
                            onClick = { vm.elegirCaja(caja) },
                        )
                    }
                }
            }
        }

        RbCard(title = "¿Algo que anotar?") {
            RbTextField(
                value = vm.notaDeApertura,
                onValueChange = vm::cambiarNotaDeApertura,
                label = "Nota de la apertura (opcional)",
                placeholder = "Por ejemplo: quedaron $5.000 de anoche",
                supportingText = "Se guarda con la caja. Sirve para acordarse en el cierre.",
                enabled = !vm.guardando,
            )
        }

        vm.errorDeAccion?.let { error ->
            RbErrorState(
                title = error.title,
                message = error.message,
                retryLabel = error.retryLabel,
                onRetry = if (error.retryLabel != null) vm::abrirCaja else null,
            )
        }

        vm.impedimentoParaAbrir()?.let { motivo ->
            Text(
                text = motivo,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }

        RbButton(
            label = if (vm.guardando) "Abriendo..." else "Abrir la caja",
            onClick = vm::abrirCaja,
            enabled = !vm.guardando && vm.impedimentoParaAbrir() == null,
            fillWidth = true,
        )
    }
}
