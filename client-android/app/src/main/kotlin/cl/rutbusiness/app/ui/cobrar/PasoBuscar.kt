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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import cl.rutbusiness.app.diag.Latencia
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos
import cl.rutbusiness.core.api.models.ProductDto
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

    // Mide del toque al frame que ya trae el carrito nuevo. Se dispara con el
    // conteo de unidades, así que sólo corre cuando algo entró de verdad.
    LaunchedEffect(vm.carrito.unidades) {
        if (vm.carrito.unidades > 0) withFrameNanos { Latencia.cerrar("agregar al carrito") }
    }

    Column(modifier = modifier) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = dimens.space3, vertical = dimens.space2),
        ) {
            RbTextField(
                value = vm.consulta,
                onValueChange = vm::cambiarConsulta,
                label = "Buscar producto",
                placeholder = "Nombre o código de barras",
                supportingText = "Escribe parte del nombre, o el código de barras completo.",
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
            loadingLabel = "Buscando en tu catálogo...",
            error = vm.errorBusqueda,
            onRetry = vm::buscarAhora,
            emptyTitle = if (vm.consulta.isBlank()) {
                "Todavía no tienes productos"
            } else {
                "Nada con «${vm.consulta.trim()}»"
            },
            // El vacío enseña los dos caminos que existen **dentro de la app**,
            // y no manda a "cargarlos en el sistema del negocio" como decía
            // antes: eso es un callejón sin salida para alguien que sólo tiene
            // el teléfono en la mano.
            emptyHint = if (vm.consulta.isBlank()) {
                "Pídeselo al agente («agrega un producto»), o escanea un código de barras: " +
                    "cuando no esté en el catálogo, la app te deja crearlo ahí mismo."
            } else {
                "Revisa cómo se escribe, o prueba con una palabra más corta."
            },
            emptyActionLabel = "Borrar la búsqueda".takeIf { vm.consulta.isNotBlank() },
            onEmptyAction = { vm.cambiarConsulta("") }.takeIf { vm.consulta.isNotBlank() },
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
            total = vm.carrito.total?.let { vm.moneda.formatear(it) },
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
            // "El carrito está vacío" y no "sin productos todavía": arriba, en el
            // vacío del catálogo, dice "Todavía no tienes productos", y las dos
            // frases juntas se leían como la misma cosa dicha dos veces. Son
            // cosas distintas -- el catálogo del negocio y el carrito de esta
            // venta -- y ahora se llaman distinto. Es además la misma frase que
            // usa el panel del escáner para el carrito vacío.
            text = if (unidades == 0) {
                "El carrito está vacío"
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
