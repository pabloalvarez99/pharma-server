package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.background
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import cl.rutbusiness.app.diag.Latencia
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.offline.Fechado
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorCopy
import cl.rutbusiness.ui.components.RbList
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbReflowRow
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
    val haptica = LocalHapticFeedback.current
    // El reloj se lee una vez por composición y no en cada fila: la antigüedad
    // del catálogo es una sola frase, no algo que tenga que latir.
    val ahora = LocalOffline.current?.reloj?.invoke() ?: 0L

    // Mide del toque al frame que ya trae el carrito nuevo. Se dispara con el
    // conteo de unidades, así que sólo corre cuando algo entró de verdad.
    LaunchedEffect(vm.carrito.unidades) {
        if (vm.carrito.unidades > 0) withFrameNanos { Latencia.cerrar("agregar al carrito") }
    }

    BuscarContenido(
        modifier = modifier,
        consulta = vm.consulta,
        onConsulta = vm::cambiarConsulta,
        // Sin señal, la línea de ayuda cede el lugar a la fecha de lo que se
        // está mostrando. No se suman las dos: en un panel de 640dp cada
        // renglón extra arriba se lo saca a la lista de productos, que es lo
        // que la cajera vino a tocar. Y de las dos frases, "de cuándo es este
        // stock" es la que decide una venta.
        ayuda = when {
            !vm.hayConexion && vm.catalogoGuardadoEn != null ->
                "Guardado ${Fechado(Unit, vm.catalogoGuardadoEn!!).antiguedad(ahora)}. " +
                    "El stock puede haber cambiado."
            !packActual().features.barcode ->
                "Escribe el nombre de lo que vendes (tomate, atado, bolsa…)."
            else ->
                "Escribe parte del nombre, o el código de barras completo."
        },
        // Cámara + pack.barcode: feria apaga el escáner (ADR-0022) aunque el
        // teléfono tenga cámara. En un test sin LocalRubro el pack default
        // deja barcode=true y el botón sigue el hardware.
        onEscanear = when {
            !packActual().features.barcode -> null
            LocalCamaraDeCodigos.current == null -> null
            else -> vm::abrirEscaner
        },
        etiquetaBuscar = if (packActual().features.barcode) {
            "Buscar producto"
        } else {
            "¿Qué vendiste?"
        },
        placeholderBuscar = if (packActual().features.barcode) {
            "Nombre o código de barras"
        } else {
            "Nombre (tomate, cilantro…)"
        },
        resultados = vm.resultados,
        buscando = vm.buscando,
        errorBusqueda = vm.errorBusqueda,
        onBuscarAhora = vm::buscarAhora,
        cantidadEnCarrito = vm::cantidadEnCarrito,
        precioDe = { vm.moneda.formatear(it.price) },
        onAgregar = { producto ->
            Latencia.marcar()
            // El tic corto es para que la cajera sepa que entró sin levantar la
            // vista del mostrador.
            haptica.performHapticFeedback(HapticFeedbackType.LongPress)
            vm.agregar(producto)
        },
        unidades = vm.carrito.unidades,
        // Sin conexión no va monto: el adelanto del mostrador lo suma el
        // teléfono, y sin server no hay quien lo confirme después. La barra ya
        // sabe decirlo con palabras cuando no hay número.
        total = if (vm.hayConexion) vm.carrito.total?.let { vm.moneda.formatear(it) } else null,
        onCobrar = vm::irAPagar,
    )
}

/**
 * El paso de buscar, sin `ViewModel`.
 *
 * Todo lo que decide el layout entra por parámetro para que se pueda medir en
 * una prueba de JUnit -que es donde vive la escala de letra al 200%- sin montar
 * el grafo de red, la sesión y el caché.
 */
@Composable
internal fun BuscarContenido(
    consulta: String,
    onConsulta: (String) -> Unit,
    ayuda: String,
    onEscanear: (() -> Unit)?,
    resultados: List<ProductDto>,
    buscando: Boolean,
    errorBusqueda: RbErrorCopy?,
    onBuscarAhora: () -> Unit,
    cantidadEnCarrito: (String) -> Int,
    precioDe: (ProductDto) -> String,
    onAgregar: (ProductDto) -> Unit,
    unidades: Int,
    total: String?,
    onCobrar: () -> Unit,
    modifier: Modifier = Modifier,
    etiquetaBuscar: String = "Buscar producto",
    placeholderBuscar: String = "Nombre o código de barras",
) {
    val dimens = RbTheme.dimens

    // Con algo escrito, la pantalla es de los resultados.
    //
    // Es el momento exacto en que el teclado se llevó más de la mitad del panel
    // y la cajera está mirando si apareció lo que busca. Las dos cosas que se
    // esconden acá son las dos que enseñan a usar el buscador -la línea de ayuda
    // y el botón de la cámara-, y enseñar ya no es lo que hace falta: son 100dp
    // en 720x1280, o sea la fila y media de productos que faltaban. Vuelven
    // solas al borrar lo escrito, que es cuando la cajera está eligiendo entre
    // teclear y escanear.
    val escribiendo = consulta.isNotBlank()

    Column(modifier = modifier) {
        // El buscador va arriba de la lista y **acotado a su alto natural**.
        //
        // Antes esta misma columna llevaba `wrapContentHeight(unbounded = true)`
        // para tapar un `fillMaxSize()` que `RbTextField` ya no tiene, y el
        // efecto era que el bloque de búsqueda pedía todo el alto que hubiera.
        // Con el teclado abierto en 720x1280 la lista de resultados quedaba en
        // **cero**: el producto se encontraba y no se veía hasta cerrar el
        // teclado. Al 200% de escala, además, la barra de total y el botón
        // Cobrar quedaban fuera de la pantalla.
        //
        // Meterlo adentro del `LazyColumn` como primer ítem también le da el
        // alto sobrante a la lista, pero se midió y es peor: el encabezado ocupa
        // 190dp de los 175dp de lista que deja el teclado, así que no se ve
        // ninguna fila hasta scrollear, y al scrollear el campo se recicla y el
        // teclado se cierra solo. Fijo y chico, el campo no se mueve nunca y las
        // filas empiezan justo debajo.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = dimens.space3, vertical = dimens.space2),
        ) {
            RbTextField(
                value = consulta,
                onValueChange = onConsulta,
                label = etiquetaBuscar,
                placeholder = placeholderBuscar,
                supportingText = if (escribiendo) null else ayuda,
                keyboardType = KeyboardType.Text,
                // La tecla de acción del teclado busca ya, sin esperar el
                // rebote de 250 ms. No es la única forma -la lista se refresca
                // sola mientras se escribe-, es la que la cajera aprieta cuando
                // terminó de escribir y quiere el resultado ahora.
                imeAction = ImeAction.Search,
                onImeAction = onBuscarAhora,
            )

            if (onEscanear != null && !escribiendo) {
                RbButton(
                    label = "Escanear código",
                    onClick = onEscanear,
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                    modifier = Modifier.padding(top = dimens.space2),
                )
            }
        }

        RbDivider()

        RbList(
            items = resultados,
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
            loading = buscando && resultados.isEmpty(),
            error = errorBusqueda,
            onRetry = onBuscarAhora,
            emptyTitle = if (consulta.isBlank()) {
                "Todavía no hay productos"
            } else {
                "Nada con \"${consulta.trim()}\""
            },
            emptyHint = if (consulta.isBlank()) {
                "Cuando cargues productos en el sistema del negocio, van a aparecer acá para cobrarlos."
            } else {
                "Revisa cómo se escribe, o prueba con una palabra más corta."
            },
            key = { it.id },
        ) { producto ->
            FilaDeProducto(
                producto = producto,
                enCarrito = cantidadEnCarrito(producto.id),
                precio = precioDe(producto),
                onAgregar = { onAgregar(producto) },
            )
        }

        BarraDeTotal(
            unidades = unidades,
            total = total,
            onCobrar = onCobrar,
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
 *
 * Está **anclada**: queda fuera de la lista a propósito, porque el total y el
 * botón de cobrar tienen que verse con el teclado abierto y con la letra al
 * 200%, que es cuando menos pantalla hay.
 *
 * Por eso también es una fila y no dos renglones apilados. El total al lado del
 * botón mide unos 46dp menos, y esos 46dp se los queda la lista de resultados —
 * que con el teclado arriba es lo único que estaba faltando. [RbReflowRow] los
 * baja a dos renglones sola, y sólo cuando al texto no le alcanza el ancho sin
 * partir una palabra por la mitad.
 */
@Composable
private fun BarraDeTotal(
    unidades: Int,
    total: String?,
    onCobrar: () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbReflowRow(
        spacing = dimens.space2,
        modifier = Modifier
            .fillMaxWidth()
            .background(colors.surfaceRaised)
            .windowInsetsPadding(
                WindowInsets.safeDrawing.only(
                    WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                ),
            )
            .imePadding()
            .padding(horizontal = dimens.space3, vertical = dimens.space2),
        content = {
            Text(
                text = if (unidades == 0) {
                    "Sin productos todavía"
                } else {
                    "$unidades ${if (unidades == 1) "producto" else "productos"} · ${total ?: "el sistema confirma el total al cobrar"}"
                },
                style = RbTheme.typography.bodyStrong,
                color = colors.textPrimary,
                textAlign = TextAlign.Start,
            )
        },
        trailing = {
            RbButton(
                label = "Cobrar",
                onClick = onCobrar,
                enabled = unidades > 0,
            )
        },
    )
}
