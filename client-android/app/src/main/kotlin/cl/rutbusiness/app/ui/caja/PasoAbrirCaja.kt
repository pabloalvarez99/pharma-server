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
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 1: abrir la caja (o el puesto de feria).
 *
 * En retail es lo primero del día y tiene que costar un campo. En feria el
 * puesto se abre solo con $0; si la dueña ve este formulario, el copy habla de
 * puesto y el CTA es «Empezar el día».
 *
 * El campo del monto va **arriba de todo** y la pantalla scrollea con
 * `imePadding`: cuando el teclado numérico sube, el campo enfocado se lleva a la
 * vista y el botón queda alcanzable scrolleando.
 */
@Composable
fun PasoAbrirCaja(vm: CajaViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val feria = vm.esFeria || esFeria()
    val copy = copyAbrirCaja(feria)

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = copy.tituloCard) {
            Text(
                text = copy.ayuda,
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            RbTextField(
                value = vm.montoDeApertura,
                onValueChange = vm::cambiarMontoDeApertura,
                label = copy.etiquetaMonto,
                placeholder = "0",
                supportingText = copy.ayudaMonto,
                numeric = true,
                keyboardType = tecladoDePlata(vm.moneda),
                enabled = !vm.guardando,
            )
        }

        // Elegir caja física sólo tiene sentido con más de una. Con una (o
        // ninguna) no se pregunta: feria casi nunca tiene cajones nombrados.
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

        if (!feria) {
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
            label = if (vm.guardando) copy.ctaGuardando else copy.cta,
            onClick = vm::abrirCaja,
            enabled = !vm.guardando && vm.impedimentoParaAbrir() == null,
            fillWidth = true,
        )
    }
}
