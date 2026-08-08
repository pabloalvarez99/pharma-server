package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import cl.rutbusiness.app.diag.Latencia
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.offline.Fechado
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbList
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme
import androidx.compose.material3.Text

/**
 * Paso 1: buscar y agregar.
 *
 * El campo de búsqueda acepta lo mismo que va a mandar la cámara cuando entre:
 * si lo tipeado son ocho dígitos o más, se resuelve como código de barras antes
 * de buscar por nombre. Cuando el escáner llegue, sólo va a escribir en este
 * mismo campo.
 */
@Composable
fun PasoBuscar(vm: CobrarViewModel, modifier: Modifier = Modifier) {
    val dimens = RbTheme.dimens
    val haptica = LocalHapticFeedback.current
    // El reloj se lee una vez por composición y no en cada fila: la antigüedad
    // del catálogo es una sola frase, no algo que tenga que latir.
    val ahora = LocalOffline.current?.reloj?.invoke() ?: 0L

    // Mide del toque al frame que ya trae el carrito nuevo. Se dispara con el
    // conteo de unidades, así que sólo corre cuando algo entró de verdad.
    LaunchedEffect(vm.carrito.unidades) {
        if (vm.carrito.unidades > 0) withFrameNanos { Latencia.cerrar("agregar al carrito") }
    }

    Column(modifier = modifier) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                // `wrapContentHeight(unbounded = true)` no es decoración.
                // `RbTextField` pinta su `decorationBox` con `fillMaxSize()`, así
                // que dentro de una columna de altura acotada el campo se estira
                // hasta comerse la pantalla entera y deja la lista y la barra de
                // total fuera de cuadro. En el login no se veía porque ahí la
                // columna scrollea y la altura máxima ya es infinita.
                // Midiendo con altura infinita, ese `fillMaxSize` queda inerte y
                // el campo vuelve a medir su contenido.
                // TODO(design-system): cuando `RbTextField` use `fillMaxWidth()`
                // en vez de `fillMaxSize()`, borrar esta línea.
                //
                // `align = Top` no es adorno: el default de `wrapContentHeight`
                // es **centrar**, y cuando el bloque mide más que el alto
                // disponible -teclado abierto, franja de "sin conexión", letra
                // al 200%- centrarlo lo desborda por arriba y por abajo en
                // partes iguales. El desborde de arriba se dibuja **encima** de
                // la barra de título: se veía "Buscar producto" pisando
                // "Cobrar" y el botón "Salir". Alineado arriba, lo que sobra
                // sale por abajo, que es donde hay lista para scrollear.
                .wrapContentHeight(align = Alignment.Top, unbounded = true)
                .padding(horizontal = dimens.space3, vertical = dimens.space2),
        ) {
            RbTextField(
                value = vm.consulta,
                onValueChange = vm::cambiarConsulta,
                label = "Buscar producto",
                placeholder = "Nombre o código de barras",
                // Sin señal, la línea de ayuda cede el lugar a la fecha de lo
                // que se está mostrando. No se suman las dos: en un panel de
                // 640dp cada renglón extra arriba se lo saca a la lista de
                // productos, que es lo que la cajera vino a tocar. Y de las dos
                // frases, "de cuándo es este stock" es la que decide una venta.
                supportingText = if (!vm.hayConexion && vm.catalogoGuardadoEn != null) {
                    "Guardado ${Fechado(Unit, vm.catalogoGuardadoEn!!).antiguedad(ahora)}. " +
                        "El stock puede haber cambiado."
                } else {
                    "Escribe parte del nombre, o el código de barras completo."
                },
                keyboardType = KeyboardType.Text,
            )

            // El botón de la cámara sólo existe si el aparato tiene uno. En una
            // tablet sin cámara -o en un test- no aparece, en vez de aparecer y
            // llevar a un cartel de disculpas.
            if (LocalCamaraDeCodigos.current != null) {
                RbButton(
                    label = "Escanear código",
                    onClick = vm::abrirEscaner,
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                    modifier = Modifier.padding(top = dimens.space2),
                )
            }
        }

        RbDivider()

        RbList(
            items = vm.resultados,
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
            loading = vm.buscando && vm.resultados.isEmpty(),
            error = vm.errorBusqueda,
            onRetry = vm::buscarAhora,
            emptyTitle = if (vm.consulta.isBlank()) {
                "Todavía no hay productos"
            } else {
                "Nada con \"${vm.consulta.trim()}\""
            },
            emptyHint = if (vm.consulta.isBlank()) {
                "Cuando cargues productos en el sistema del negocio, van a aparecer acá para cobrarlos."
            } else {
                "Revisa cómo se escribe, o prueba con una palabra más corta."
            },
            key = { it.id },
        ) { producto ->
            FilaDeProducto(
                producto = producto,
                enCarrito = vm.cantidadEnCarrito(producto.id),
                precio = vm.moneda.formatear(producto.price),
                onAgregar = {
                    Latencia.marcar()
                    // El tic corto es para que la cajera sepa que entró sin
                    // levantar la vista del mostrador.
                    haptica.performHapticFeedback(HapticFeedbackType.LongPress)
                    vm.agregar(producto)
                },
            )
        }

        BarraDeTotal(
            unidades = vm.carrito.unidades,
            // Sin conexión no va monto: el adelanto del mostrador lo suma el
            // teléfono, y sin server no hay quien lo confirme después. La barra
            // ya sabe decirlo con palabras cuando no hay número.
            total = if (vm.hayConexion) {
                vm.carrito.total?.let { vm.moneda.formatear(it) }
            } else {
                null
            },
            onCobrar = vm::irAPagar,
        )
    }
}

/**
 * Una fila del catálogo.
 *
 * El chip de la derecha está **siempre**, y ése es el punto: pasa de "Agregar"
 * a "2" cuando el producto entra al carrito, y como el texto se achica nunca
 * fuerza un salto de línea nuevo. Si el chip apareciera recién al agregar, la
 * fila crecería bajo el dedo y la siguiente se movería justo cuando la cajera
 * va a tocarla.
 */
@Composable
private fun FilaDeProducto(
    producto: ProductDto,
    enCarrito: Int,
    precio: String,
    onAgregar: () -> Unit,
) {
    val sinStock = producto.physicalStock && producto.stock <= 0
    RbListRow(
        title = producto.name,
        subtitle = if (!producto.physicalStock) {
            "Servicio"
        } else if (sinStock) {
            "Sin stock"
        } else {
            "${producto.stock} disponibles"
        },
        value = precio,
        trailing = {
            if (enCarrito > 0) {
                RbChip(label = "$enCarrito", tone = RbChipTone.Brand)
            } else {
                RbChip(label = "Agregar", tone = RbChipTone.Neutral)
            }
        },
        onClick = onAgregar,
    )
}

/**
 * La barra de abajo.
 *
 * Está siempre, aun con el carrito vacío, y con la misma altura: si apareciera
 * al agregar el primer producto, la lista entera se correría hacia arriba
 * exactamente en el momento en que el dedo va bajando a tocar otra fila.
 */
@Composable
private fun BarraDeTotal(
    unidades: Int,
    total: String?,
    onCobrar: () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(colors.surfaceRaised)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            )
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = if (unidades == 0) {
                "Sin productos todavía"
            } else {
                "$unidades ${if (unidades == 1) "producto" else "productos"} · ${total ?: "el sistema confirma el total al cobrar"}"
            },
            style = RbTheme.typography.bodyStrong,
            color = colors.textPrimary,
            textAlign = TextAlign.Start,
            modifier = Modifier.fillMaxWidth(),
        )
        RbButton(
            label = "Cobrar",
            onClick = onCobrar,
            enabled = unidades > 0,
            fillWidth = true,
        )
    }
}
