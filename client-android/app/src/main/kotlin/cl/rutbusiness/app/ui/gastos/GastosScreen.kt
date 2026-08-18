package cl.rutbusiness.app.ui.gastos

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.icons.RbIcons
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbTouchTarget

@Composable
fun GastosRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.Activa,
    onVolver: () -> Unit,
) {
    val vm: GastosViewModel = viewModel(
        key = "gastos:${estado.baseUrl}",
        factory = viewModelFactory { initializer { GastosViewModel(sesion) } },
    )
    GastosScreen(vm = vm, onVolver = onVolver)
}

/**
 * "Lo que gastaste": la plata que sale del puesto, hoy o este mes, con camino
 * a anotar un gasto nuevo en menos de diez segundos.
 *
 * Regla de plata: acá no se suma ningún total del período — cada monto que se
 * muestra viene tal cual del server.
 */
@Composable
private fun GastosScreen(vm: GastosViewModel, onVolver: () -> Unit) {
    val dimens = RbTheme.dimens
    val feria = esFeria()

    var anotando by rememberSaveable { mutableStateOf(false) }

    val atras: () -> Unit = if (anotando) {
        { anotando = false }
    } else {
        onVolver
    }
    BackHandler(onBack = atras)

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = if (anotando) CopyGastos.tituloFormulario(feria) else CopyGastos.titulo(feria),
            subtitle = if (anotando) null else CopyGastos.subtitulo(feria),
            onBack = atras,
        )

        if (anotando) {
            AnotarGasto(
                vm = vm,
                feria = feria,
                onGuardado = { anotando = false },
                onCancelar = { anotando = false },
            )
            return@Column
        }

        val fatal = vm.error
        when {
            fatal != null && vm.gastos.isEmpty() -> RbErrorState(
                title = CopyGastos.tituloError(feria),
                message = fatal.message,
                modifier = Modifier.padding(dimens.space3),
                retryLabel = fatal.retryLabel,
                onRetry = if (fatal.retryLabel != null) vm::cargar else null,
            )

            vm.cargando && vm.gastos.isEmpty() -> Column {
                RbLoadingState(label = if (feria) "Buscando lo que gastaste..." else "Cargando gastos...")
                RbSkeletonLines(lines = 5, modifier = Modifier.padding(dimens.space3))
            }

            else -> Column(modifier = Modifier.weight(1f).fillMaxSize()) {
                SelectorDeRango(rango = vm.rango, onCambiar = vm::cambiarRango)

                if (fatal != null) {
                    RbErrorState(
                        title = CopyGastos.tituloError(feria),
                        message = fatal.message,
                        modifier = Modifier.padding(horizontal = dimens.space3, vertical = dimens.space2),
                        retryLabel = fatal.retryLabel,
                        onRetry = if (fatal.retryLabel != null) vm::cargar else null,
                    )
                }

                ListaDeGastos(
                    moneda = vm.moneda,
                    gastos = vm.gastos,
                    feria = feria,
                    modifier = Modifier.weight(1f),
                )

                Column(modifier = Modifier.padding(dimens.space3)) {
                    RbButton(
                        label = CopyGastos.ctaAnotar(feria),
                        onClick = {
                            vm.abrirFormulario()
                            anotando = true
                        },
                        fillWidth = true,
                    )
                }
            }
        }
    }
}

@Composable
private fun SelectorDeRango(rango: RangoDeGastos, onCambiar: (RangoDeGastos) -> Unit) {
    val dimens = RbTheme.dimens
    Row(
        modifier = Modifier.fillMaxWidth().padding(dimens.space3),
    ) {
        RbChip(
            label = CopyGastos.chipHoy(),
            tone = if (rango == RangoDeGastos.Hoy) RbChipTone.Brand else RbChipTone.Neutral,
            selected = rango == RangoDeGastos.Hoy,
            onClick = { onCambiar(RangoDeGastos.Hoy) },
            modifier = Modifier.padding(end = dimens.space2),
        )
        RbChip(
            label = CopyGastos.chipEsteMes(),
            tone = if (rango == RangoDeGastos.EsteMes) RbChipTone.Brand else RbChipTone.Neutral,
            selected = rango == RangoDeGastos.EsteMes,
            onClick = { onCambiar(RangoDeGastos.EsteMes) },
        )
    }
}

/** La lista de gastos, agrupada por día cuando el rango es el mes. */
@Composable
internal fun ListaDeGastos(
    moneda: Moneda,
    gastos: List<GastoDto>,
    feria: Boolean,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    if (gastos.isEmpty()) {
        Box(
            modifier = modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(dimens.space3),
            contentAlignment = Alignment.Center,
        ) {
            RbEmptyState(
                title = CopyGastos.tituloVacio(feria),
                icon = RbIcons.dineroContorno,
                benefit = CopyGastos.beneficioVacio(feria),
                hint = CopyGastos.pistaVacio(feria),
            )
        }
        return
    }

    val grupos = remember(gastos) {
        gastos.groupBy { it.incurredAt.substringBefore('T') }
            .toList()
            .sortedByDescending { it.first }
    }

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            start = dimens.space3,
            end = dimens.space3,
            bottom = dimens.space3,
        ),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        grupos.forEach { (fecha, deEseDia) ->
            item(key = "encabezado-$fecha") {
                Text(
                    text = fechaAgrupada(fecha),
                    style = RbTheme.typography.support,
                    color = RbTheme.colors.textSecondary,
                )
            }
            items(items = deEseDia, key = { it.id }) { gasto ->
                FilaDeGasto(gasto = gasto, moneda = moneda)
            }
        }
    }
}

/** Descripción + monto + cuándo, en ese orden de importancia. */
@Composable
internal fun FilaDeGasto(gasto: GastoDto, moneda: Moneda, modifier: Modifier = Modifier) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    RbReflowRow(
        spacing = dimens.space3,
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surface)
            .border(dimens.border, colors.outline, shape)
            .rbTouchTarget()
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        content = {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
                Text(
                    text = gasto.description.ifBlank { gasto.category },
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                )
                Text(
                    text = horaCortaDeGasto(gasto.incurredAt),
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        },
        trailing = {
            RbAmount(amount = moneda.formatear(gasto.amount))
        },
    )
}

/** `HH:mm` de un instante ISO-8601, mismo criterio que `horaCorta` en `VentasScreen.kt`. */
internal fun horaCortaDeGasto(iso: String): String {
    val tras = iso.substringAfter('T', "")
    if (tras.length < 5) return iso
    return tras.substring(0, 5)
}

/** `dd-mm-aaaa` de una fecha `YYYY-MM-DD`, en el orden en que se lee en Chile. */
internal fun fechaAgrupada(fecha: String): String {
    val partes = fecha.split('-')
    return if (partes.size == 3) "${partes[2]}-${partes[1]}-${partes[0]}" else fecha
}
