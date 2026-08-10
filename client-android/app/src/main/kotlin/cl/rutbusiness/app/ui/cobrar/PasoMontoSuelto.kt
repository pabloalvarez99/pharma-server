package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Cobrar un monto y nada más.
 *
 * "Son $2.000, una bolsa." No hay producto que buscar porque no hay nada
 * cargado, y en un puesto eso no es un caso raro: es la venta de todos los días.
 *
 * Una sola cosa en pantalla, y por una razón medida: con el teclado numérico
 * abierto quedan ~320dp de panel, y al 200% de escala un campo con su rótulo más
 * un botón ya es todo lo que entra. Cualquier cosa que se agregue acá empuja el
 * botón fuera de la pantalla justo cuando el cliente está esperando el vuelto.
 */
@Composable
fun PasoMontoSuelto(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    MontoSueltoContenido(
        modifier = modifier,
        monto = vm.montoSuelto,
        onMonto = vm::cambiarMontoSuelto,
        simbolo = vm.moneda.simbolo,
        error = vm.errorMontoSuelto,
        preparando = vm.preparandoMontoSuelto,
        onConfirmar = vm::confirmarMontoSuelto,
        onCancelar = vm::cancelarMontoSuelto,
    )
}

/**
 * El paso, sin `ViewModel`.
 *
 * Igual que [BuscarContenido]: todo lo que decide el layout entra por parámetro
 * para poder medirlo al 200% en un panel recortado por el teclado sin montar red
 * ni sesión.
 */
@Composable
internal fun MontoSueltoContenido(
    monto: String,
    onMonto: (String) -> Unit,
    simbolo: String,
    error: String?,
    preparando: Boolean,
    onConfirmar: () -> Unit,
    onCancelar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbTextField(
            value = monto,
            onValueChange = onMonto,
            label = "¿Cuánto le cobras?",
            placeholder = "0",
            supportingText = "En $simbolo. No hace falta que tengas nada cargado.",
            errorMessage = error,
            enabled = !preparando,
            numeric = true,
            keyboardType = KeyboardType.Number,
            // La tecla del teclado cobra, sin bajar la vista al botón. Nunca es
            // la única forma: el botón está igual, abajo y grande.
            imeAction = ImeAction.Done,
            onImeAction = onConfirmar,
        )

        RbButton(
            label = if (preparando) "Preparando…" else "Cobrar este monto",
            onClick = onConfirmar,
            enabled = !preparando,
            fillWidth = true,
        )

        RbButton(
            label = "Volver",
            onClick = onCancelar,
            variant = RbButtonVariant.Secondary,
            enabled = !preparando,
            fillWidth = true,
        )

        // Qué va a quedar anotado, dicho antes y no después. La venta existe en
        // el sistema como cualquier otra -entra a la caja, al resumen del día y
        // al comprobante-, y esa línea es lo que la dueña va a ver mañana.
        Text(
            text = "Queda anotada como \"Venta suelta\" en tu día y en la caja.",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )
    }
}
