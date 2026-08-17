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
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 3: sacar o meter plata del puesto / cajón, con motivo.
 *
 * El motivo es obligatorio y no por burocracia: el server lo exige
 * (`add_movement` en `crates/domain/src/cash_register/service.rs` rechaza
 * `reason` vacío), y en el cierre un retiro sin motivo se ve exactamente igual
 * que plata que desapareció. El problema real no es que sea obligatorio: es
 * que escribirlo a mano cuesta con las manos ocupadas y un teléfono viejo, y
 * por eso [motivosDeUnToque] pone las razones más comunes de feria a un toque
 * -llenan el campo de arriba, se pueden editar después, y tocar la misma de
 * nuevo la vacía-. Reusa `RbChip`/`RbChipRow`, mismo patrón que las unidades
 * sugeridas de `CatalogoScreen.kt`.
 *
 * Los dos campos van arriba y la pantalla scrollea con `imePadding`: con el
 * teclado numérico arriba, el campo del monto sigue a la vista y el botón se
 * alcanza scrolleando.
 */
@Composable
fun PasoMovimiento(vm: CajaViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val esRetiro = vm.tipoDeMovimiento == NuevoMovimiento.RETIRO
    val feria = vm.esFeria || esFeria()
    val copy = copyMovimientoCaja(feria = feria, esRetiro = esRetiro)
    val motivo = copyMotivoMovimiento(feria = feria, esRetiro = esRetiro)
    val sugerencias = motivosDeUnToque(feria = feria, esRetiro = esRetiro)

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space4),
    ) {
        RbCard(title = copy.tituloCard) {
            Text(
                text = copy.ayuda,
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            RbTextField(
                value = vm.montoDelMovimiento,
                onValueChange = vm::cambiarMontoDelMovimiento,
                label = copy.etiquetaMonto,
                placeholder = "0",
                numeric = true,
                keyboardType = tecladoDePlata(vm.moneda),
                enabled = !vm.guardando,
            )
        }

        RbCard(title = motivo.tituloCard) {
            RbTextField(
                value = vm.motivoDelMovimiento,
                onValueChange = vm::cambiarMotivoDelMovimiento,
                label = motivo.etiqueta,
                placeholder = motivo.placeholder,
                supportingText = motivo.ayuda,
                enabled = !vm.guardando,
            )

            // Un toque llena el campo de arriba con la razón más común; el
            // campo manda -se puede seguir editando después del toque, y
            // tocar la misma chip de nuevo la vacía en vez de dejarla pegada-.
            // Nada de esto pisa lo que la dueña ya escribió a mano: sólo
            // cambia si ella toca una chip.
            if (sugerencias.isNotEmpty()) {
                RbChipRow {
                    sugerencias.forEach { sugerida ->
                        val elegida = vm.motivoDelMovimiento.equals(sugerida, ignoreCase = true)
                        RbChip(
                            label = sugerida,
                            tone = if (elegida) RbChipTone.Brand else RbChipTone.Neutral,
                            selected = elegida,
                            onClick = if (vm.guardando) {
                                null
                            } else {
                                { vm.cambiarMotivoDelMovimiento(if (elegida) "" else sugerida) }
                            },
                        )
                    }
                }
            }
        }

        vm.errorDeAccion?.let { error ->
            RbErrorState(
                title = error.title,
                message = error.message,
                retryLabel = error.retryLabel,
                onRetry = if (error.retryLabel != null) vm::guardarMovimiento else null,
            )
        }

        vm.impedimentoParaMover()?.let { motivo ->
            Text(
                text = motivo,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        }

        RbButton(
            label = if (vm.guardando) copy.ctaGuardando else copy.cta,
            onClick = vm::guardarMovimiento,
            enabled = !vm.guardando && vm.impedimentoParaMover() == null,
            fillWidth = true,
        )

        RbButton(
            label = copy.ctaVolver,
            onClick = vm::volverAlEstado,
            variant = RbButtonVariant.Secondary,
            enabled = !vm.guardando,
            fillWidth = true,
        )
    }
}
