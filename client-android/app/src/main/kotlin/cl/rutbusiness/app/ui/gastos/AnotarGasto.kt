package cl.rutbusiness.app.ui.gastos

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import cl.rutbusiness.app.ui.caja.tecladoDePlata
import cl.rutbusiness.core.money.Moneda
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
 * Anotar un gasto: cuánto, en qué, y con qué pagó. Tres campos, en la misma
 * pantalla — no un `AlertDialog` — igual que `PasoAbono.kt` en fiado y el
 * paso de confirmación de `DetalleDeVenta.kt` en ventas. Proveedor, nota y
 * fecha son opcionales del server y no van en este camino rápido.
 *
 * El foco arranca en el monto y el teclado es numérico: es lo primero que la
 * dueña necesita escribir.
 */
@Composable
fun AnotarGasto(
    vm: GastosViewModel,
    feria: Boolean,
    onGuardado: () -> Unit,
    onCancelar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val focoDelMonto = FocusRequester()

    LaunchedEffect(vm.avisoGuardado) {
        if (vm.avisoGuardado != null) onGuardado()
    }

    LaunchedEffect(Unit) {
        focoDelMonto.requestFocus()
    }

    FormularioDeGasto(
        moneda = vm.moneda,
        monto = vm.montoNuevo,
        onMonto = vm::cambiarMontoNuevo,
        categoria = vm.categoriaNueva,
        onCategoria = vm::cambiarCategoriaNueva,
        descripcion = vm.descripcionNueva,
        onDescripcion = vm::cambiarDescripcionNueva,
        metodoDePago = vm.metodoDePagoNuevo,
        onMetodoDePago = vm::cambiarMetodoDePagoNuevo,
        impedimento = vm.impedimentoParaAnotar(),
        error = vm.errorDeGuardado,
        guardando = vm.guardando,
        feria = feria,
        focoDelMonto = focoDelMonto,
        onAnotar = { vm.anotar(feria) },
        onCancelar = onCancelar,
        modifier = modifier,
    )
}

/** El formulario, sin `ViewModel` detrás para poder medirlo. */
@Composable
internal fun FormularioDeGasto(
    moneda: Moneda,
    monto: String,
    onMonto: (String) -> Unit,
    categoria: String,
    onCategoria: (String) -> Unit,
    descripcion: String,
    onDescripcion: (String) -> Unit,
    metodoDePago: String,
    onMetodoDePago: (String) -> Unit,
    impedimento: String?,
    error: RbErrorCopy?,
    guardando: Boolean,
    feria: Boolean,
    onAnotar: () -> Unit,
    onCancelar: () -> Unit,
    modifier: Modifier = Modifier,
    focoDelMonto: FocusRequester? = null,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        RbCard(title = CopyGastos.labelMonto()) {
            val campoMonto = if (focoDelMonto != null) {
                Modifier.focusRequester(focoDelMonto)
            } else {
                Modifier
            }
            RbTextField(
                value = monto,
                onValueChange = onMonto,
                label = CopyGastos.labelMonto(),
                placeholder = "0",
                numeric = true,
                keyboardType = tecladoDePlata(moneda),
                enabled = !guardando,
                modifier = campoMonto,
            )
        }

        RbCard(title = CopyGastos.labelCategoria(feria)) {
            RbChipRow {
                for ((etiqueta, valor) in CopyGastos.CATEGORIAS_SUGERIDAS) {
                    RbChip(
                        label = etiqueta,
                        tone = if (categoria == valor) RbChipTone.Brand else RbChipTone.Neutral,
                        selected = categoria == valor,
                        onClick = { onCategoria(valor) },
                    )
                }
            }
            RbTextField(
                value = categoria,
                onValueChange = onCategoria,
                label = CopyGastos.labelCategoria(feria),
                placeholder = CopyGastos.placeholderCategoria(),
                enabled = !guardando,
            )
        }

        RbCard(title = CopyGastos.labelDescripcion()) {
            RbTextField(
                value = descripcion,
                onValueChange = onDescripcion,
                label = CopyGastos.labelDescripcion(),
                placeholder = CopyGastos.placeholderDescripcion(feria),
                enabled = !guardando,
            )
        }

        RbCard(title = CopyGastos.tituloComoPagaste()) {
            RbChipRow {
                RbChip(
                    label = CopyGastos.chipEfectivo(),
                    tone = if (metodoDePago == "cash") RbChipTone.Brand else RbChipTone.Neutral,
                    selected = metodoDePago == "cash",
                    onClick = { onMetodoDePago("cash") },
                )
                RbChip(
                    label = CopyGastos.chipTransferencia(),
                    tone = if (metodoDePago == "transfer") RbChipTone.Brand else RbChipTone.Neutral,
                    selected = metodoDePago == "transfer",
                    onClick = { onMetodoDePago("transfer") },
                )
                RbChip(
                    label = CopyGastos.chipTarjeta(),
                    tone = if (metodoDePago == "card") RbChipTone.Brand else RbChipTone.Neutral,
                    selected = metodoDePago == "card",
                    onClick = { onMetodoDePago("card") },
                )
            }
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
        }

        RbButton(
            label = if (guardando) CopyGastos.ctaAnotando(feria) else CopyGastos.ctaAnotar(feria),
            onClick = onAnotar,
            enabled = !guardando && impedimento == null,
            fillWidth = true,
        )

        RbButton(
            label = CopyGastos.botonCancelar(),
            onClick = onCancelar,
            variant = RbButtonVariant.Secondary,
            enabled = !guardando,
            fillWidth = true,
        )
    }
}
