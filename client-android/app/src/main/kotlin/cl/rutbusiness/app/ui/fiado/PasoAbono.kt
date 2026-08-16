package cl.rutbusiness.app.ui.fiado

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
import cl.rutbusiness.app.ui.caja.tecladoDePlata
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbErrorCopy
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Anotar lo que el cliente está pagando ahora.
 *
 * El campo del monto va arriba de todo, con la deuda actual a la vista para no
 * tener que volver a mirarla: la pregunta del mostrador es "¿cuánto me das?" y
 * la referencia es cuánto debe.
 */
@Composable
fun PasoAbono(vm: FiadoViewModel, modifier: Modifier = Modifier) {
    FormularioDeAbono(
        moneda = vm.moneda,
        deudaActual = vm.cuenta?.balance,
        monto = vm.montoDelAbono,
        onMonto = vm::cambiarMontoDelAbono,
        nota = vm.notaDelAbono,
        onNota = vm::cambiarNotaDelAbono,
        hayCajaAbierta = vm.cajaAbierta != null,
        entraALaCaja = vm.entraALaCaja,
        onEntraALaCaja = vm::cambiarEntraALaCaja,
        esFeria = esFeria(),
        impedimento = vm.impedimentoParaAbonar(),
        error = vm.errorDeAccion,
        guardando = vm.guardando,
        onAnotar = vm::registrarAbono,
        onVolver = vm::volverAlDetalle,
        modifier = modifier,
    )
}

/**
 * El formulario del abono, sin `ViewModel` detrás para poder medirlo.
 *
 * Misma disciplina de layout que [cl.rutbusiness.app.ui.caja.FormularioDeArqueo],
 * y por el mismo motivo: es un campo de plata con teclado numérico. Campo
 * primero, columna que scrollea con `imePadding`, botones a todo el ancho uno
 * debajo del otro.
 *
 * @param deudaActual el saldo que mandó el server, en su texto decimal. Se
 *   muestra como referencia; no se resta nada contra él.
 * @param esFeria pack feria: el abono en efectivo cuenta en la plata del día
 *   (puesto), no habla de cajón. El toggle se ofrece si hay sesión o se puede
 *   abrir el puesto.
 */
@Composable
internal fun FormularioDeAbono(
    moneda: Moneda,
    deudaActual: String?,
    monto: String,
    onMonto: (String) -> Unit,
    nota: String,
    onNota: (String) -> Unit,
    hayCajaAbierta: Boolean,
    entraALaCaja: Boolean,
    onEntraALaCaja: (Boolean) -> Unit,
    impedimento: String?,
    error: RbErrorCopy?,
    guardando: Boolean,
    onAnotar: () -> Unit,
    onVolver: () -> Unit,
    modifier: Modifier = Modifier,
    esFeria: Boolean = false,
) {
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
        RbCard(title = "¿Cuánto te está pagando?") {
            if (deudaActual != null) {
                Text(
                    text = "Debe ${moneda.formatear(deudaActual)}. Puede pagarte todo o una parte.",
                    style = RbTheme.typography.body,
                    color = colors.textSecondary,
                )
            }

            RbTextField(
                value = monto,
                onValueChange = onMonto,
                label = "Plata que te da",
                placeholder = "0",
                numeric = true,
                keyboardType = tecladoDePlata(moneda),
                enabled = !guardando,
            )
        }

        // Retail: sólo con caja abierta hay arqueo que tocar.
        // Feria: el puesto se puede abrir con $0; el toggle va aunque la sesión
        // todavía se esté asegurando (default efectivo = cuenta en el día).
        if (hayCajaAbierta || esFeria) {
            RbCard(title = "¿Cómo te paga?") {
                RbChipRow {
                    RbChip(
                        label = "En efectivo",
                        tone = if (entraALaCaja) RbChipTone.Brand else RbChipTone.Neutral,
                        selected = entraALaCaja,
                        onClick = { onEntraALaCaja(true) },
                    )
                    RbChip(
                        label = "Transferencia",
                        tone = if (!entraALaCaja) RbChipTone.Brand else RbChipTone.Neutral,
                        selected = !entraALaCaja,
                        onClick = { onEntraALaCaja(false) },
                    )
                }
                Text(
                    text = ayudaComoPaga(feria = esFeria, entraALaCaja = entraALaCaja),
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        }

        RbCard(title = "¿Algo que anotar?") {
            RbTextField(
                value = nota,
                onValueChange = onNota,
                label = "Nota del pago (opcional)",
                placeholder = "Quedó de traer el resto el viernes",
                enabled = !guardando,
            )
        }

        error?.let { copy ->
            RbErrorState(
                title = copy.title,
                message = copy.message,
                retryLabel = copy.retryLabel,
                onRetry = if (copy.retryLabel != null) onAnotar else null,
            )
        }

        if (impedimento != null) {
            Text(
                text = impedimento,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        } else {
            Text(
                text = "Vas a anotar que te pagó:",
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
            RbAmount(
                // Lo que la dueña escribió, escrito con la moneda del negocio.
                // No es un cálculo: es el mismo monto que va a viajar.
                amount = moneda.formatear(monto.replace(',', '.')),
            )
        }

        RbButton(
            label = if (guardando) "Anotando..." else "Anotar el pago",
            onClick = onAnotar,
            enabled = !guardando && impedimento == null,
            fillWidth = true,
        )

        RbButton(
            label = "Volver a la cuenta",
            onClick = onVolver,
            variant = RbButtonVariant.Secondary,
            enabled = !guardando,
            fillWidth = true,
        )
    }
}
