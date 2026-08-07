package cl.rutbusiness.app.ui.cobrar

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTopBar

@Composable
fun CobrarRoute(sesion: SessionRepository, estado: EstadoSesion.Activa) {
    val vm: CobrarViewModel = viewModel(
        key = "cobrar:${estado.baseUrl}",
        factory = viewModelFactory { initializer { CobrarViewModel(sesion) } },
    )
    CobrarScreen(vm)
}

/**
 * Cobrar: la pantalla que la cajera usa doscientas veces al día.
 *
 * Tres pasos en vez de un tablero: buscar, pagar, comprobante. Un POS de
 * escritorio pone búsqueda, carrito y totales a la vista al mismo tiempo, y eso
 * en un teléfono de 720p con la letra al 200% termina en tres columnas de
 * cuarenta caracteres. Un paso a la vez ocupa el ancho completo siempre, y el
 * paso actual nunca deja de caber.
 *
 * El carrito sobrevive los tres pasos y también los errores: si el cobro falla,
 * se vuelve al paso de pago con todo cargado, no a una pantalla vacía.
 */
@Composable
private fun CobrarScreen(vm: CobrarViewModel) {
    // Atrás desde el paso de pago vuelve a buscar, con el carrito intacto. Se
    // registra más adentro que el de `RutBusinessApp`, así que gana: primero se
    // deshace el paso, y recién cuando ya no hay paso que deshacer, atrás
    // cambia de pestaña. Sin esto, atrás en medio de un cobro saltaba al agente
    // y la cajera perdía de vista la venta que estaba cerrando.
    //
    // En Comprobante queda apagado a propósito: la venta ya se cobró y "volver"
    // al paso de pago sugeriría que se puede cobrar de nuevo. La salida de ahí
    // es "Nueva venta", que además limpia la clave de idempotencia.
    BackHandler(enabled = vm.paso == PasoDeCobro.Pago, onBack = vm::volverABuscar)

    // El escáner tapa el paso actual en vez de ser un cuarto paso: el carrito
    // sigue siendo el mismo y salir de la cámara deja la venta donde estaba.
    // Se registra después del anterior, así que atrás cierra primero la cámara.
    BackHandler(enabled = vm.escaneando) {
        if (vm.creandoProducto) vm.cancelarCreacion() else vm.cerrarEscaner()
    }
    if (vm.escaneando) {
        PasoEscaner(vm, modifier = Modifier.fillMaxSize())
        return
    }

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = when (vm.paso) {
                PasoDeCobro.Buscar -> "Cobrar"
                PasoDeCobro.Pago -> "Cómo paga"
                PasoDeCobro.Comprobante -> "Listo"
            },
            subtitle = when (vm.paso) {
                PasoDeCobro.Buscar -> "Busca el producto y agrégalo"
                PasoDeCobro.Pago -> "Revisa lo que lleva y elige el pago"
                PasoDeCobro.Comprobante -> null
            },
            onBack = if (vm.paso == PasoDeCobro.Pago) vm::volverABuscar else null,
            actions = {
                if (vm.paso == PasoDeCobro.Buscar) {
                    RbButton(
                        label = "Salir",
                        onClick = vm::salir,
                        variant = RbButtonVariant.Secondary,
                    )
                }
            },
        )

        when (vm.paso) {
            PasoDeCobro.Buscar -> PasoBuscar(vm, modifier = Modifier.fillMaxSize())
            PasoDeCobro.Pago -> PasoPago(vm, modifier = Modifier.fillMaxSize())
            PasoDeCobro.Comprobante -> PasoComprobante(vm, modifier = Modifier.fillMaxSize())
        }
    }
}
