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
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Paso 3: sacar o meter plata del cajón, con motivo.
 *
 * El motivo es obligatorio y no por burocracia: en el cierre, un retiro sin
 * motivo se ve exactamente igual que plata que desapareció. Escribir "pagué el
 * pan" a las once de la mañana es lo que evita una diferencia sin explicación a
 * las nueve de la noche.
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

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = if (esRetiro) "¿Cuánto sacas?" else "¿Cuánto metes?") {
            Text(
                text = if (esRetiro) {
                    "La plata que sacas del cajón para pagar algo o para guardarla. Se descuenta " +
                        "de lo que debería haber al cerrar."
                } else {
                    "La plata que le agregas al cajón: cambio que trajiste, un vuelto que " +
                        "devolvieron. Se suma a lo que debería haber al cerrar."
                },
                style = RbTheme.typography.body,
                color = colors.textSecondary,
            )

            RbTextField(
                value = vm.montoDelMovimiento,
                onValueChange = vm::cambiarMontoDelMovimiento,
                label = if (esRetiro) "Plata que sacas" else "Plata que metes",
                placeholder = "0",
                numeric = true,
                keyboardType = tecladoDePlata(vm.moneda),
                enabled = !vm.guardando,
            )
        }

        RbCard(title = "¿Por qué?") {
            RbTextField(
                value = vm.motivoDelMovimiento,
                onValueChange = vm::cambiarMotivoDelMovimiento,
                label = "Motivo",
                placeholder = if (esRetiro) "Le pagué al del pan" else "Traje cambio de mi casa",
                supportingText = "Con esto vas a saber en el cierre qué pasó con esa plata.",
                enabled = !vm.guardando,
            )
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
            label = when {
                vm.guardando -> "Anotando..."
                esRetiro -> "Anotar que sacaste"
                else -> "Anotar que metiste"
            },
            onClick = vm::guardarMovimiento,
            enabled = !vm.guardando && vm.impedimentoParaMover() == null,
            fillWidth = true,
        )

        RbButton(
            label = "Volver a la caja",
            onClick = vm::volverAlEstado,
            variant = RbButtonVariant.Secondary,
            enabled = !vm.guardando,
            fillWidth = true,
        )
    }
}
