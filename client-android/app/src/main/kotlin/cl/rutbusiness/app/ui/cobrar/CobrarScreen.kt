package cl.rutbusiness.app.ui.cobrar

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.impresora.TarjetaDeReimpresion
import cl.rutbusiness.app.ui.impresora.impresoraViewModel
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme

@Composable
fun CobrarRoute(sesion: SessionRepository, estado: EstadoSesion.Activa) {
    // Los servicios offline se leen acá, en el composable, y se le pasan al
    // `ViewModel` por el constructor: un `ViewModel` no puede leer un
    // `CompositionLocal`, y tampoco debería — es la frontera que mantiene la
    // lógica testeable sin montar Compose.
    val offline = LocalOffline.current
    val vm: CobrarViewModel = viewModel(
        key = "cobrar:${estado.baseUrl}",
        factory = viewModelFactory { initializer { CobrarViewModel(sesion, offline) } },
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
@OptIn(ExperimentalLayoutApi::class)
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

    // Reimprimir se pide todo el tiempo y casi nunca justo después de cobrar:
    // se cortó el papel, salió corrida, el cliente volvió a pedirla. Por eso
    // vive en el paso de Buscar, que es donde la cajera está parada cuando no
    // está cobrando, y no en la pantalla de comprobante que ya se cerró.
    val impresora = impresoraViewModel()
    var reimprimiendo by rememberSaveable { mutableStateOf(false) }

    // El escáner tapa el paso actual en vez de ser un cuarto paso: el carrito
    // sigue siendo el mismo y salir de la cámara deja la venta donde estaba.
    // Se registra después del anterior, así que atrás cierra primero la cámara.
    //
    // Va después de crear el estado de impresora a propósito: los `remember` de
    // arriba tienen que ejecutarse en TODAS las composiciones, también mientras
    // la cámara está abierta. Si el `return` de abajo los saltara, Compose vería
    // otra cantidad de slots al abrir y cerrar el escáner y se perdería la
    // última boleta — el botón de reimprimir desaparecería después de escanear.
    BackHandler(enabled = vm.escaneando) {
        if (vm.creandoProducto) vm.cancelarCreacion() else vm.cerrarEscaner()
    }
    val barcodeOk = packActual().features.barcode
    // Pack feria: barcode=false - cierra el escáner si quedó abierto.
    LaunchedEffect(vm.escaneando, barcodeOk) {
        if (vm.escaneando && !barcodeOk) vm.cerrarEscaner()
    }
    if (vm.escaneando && barcodeOk) {
        PasoEscaner(vm, modifier = Modifier.fillMaxSize())
        return
    }

    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        // En el paso de buscar, la barra de título cede cuando no cabe.
        //
        // Con el teclado abierto en 720x1280 quedan unos 320dp de panel, y esta
        // barra se lleva 73dp al 100% de escala y **140dp al 200%**. Al 200% eso
        // deja fuera de la pantalla, a la vez, la lista de resultados y el botón
        // Cobrar: no es que se vean apretados, es que no entran. De todo lo que hay
        // arriba, lo único que no sirve para cobrar es el rótulo con el nombre de la
        // pantalla, así que es lo que se va.
        //
        // Sólo cuando **no cabe**: en un panel entero, o en un teléfono con más dp
        // que el de referencia, la barra se queda con "Salir" y "Reimprimir" a la
        // vista aunque el teclado esté arriba.
        val hayBarra = vm.paso != PasoDeCobro.Buscar || cabeLaBarraDeTitulo(maxHeight)

        Column(modifier = Modifier.fillMaxSize()) {
            if (hayBarra) {
                RbTopBar(
                    title = when (vm.paso) {
                        PasoDeCobro.Buscar -> "Cobrar"
                        PasoDeCobro.Pago -> "Cómo paga"
                        // Sin red la venta quedó guardada, no cobrada en el sistema.
                        // "Listo" ahí sería la palabra equivocada.
                        PasoDeCobro.Comprobante ->
                            if (vm.ventaEncolada != null) "Guardada" else "Listo"
                    },
                    subtitle = when (vm.paso) {
                        PasoDeCobro.Buscar -> "Busca el producto y agrégalo"
                        PasoDeCobro.Pago -> "Revisa lo que lleva y elige el pago"
                        PasoDeCobro.Comprobante -> null
                    },
                    onBack = if (vm.paso == PasoDeCobro.Pago) vm::volverABuscar else null,
                    actions = {
                        if (vm.paso == PasoDeCobro.Buscar) {
                            // `FlowRow` y **no** `RbChipRow`: ese componente hace
                            // `fillMaxWidth()` porque está pensado para una fila de
                            // chips que ocupa la pantalla, y acá eso le comía el
                            // ancho al título — el subtítulo quedaba tapado por el
                            // botón. Lo que se necesita es sólo la parte de
                            // envolver: al 200% "Reimprimir" y "Salir" no entran uno
                            // al lado del otro en 360dp y tienen que bajar a dos
                            // líneas en vez de partir una palabra por la mitad.
                            FlowRow(
                                horizontalArrangement = Arrangement.spacedBy(RbTheme.dimens.space2),
                                verticalArrangement = Arrangement.spacedBy(RbTheme.dimens.space1),
                            ) {
                                // Pack feria: printer=false - sin reimprimir térmica.
                                if (packActual().features.printer &&
                                    impresora?.hayUltimaBoleta == true
                                ) {
                                    RbButton(
                                        label = "Reimprimir",
                                        onClick = { reimprimiendo = true },
                                        variant = RbButtonVariant.Secondary,
                                    )
                                }
                                RbButton(
                                    label = "Salir",
                                    onClick = vm::salir,
                                    variant = RbButtonVariant.Secondary,
                                )
                            }
                        }
                    },
                )
            }

            if (reimprimiendo && packActual().features.printer &&
                impresora != null && vm.paso == PasoDeCobro.Buscar
            ) {
                TarjetaDeReimpresion(
                    vm = impresora,
                    onCerrar = {
                        reimprimiendo = false
                        impresora.cerrar()
                    },
                    modifier = Modifier.padding(RbTheme.dimens.space3),
                )
            }

            // `weight(1f)` y **no** `fillMaxSize()`. Adentro de esta columna,
            // `fillMaxSize` le pide al paso el alto completo de la pantalla, sin
            // descontar lo que ya ocupó la barra de arriba: el paso arranca en cero
            // y se dibuja **encima** del título. Con la pantalla alta no se nota
            // porque arriba del paso hay padding, pero apenas queda menos alto -el
            // teclado abierto, la franja de "sin conexión", la letra al 200%- el
            // campo de búsqueda le pisa el "Cobrar" y el botón "Salir". Se vio en un
            // emulador API 23 con el teclado arriba.
            when (vm.paso) {
                PasoDeCobro.Buscar -> PasoBuscar(vm, modifier = Modifier.weight(1f))
                PasoDeCobro.Pago -> PasoPago(vm, modifier = Modifier.weight(1f))
                PasoDeCobro.Comprobante -> PasoComprobante(vm, modifier = Modifier.weight(1f))
            }
        }
    }
}

/**
 * Si la barra de título cabe sin dejar inservible el paso de buscar.
 *
 * El paso necesita, apilados: el campo de búsqueda, **una** fila de producto y
 * el botón de cobrar. Medidos en el panel de referencia al 100% de escala son
 * 96 + 72 + 72 = 240dp, y la barra de título son otros 73dp: 313dp en total.
 * [ALTO_PARA_LA_BARRA] es ese número con un poco de aire.
 *
 * El umbral se escala con la **letra ya escalada**, no con un factor fijo:
 * Android 14 escala de forma no lineal -17sp al 200% dan ~29sp, no 34- y las
 * cuatro piezas de arriba crecen con esa curva, no con la que uno supondría.
 * Así el mismo número sirve al 100%, al 200% y en lo que venga.
 *
 * Vive acá, `internal`, para que la prueba de pantalla use **esta** regla y no
 * una copia que se despegue de la de producción.
 */
@Composable
internal fun cabeLaBarraDeTitulo(altoDisponible: Dp): Boolean {
    val cuerpo = with(LocalDensity.current) { RbTheme.typography.body.fontSize.toDp() }
    return altoDisponible >= ALTO_PARA_LA_BARRA * (cuerpo / CUERPO_SIN_ESCALAR)
}

/** Lo que pide el paso de buscar, con barra de título, al 100% de escala. */
private val ALTO_PARA_LA_BARRA = 330.dp

/** El cuerpo de texto del sistema de diseño sin escalar, para medir la curva. */
private val CUERPO_SIN_ESCALAR = 17.dp
