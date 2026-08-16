package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import cl.rutbusiness.app.diag.Latencia
import cl.rutbusiness.app.ui.catalogo.abrirCatalogo
import cl.rutbusiness.app.ui.catalogo.copyCatalogo
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.offline.Fechado
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorCopy
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * Paso 1: buscar y agregar.
 *
 * Se siente la mesa del puesto, no un POS de retail: campo calmado, cada
 * resultado es una tarjeta gruesa (nombre + precio), y el botón Cobrar es
 * brand fill de 56dp anclado abajo.
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

    val pack = packActual()
    val barcodeOk = pack.features.barcode
    val copy = copyBuscarCobrar(barcodeOk)
    // El catálogo se nombra con las palabras del pack, no con las de esta
    // pantalla: en feria el botón dice "Agregar una cosa" y en farmacia "Agregar
    // un producto", porque lo dice `vocab` (ADR-0022).
    val copyDelCatalogo = copyCatalogo(pack)
    val feria = esFeria()
    BuscarContenido(
        modifier = modifier,
        consulta = vm.consulta,
        onConsulta = vm::cambiarConsulta,
        feria = feria,
        // Sin señal, la línea de ayuda cede el lugar a la fecha de lo que se
        // está mostrando. No se suman las dos: en un panel de 640dp cada
        // renglón extra arriba se lo saca a la lista, que es lo que la
        // cajera vino a tocar. Y de las dos frases, "de cuándo es esto" es
        // la que decide una venta.
        ayuda = when {
            !vm.hayConexion && vm.catalogoGuardadoEn != null ->
                copyAyudaCatalogoGuardado(
                    feria = feria,
                    antiguedad = Fechado(Unit, vm.catalogoGuardadoEn!!).antiguedad(ahora),
                )
            else -> copy.ayudaOnline
        },
        // Cámara + pack.barcode: feria apaga el escáner (ADR-0022) aunque el
        // teléfono tenga cámara. En un test sin LocalRubro el pack default
        // deja barcode=true y el botón sigue el hardware.
        onEscanear = if (
            escanerVisible(
                barcode = barcodeOk,
                hayCamara = LocalCamaraDeCodigos.current != null,
            )
        ) {
            vm::abrirEscaner
        } else {
            null
        },
        etiquetaBuscar = copy.etiqueta,
        placeholderBuscar = copy.placeholder,
        // La salida del callejón sin salida. Antes, con el catálogo vacío esta
        // pantalla decía "cuando cargues productos van a aparecer acá" y no
        // había ningún lugar en la app donde cargarlos que no fuera el escáner
        // -que en feria está apagado-. Ahora el vacío ofrece el camino.
        onCargarLoQueVendo = abrirCatalogo(),
        etiquetaCargar = copyDelCatalogo.agregar,
        tituloSinCatalogo = copyDelCatalogo.vacioTitulo,
        pistaSinCatalogo = copyDelCatalogo.vacioPista,
        onMontoSuelto = vm::abrirMontoSuelto,
        montoSueltoPuesto = vm.montoSueltoEnElCarrito()?.let { vm.moneda.formatear(it) },
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
    /**
     * Abre "Lo que vendo". `null` cuando desde acá no se puede -sin sesión-, y
     * entonces la pantalla no ofrece el camino en vez de ofrecer un botón muerto.
     */
    onCargarLoQueVendo: (() -> Unit)? = null,
    etiquetaCargar: String = "Agregar lo que vendo",
    tituloSinCatalogo: String = "Todavía no hay productos",
    pistaSinCatalogo: String = "Agrega lo que vendes con su precio y ya puedes cobrarlo.",
    /** Abre el cobro rápido. `null` lo deja fuera de la pantalla. */
    onMontoSuelto: (() -> Unit)? = null,
    /** El monto suelto que ya está en el carrito, formateado. */
    montoSueltoPuesto: String? = null,
    /** Pack feria: el miss de búsqueda enseña venderle al agente. */
    feria: Boolean = false,
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
        // Campo de búsqueda **calmado**: alto natural, sin tutorial largo, fijo
        // fuera de la lista para que con el teclado no se coma los resultados.
        //
        // Antes un `wrapContentHeight(unbounded)` le daba al bloque de búsqueda
        // todo el alto sobrante y la lista quedaba en cero. Fijo y chico, el
        // campo no se mueve y las tarjetas empiezan justo debajo.
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

            // El cobro rápido está a la vista, no escondido: "son $2.000" es la
            // venta más común del puesto, y esconderla detrás de un menú sería
            // poner dos toques en el camino más corto que tiene la app.
            //
            // Se esconde mientras se escribe, igual que el botón de la cámara y
            // por el mismo motivo medido: son los dp que le faltan a la lista de
            // resultados con el teclado arriba. Quien está tipeando un nombre no
            // está por cobrar un monto suelto.
            if (onMontoSuelto != null && !escribiendo) {
                RbButton(
                    label = if (montoSueltoPuesto == null) {
                        "Cobrar un monto"
                    } else {
                        "Monto puesto: $montoSueltoPuesto"
                    },
                    onClick = onMontoSuelto,
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                    modifier = Modifier.padding(top = dimens.space2),
                )
            }
        }

        RbDivider()

        // Mesa: tarjetas con aire, no filas de extracto con divisor a borde.
        // (RbList trae divisor y no da el aire de tarjeta que pide el puesto.)
        val listaMod = Modifier
            .weight(1f)
            .fillMaxWidth()

        when {
            errorBusqueda != null -> Column(
                modifier = listaMod.verticalScroll(rememberScrollState()),
            ) {
                RbErrorState(
                    title = errorBusqueda.title,
                    message = errorBusqueda.message,
                    modifier = Modifier.padding(dimens.space3),
                    retryLabel = errorBusqueda.retryLabel,
                    onRetry = onBuscarAhora,
                )
            }

            buscando && resultados.isEmpty() -> Column(modifier = listaMod) {
                RbLoadingState(label = "Buscando…")
                RbSkeletonLines(
                    lines = 5,
                    modifier = Modifier.padding(dimens.space3),
                )
            }

            resultados.isEmpty() -> Column(
                modifier = listaMod.verticalScroll(rememberScrollState()),
            ) {
                RbEmptyState(
                    title = if (consulta.isBlank()) {
                        tituloSinCatalogo
                    } else {
                        "Nada con \"${consulta.trim()}\""
                    },
                    hint = if (consulta.isBlank()) {
                        pistaSinCatalogo
                    } else {
                        pistaBusquedaVacia(
                            feria = feria,
                            consulta = consulta,
                            puedeCargar = onCargarLoQueVendo != null,
                        )
                    },
                    actionLabel = if (onCargarLoQueVendo != null) etiquetaCargar else null,
                    onAction = onCargarLoQueVendo,
                )
            }

            else -> LazyColumn(
                modifier = listaMod,
                contentPadding = PaddingValues(
                    horizontal = dimens.space3,
                    vertical = dimens.space2,
                ),
                verticalArrangement = Arrangement.spacedBy(dimens.space2),
            ) {
                items(items = resultados, key = { it.id }) { producto ->
                    FilaDeProducto(
                        producto = producto,
                        enCarrito = cantidadEnCarrito(producto.id),
                        precio = precioDe(producto),
                        feria = feria,
                        onAgregar = { onAgregar(producto) },
                    )
                }
            }
        }

        BarraDeTotal(
            unidades = unidades,
            total = total,
            feria = feria,
            onCobrar = onCobrar,
        )
    }
}

/**
 * Una cosa de la mesa: tarjeta gruesa, nombre grande, precio numérico, ≥56dp.
 *
 * En feria no se grita stock ni "Servicio": el puesto no cuenta unidades en
 * góndola. En retail sí se avisa sin stock / disponibles.
 */
@Composable
private fun FilaDeProducto(
    producto: ProductDto,
    enCarrito: Int,
    precio: String,
    feria: Boolean,
    onAgregar: () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card
    val sinStock = producto.physicalStock && producto.stock <= 0
    val detalle: String? = when {
        feria -> null
        !producto.physicalStock -> "Servicio"
        sinStock -> "Sin stock"
        else -> "${producto.stock} disponibles"
    }

    RbReflowRow(
        spacing = dimens.space3,
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surface)
            .border(dimens.border, colors.outline, shape)
            .rbClickable(
                onClick = onAgregar,
                role = Role.Button,
                shape = shape,
            )
            .rbTouchTarget()
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        content = {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
                Text(
                    text = producto.name,
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                )
                if (detalle != null) {
                    Text(
                        text = detalle,
                        style = RbTheme.typography.support,
                        color = if (sinStock) colors.dangerText else colors.textSecondary,
                    )
                }
            }
        },
        trailing = {
            Column(
                verticalArrangement = Arrangement.spacedBy(dimens.space1),
                horizontalAlignment = Alignment.End,
            ) {
                RbAmount(
                    amount = precio,
                    emphasis = RbAmountEmphasis.Body,
                )
                if (enCarrito > 0) {
                    RbChip(label = "$enCarrito", tone = RbChipTone.Brand)
                } else {
                    RbChip(label = "Agregar", tone = RbChipTone.Neutral)
                }
            }
        },
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
 * El botón es brand fill (Primary) y ≥56dp vía [RbButton].
 */
@Composable
private fun BarraDeTotal(
    unidades: Int,
    total: String?,
    feria: Boolean,
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
                text = copyBarraCarrito(unidades = unidades, total = total, feria = feria),
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
                // Brand fill Primary por default; 56dp vía rbTouchTarget.
            )
        },
    )
}
