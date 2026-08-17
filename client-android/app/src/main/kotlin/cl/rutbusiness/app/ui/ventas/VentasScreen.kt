package cl.rutbusiness.app.ui.ventas

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
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
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

@Composable
fun VentasRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.Activa,
    onVolver: () -> Unit,
) {
    val vm: VentasViewModel = viewModel(
        key = "ventas:${estado.baseUrl}",
        factory = viewModelFactory { initializer { VentasViewModel(sesion) } },
    )
    VentasScreen(vm = vm, onVolver = onVolver)
}

/**
 * "Lo que vendiste hoy": la lista de ventas del día, más nueva primero, con
 * camino a ver el detalle de una y deshacerla.
 *
 * Regla de plata: acá no se suma ni un total del día — cada monto que se
 * muestra viene tal cual del server.
 */
@Composable
private fun VentasScreen(vm: VentasViewModel, onVolver: () -> Unit) {
    val dimens = RbTheme.dimens
    val feria = esFeria()

    val atras: () -> Unit = when (vm.paso) {
        PasoDeVentas.Confirmando -> vm::cancelarDeshacer
        PasoDeVentas.Detalle -> vm::volverALaLista
        PasoDeVentas.Lista -> onVolver
    }
    BackHandler(onBack = atras)

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = when (vm.paso) {
                PasoDeVentas.Lista -> tituloHistorial(feria)
                PasoDeVentas.Detalle, PasoDeVentas.Confirmando -> tituloDetalleVenta(feria)
            },
            subtitle = when (vm.paso) {
                PasoDeVentas.Lista -> subtituloHistorial(feria)
                PasoDeVentas.Detalle, PasoDeVentas.Confirmando -> subtituloDetalleVenta()
            },
            onBack = atras,
        )

        val fatal = vm.error
        when {
            fatal != null && vm.paso == PasoDeVentas.Lista -> RbErrorState(
                title = tituloErrorHistorial(feria),
                message = fatal.message,
                modifier = Modifier.padding(dimens.space3),
                retryLabel = fatal.retryLabel,
                onRetry = if (fatal.retryLabel != null) vm::cargar else null,
            )

            vm.cargando && vm.ventas.isEmpty() -> Column {
                RbLoadingState(label = if (feria) "Buscando lo que vendiste..." else "Cargando ventas...")
                RbSkeletonLines(lines = 5, modifier = Modifier.padding(dimens.space3))
            }

            else -> when (vm.paso) {
                PasoDeVentas.Lista -> ListaDeVentas(
                    moneda = vm.moneda,
                    ventas = vm.ventas,
                    feria = feria,
                    onElegir = vm::abrirVenta,
                )

                PasoDeVentas.Detalle, PasoDeVentas.Confirmando ->
                    DetalleDeVenta(vm = vm, feria = feria)
            }
        }
    }
}

/**
 * La lista de ventas de hoy, sin `ViewModel` detrás para poder medirla.
 *
 * `LazyColumn`: un día bueno de feria son decenas de ventas, y el piso de
 * hardware presupuesta 1-2 GB de RAM para todo el aparato.
 */
@Composable
internal fun ListaDeVentas(
    moneda: Moneda,
    ventas: List<VentaDto>,
    feria: Boolean,
    onElegir: (VentaDto) -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    if (ventas.isEmpty()) {
        Box(
            modifier = modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(dimens.space3),
            contentAlignment = Alignment.Center,
        ) {
            RbEmptyState(
                title = tituloVacioHistorial(feria),
                icon = RbIcons.historial,
                benefit = beneficioVacioHistorial(feria),
                hint = pistaVacioHistorial(feria),
            )
        }
        return
    }

    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            start = dimens.space3,
            end = dimens.space3,
            top = dimens.space3,
            bottom = dimens.space3,
        ),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        items(items = ventas, key = { it.id }) { venta ->
            FilaDeVenta(
                venta = venta,
                moneda = moneda,
                feria = feria,
                onClick = { onElegir(venta) },
            )
        }
    }
}

/** Una venta del día: hora, monto, cómo pagó, en ese orden de importancia. */
@Composable
internal fun FilaDeVenta(
    venta: VentaDto,
    moneda: Moneda,
    feria: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
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
            .rbClickable(onClick = onClick, role = Role.Button, shape = shape)
            .rbTouchTarget()
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        content = {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
                Text(
                    text = horaCorta(venta.creadaEn),
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                )
                Text(
                    text = etiquetaComoPago(venta.metodoDePago, feria),
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
                // Nunca sólo el color: una venta devuelta lleva texto propio.
                if (venta.estaDevuelta) {
                    RbChip(label = etiquetaVentaDevuelta(feria), tone = RbChipTone.Neutral)
                }
            }
        },
        trailing = {
            RbAmount(amount = moneda.formatear(venta.total), emphasis = RbAmountEmphasis.Body)
        },
    )
}

/**
 * `HH:mm` de un instante ISO-8601. Igual criterio que `fechaCorta` en
 * `FiadoScreen.kt`: se recorta la cadena, sin `java.time` (minSdk 21).
 */
internal fun horaCorta(iso: String): String {
    val tras = iso.substringAfter('T', "")
    if (tras.length < 5) return iso
    return tras.substring(0, 5)
}

@Composable
internal fun BotonDeshacer(feria: Boolean, enabled: Boolean, onClick: () -> Unit) {
    RbButton(
        label = ctaDeshacerVenta(feria),
        onClick = onClick,
        variant = RbButtonVariant.Destructive,
        enabled = enabled,
        fillWidth = true,
    )
}
