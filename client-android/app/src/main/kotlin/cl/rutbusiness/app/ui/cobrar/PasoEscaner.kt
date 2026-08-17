package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import cl.rutbusiness.app.diag.Latencia
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.app.ui.scanner.AccionesDePermiso
import cl.rutbusiness.app.ui.scanner.Cartel
import cl.rutbusiness.app.ui.scanner.EstadoDelPermiso
import cl.rutbusiness.app.ui.scanner.FORMATOS_DE_RETAIL
import cl.rutbusiness.app.ui.scanner.ExplicacionDelPermiso
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos
import cl.rutbusiness.app.ui.scanner.MarcoDeLinterna
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
 * Se siente una **linterna**, no un lab: se apunta, se lee el código, vibra y
 * sigue el siguiente producto. No es una pantalla aparte del cobro, es un paso
 * de [CobrarScreen]: el carrito es el mismo, y salir de acá deja la venta donde
 * estaba. Por eso se lee un producto tras otro sin cerrar nada -entrar y salir
 * de la cámara por cada artículo son dos segundos de arranque del sensor
 * multiplicados por cada línea de la boleta-.
 *
 * La cámara vive exactamente mientras este composable está en pantalla. Salir,
 * abrir el formulario de producto nuevo o irse a otra app la sueltan, porque en
 * los tres casos el visor deja de componerse. El marco y el panel son sólo
 * pintura: no tocan el analizador ni CameraX.
 */
@Composable
fun PasoEscaner(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val camara = LocalCamaraDeCodigos.current
    val haptica = LocalHapticFeedback.current
    val feria = esFeria()

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
            title = copyTituloEscaner(vm.creandoProducto),
            subtitle = copySubtituloEscaner(
                feria = feria,
                creandoProducto = vm.creandoProducto,
                hayCamara = camara != null,
            ),
            onBack = if (vm.creandoProducto) vm::cancelarCreacion else vm::cerrarEscaner,
        )

        when {
            camara == null -> SinCamara(
                onEscribirAMano = vm::escribirAMano,
                modifier = Modifier.fillMaxSize(),
            )

            vm.creandoProducto -> FormularioDeProductoNuevo(
                vm,
                feria = feria,
                modifier = Modifier.fillMaxSize(),
            )

            else -> {
                val permiso = camara.recordarPermiso()
                if (permiso.estado == EstadoDelPermiso.Concedido) {
                    // Visor + marco de linterna: el video sigue intacto; encima
                    // sólo se pinta el cono para saber dónde apoyar el código.
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth(),
                    ) {
                        camara.Visor(
                            modifier = Modifier.fillMaxSize(),
                            formatos = FORMATOS_DE_RETAIL,
                            onCodigo = vm::escanear,
                        )
                        MarcoDeLinterna(Modifier.fillMaxSize())
                    }
                    PanelDeLectura(vm, feria = feria)
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
 * un toque equivocado por venta. Botones ≥ 56 dp vía design system.
 */
@Composable
private fun PanelDeLectura(vm: CobrarViewModel, feria: Boolean) {
    val colors = RbTheme.colors
    val lectura = vm.ultimaLectura
    val sinProducto = vm.codigoSinProducto
    val error = vm.errorDeEscaneo

    PanelInferior {
        when {
            sinProducto != null -> Cartel(
                fondo = colors.dangerContainer,
                titulo = copyTituloSinProducto(feria),
                detalle = copyDetalleSinProducto(sinProducto),
            )

            error != null -> Cartel(
                fondo = colors.dangerContainer,
                titulo = error.title,
                detalle = error.message,
            )

            lectura != null -> Cartel(
                fondo = colors.brandContainer,
                titulo = copyTituloListo(lectura.nombre),
                detalle = copyDetalleListo(feria, lectura.enCarrito),
            )

            else -> Cartel(
                fondo = colors.surfaceVariant,
                titulo = copyTituloEsperando(),
                detalle = copyDetalleEsperando(),
            )
        }

        Text(
            text = resumenDelCarrito(vm, feria),
            style = RbTheme.typography.support,
            color = colors.textSecondary,
            modifier = Modifier.fillMaxWidth(),
        )

        RbReflowRow(
            spacing = RbTheme.dimens.space2,
            content = {
                if (sinProducto != null) {
                    RbButton(label = copyBotonCrearProducto(), onClick = vm::crearProductoDelCodigo)
                } else {
                    RbButton(label = copyBotonListo(), onClick = vm::cerrarEscaner)
                }
            },
            trailing = {
                RbButton(
                    label = copyBotonEscribirAMano(),
                    onClick = vm::escribirAMano,
                    variant = RbButtonVariant.Secondary,
                )
            },
        )
    }
}

/**
 * Reusa [copyBarraCarrito]: la misma frase de "lo que llevas" que ya fija
 * [PasoBuscar], para no decirlo dos veces distinto en la misma venta.
 */
private fun resumenDelCarrito(vm: CobrarViewModel, feria: Boolean): String {
    val total = vm.carrito.total?.let { vm.moneda.formatear(it) }
    return copyBarraCarrito(unidades = vm.carrito.unidades, total = total, feria = feria)
}

/**
 * Crear el producto del código que no estaba, sin salir de la venta.
 *
 * Nombre y precio y nada más: lo que hace falta para cobrarlo hoy. El resto de
 * la ficha -categoría, laboratorio, costo- se completa después pidiéndoselo al
 * agente, con calma y sin un cliente esperando.
 *
 * Mientras este formulario está arriba, el visor no está compuesto y la cámara
 * quedó suelta: no tiene sentido tener el sensor encendido mientras alguien
 * escribe.
 */
@Composable
private fun FormularioDeProductoNuevo(
    vm: CobrarViewModel,
    feria: Boolean,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card
    val codigo = vm.codigoSinProducto

    Column(modifier = modifier) {
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(colors.surfaceRaised)
                    .border(dimens.border, colors.outlineStrong, shape)
                    .padding(horizontal = dimens.space3, vertical = dimens.space4),
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                Text(
                    text = copyEtiquetaCodigoLeido(),
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
                Text(
                    text = codigo.orEmpty(),
                    style = RbTheme.typography.numeric,
                    color = colors.textPrimary,
                )

                RbTextField(
                    value = vm.nombreNuevo,
                    onValueChange = vm::cambiarNombreNuevo,
                    label = copyEtiquetaNombreNuevo(),
                    placeholder = copyPlaceholderNombreNuevo(feria),
                    enabled = !vm.guardandoProducto,
                    imeAction = ImeAction.Next,
                )

                RbTextField(
                    value = vm.precioNuevo,
                    onValueChange = vm::cambiarPrecioNuevo,
                    label = copyEtiquetaPrecioNuevo(),
                    placeholder = copyPlaceholderPrecioNuevo(),
                    supportingText = copyAyudaPrecioNuevo(
                        feria = feria,
                        cobrado = vm.precioNuevo
                            .takeIf { it.isNotBlank() }
                            ?.let { vm.moneda.formatear(it) },
                    ),
                    numeric = true,
                    keyboardType = KeyboardType.Number,
                    enabled = !vm.guardandoProducto,
                )

                // El error va acá y no colgado de un campo: puede venir del nombre,
                // del precio o del server, y colgarlo del campo equivocado manda a
                // corregir donde no era.
                vm.errorAlCrear?.let {
                    Cartel(fondo = colors.dangerContainer, titulo = it)
                }

                Text(
                    text = copyNotaProductoNuevo(feria),
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        }

        PanelInferior {
            RbReflowRow(
                spacing = dimens.space2,
                content = {
                    RbButton(
                        label = copyBotonCrearYAgregar(vm.guardandoProducto),
                        onClick = vm::guardarProductoNuevo,
                        enabled = !vm.guardandoProducto,
                    )
                },
                trailing = {
                    RbButton(
                        label = copyBotonEscribirAMano(),
                        onClick = vm::escribirAMano,
                        variant = RbButtonVariant.Secondary,
                        enabled = !vm.guardandoProducto,
                    )
                },
            )
        }
    }
}
