package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import cl.rutbusiness.app.diag.Latencia
import cl.rutbusiness.app.ui.scanner.AccionesDePermiso
import cl.rutbusiness.app.ui.scanner.Cartel
import cl.rutbusiness.app.ui.scanner.EstadoDelPermiso
import cl.rutbusiness.app.ui.scanner.FORMATOS_DE_RETAIL
import cl.rutbusiness.app.ui.scanner.ExplicacionDelPermiso
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos
import cl.rutbusiness.app.ui.scanner.PanelInferior
import cl.rutbusiness.app.ui.scanner.SinCamara
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import kotlinx.coroutines.delay

/**
 * Escanear: la cámara puesta al servicio de una sola cosa, cargar el carrito.
 *
 * No es una pantalla aparte del cobro, es un paso de [CobrarScreen]: el carrito
 * es el mismo, y salir de acá deja la venta donde estaba. Por eso se lee un
 * producto tras otro sin cerrar nada -entrar y salir de la cámara por cada
 * artículo son dos segundos de arranque del sensor multiplicados por cada línea
 * de la boleta-.
 *
 * La cámara vive exactamente mientras este composable está en pantalla. Salir,
 * abrir el formulario de producto nuevo o irse a otra app la sueltan, porque en
 * los tres casos el visor deja de componerse.
 */
@Composable
fun PasoEscaner(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val camara = LocalCamaraDeCodigos.current
    val haptica = LocalHapticFeedback.current

    // El tic corto es la mitad de la señal: la cajera está mirando el producto
    // y el mostrador, no la pantalla. Se dispara con `ultimaLectura`, que trae
    // un contador adentro justamente para volver a vibrar cuando pasa dos veces
    // el mismo producto.
    LaunchedEffect(vm.ultimaLectura) {
        if (vm.ultimaLectura != null) {
            haptica.performHapticFeedback(HapticFeedbackType.LongPress)
            // Cierra la medición que abrió el ViewModel al aceptar el código: el
            // número que sale es lo que la cajera espera entre el pitido y ver
            // el producto adentro.
            withFrameNanos { Latencia.cerrar("escaneo al carrito") }
        }
    }
    // Dos tics para "lo leí, pero no lo tengo": distinto al de éxito sin
    // obligar a mirar. Un solo patrón para las dos cosas sería peor que nada.
    LaunchedEffect(vm.codigoSinProducto) {
        if (vm.codigoSinProducto != null) {
            haptica.performHapticFeedback(HapticFeedbackType.LongPress)
            delay(140)
            haptica.performHapticFeedback(HapticFeedbackType.LongPress)
        }
    }

    Column(modifier = modifier) {
        RbTopBar(
            title = if (vm.creandoProducto) "Producto nuevo" else "Escanear",
            subtitle = when {
                vm.creandoProducto -> "No estaba en tu catálogo"
                camara != null -> "Apunta al código de barras"
                else -> null
            },
            onBack = if (vm.creandoProducto) vm::cancelarCreacion else vm::cerrarEscaner,
        )

        when {
            camara == null -> SinCamara(
                onEscribirAMano = vm::escribirAMano,
                modifier = Modifier.fillMaxSize(),
            )

            vm.creandoProducto -> FormularioDeProductoNuevo(vm, modifier = Modifier.fillMaxSize())

            else -> {
                val permiso = camara.recordarPermiso()
                if (permiso.estado == EstadoDelPermiso.Concedido) {
                    camara.Visor(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth(),
                        formatos = FORMATOS_DE_RETAIL,
                        onCodigo = vm::escanear,
                    )
                    PanelDeLectura(vm)
                } else {
                    ExplicacionDelPermiso(
                        estado = permiso.estado,
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth(),
                    )
                    PanelInferior {
                        AccionesDePermiso(
                            estado = permiso.estado,
                            onPermitir = permiso::pedir,
                            onAjustes = permiso::abrirAjustes,
                            onEscribirAMano = vm::escribirAMano,
                        )
                    }
                }
            }
        }
    }
}

/**
 * Qué pasó con lo último que se leyó, y cómo salir.
 *
 * El cartel de arriba cambia; la fila de botones de abajo **no se mueve**. En un
 * teléfono que se usa sin mirar, un botón que cambia de lugar según el estado es
 * un toque equivocado por venta.
 */
@Composable
private fun PanelDeLectura(vm: CobrarViewModel) {
    val colors = RbTheme.colors
    val lectura = vm.ultimaLectura
    val sinProducto = vm.codigoSinProducto
    val error = vm.errorDeEscaneo

    PanelInferior {
        when {
            sinProducto != null -> Cartel(
                fondo = colors.dangerContainer,
                titulo = "Ese código no está en tu catálogo",
                detalle = "Leímos $sinProducto. Puedes crearlo ahora o escribirlo a mano.",
            )

            error != null -> Cartel(
                fondo = colors.dangerContainer,
                titulo = error.title,
                detalle = error.message,
            )

            lectura != null -> Cartel(
                fondo = colors.brandContainer,
                titulo = "Listo: ${lectura.nombre}",
                detalle = if (lectura.enCarrito > 1) {
                    "${lectura.enCarrito} unidades en el carrito"
                } else {
                    "1 unidad en el carrito"
                },
            )

            else -> Cartel(
                fondo = colors.surfaceVariant,
                titulo = "Apunta al código de barras",
                detalle = "Pasa un producto tras otro; no hace falta salir entre uno y otro.",
            )
        }

        Text(
            text = resumenDelCarrito(vm),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
            modifier = Modifier.fillMaxWidth(),
        )

        RbReflowRow(
            spacing = RbTheme.dimens.space2,
            content = {
                if (sinProducto != null) {
                    RbButton(label = "Crear producto", onClick = vm::crearProductoDelCodigo)
                } else {
                    RbButton(label = "Listo", onClick = vm::cerrarEscaner)
                }
            },
            trailing = {
                RbButton(
                    label = "Escribir a mano",
                    onClick = vm::escribirAMano,
                    variant = RbButtonVariant.Secondary,
                )
            },
        )
    }
}

private fun resumenDelCarrito(vm: CobrarViewModel): String {
    val unidades = vm.carrito.unidades
    if (unidades == 0) return "El carrito está vacío"
    val total = vm.carrito.total?.let { vm.moneda.formatear(it) }
    val palabra = if (unidades == 1) "producto" else "productos"
    return "$unidades $palabra${total?.let { " · $it" } ?: ""}"
}

/**
 * Crear el producto del código que no estaba, sin salir de la venta.
 *
 * Nombre y precio y nada más: lo que hace falta para cobrarlo hoy. El resto de
 * la ficha -categoría, laboratorio, costo- se completa después desde el
 * sistema del negocio, con calma y sin un cliente esperando.
 *
 * Mientras este formulario está arriba, el visor no está compuesto y la cámara
 * quedó suelta: no tiene sentido tener el sensor encendido mientras alguien
 * escribe.
 */
@Composable
private fun FormularioDeProductoNuevo(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val codigo = vm.codigoSinProducto

    Column(modifier = modifier) {
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = dimens.space3, vertical = dimens.space2),
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            Text(
                text = "Código $codigo",
                style = RbTheme.typography.numeric,
                color = RbTheme.colors.textPrimary,
            )

            // `wrapContentHeight(unbounded = true)`: mismo motivo que en
            // `PasoBuscar`. `RbTextField` pinta su decoración con
            // `fillMaxSize()` y dentro de una altura acotada se come la
            // pantalla. Se borra cuando el design system pase a `fillMaxWidth`.
            Column(modifier = Modifier.wrapContentHeight(unbounded = true)) {
                RbTextField(
                    value = vm.nombreNuevo,
                    onValueChange = vm::cambiarNombreNuevo,
                    label = "Nombre",
                    placeholder = "Como quieres verlo en la boleta",
                    enabled = !vm.guardandoProducto,
                    imeAction = ImeAction.Next,
                )
            }

            Column(modifier = Modifier.wrapContentHeight(unbounded = true)) {
                RbTextField(
                    value = vm.precioNuevo,
                    onValueChange = vm::cambiarPrecioNuevo,
                    label = "Precio",
                    placeholder = "Sólo números",
                    supportingText = vm.precioNuevo
                        .takeIf { it.isNotBlank() }
                        ?.let { "Se cobra ${vm.moneda.formatear(it)}" }
                        ?: "El precio de venta al público.",
                    numeric = true,
                    keyboardType = KeyboardType.Number,
                    enabled = !vm.guardandoProducto,
                )
            }

            // El error va acá y no colgado de un campo: puede venir del nombre,
            // del precio o del server, y colgarlo del campo equivocado manda a
            // corregir donde no era.
            vm.errorAlCrear?.let {
                Cartel(fondo = RbTheme.colors.dangerContainer, titulo = it)
            }

            Text(
                text = "Nace con una unidad en stock: la que tienes en la mano. El resto de la " +
                    "ficha se completa después desde el sistema del negocio.",
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        }

        PanelInferior {
            RbReflowRow(
                spacing = dimens.space2,
                content = {
                    RbButton(
                        label = if (vm.guardandoProducto) "Creando…" else "Crear y agregar",
                        onClick = vm::guardarProductoNuevo,
                        enabled = !vm.guardandoProducto,
                    )
                },
                trailing = {
                    RbButton(
                        label = "Escribir a mano",
                        onClick = vm::escribirAMano,
                        variant = RbButtonVariant.Secondary,
                        enabled = !vm.guardandoProducto,
                    )
                },
            )
        }
    }
}
