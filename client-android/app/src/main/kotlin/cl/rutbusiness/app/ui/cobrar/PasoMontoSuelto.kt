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
import cl.rutbusiness.app.ui.rubro.esFeria
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
 *
 * Visual: campo calmado, CTA brand fill 56dp, pie feria con "puesto".
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
        feria = esFeria(),
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
    feria: Boolean = false,
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
        // Campo + CTA primario: con teclado numérico y letra al 200% no cabe
        // nada más arriba sin empujar el botón fuera del panel de ~320dp.
        RbTextField(
            value = monto,
            onValueChange = onMonto,
            label = copyLabelMontoSuelto(feria),
            placeholder = "0",
            supportingText = copyAyudaMontoSuelto(feria, simbolo),
            errorMessage = error,
            enabled = !preparando,
            numeric = true,
            keyboardType = KeyboardType.Number,
            // La tecla del teclado cobra, sin bajar la vista al botón. Nunca es
            // la única forma: el botón está igual, abajo y grande.
            imeAction = ImeAction.Done,
            onImeAction = onConfirmar,
        )

        // Brand fill Primary + fillWidth + ≥56dp (rbTouchTarget).
        RbButton(
            label = copyCtaMontoSuelto(feria, preparando),
            onClick = onConfirmar,
            enabled = !preparando,
            fillWidth = true,
        )

        RbButton(
            label = copyVolverMontoSuelto(),
            onClick = onCancelar,
            variant = RbButtonVariant.Secondary,
            enabled = !preparando,
            fillWidth = true,
        )

        // Qué va a quedar anotado, dicho antes y no después. La venta existe en
        // el sistema como cualquier otra -entra al día, a la caja/puesto y al
        // papel-, y esa línea es lo que la dueña va a ver mañana. Feria: "puesto".
        Text(
            text = copyNotaMontoSuelto(feria),
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )
    }
}
