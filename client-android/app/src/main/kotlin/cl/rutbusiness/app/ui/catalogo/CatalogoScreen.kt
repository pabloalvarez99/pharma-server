package cl.rutbusiness.app.ui.catalogo

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorCopy
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.icons.RbIcons
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbTouchTarget

/**
 * "Lo que vendo" enchufada a la sesión.
 *
 * La clave del `viewModel` incluye la clave de unidad del pack: si el negocio
 * cambia de rubro, el catálogo se vuelve a construir leyendo el atributo que
 * declara el rubro nuevo, en vez de seguir buscando el del viejo.
 */
@Composable
fun CatalogoRoute(sesion: SessionRepository, onCerrar: () -> Unit) {
    val pack = packActual()
    val copy = copyCatalogo(pack)
    val ensure = usaEnsure(pack)
    val vm: CatalogoViewModel = viewModel(
        key = "catalogo:${copy.claveUnidad}:ensure=$ensure",
        factory = viewModelFactory {
            initializer {
                CatalogoViewModel(
                    sesion,
                    claveDeUnidad = copy.claveUnidad,
                    usaEnsure = ensure,
                )
            }
        },
    )
    CatalogoScreen(vm = vm, copy = copy, onCerrar = onCerrar)
}

/**
 * La pantalla que le faltaba al producto.
 *
 * Dos pasos y no un tablero, por lo mismo que Cobrar: en 360dp con la letra al
 * 200% una lista y un formulario a la vez no entran, y el que pierde siempre es
 * el formulario. Un paso a la vez ocupa el ancho completo y siempre cabe.
 *
 * La lista se lee como la mesa del puesto: cada cosa es una tarjeta con nombre
 * grande y precio numérico, no una grilla de inventario.
 */
@Composable
internal fun CatalogoScreen(
    vm: CatalogoViewModel,
    copy: CopyCatalogo,
    onCerrar: () -> Unit,
) {
    // Atrás desde el formulario vuelve a la lista en vez de cerrar la pantalla:
    // este handler se registra más adentro que el del contenedor, así que gana.
    BackHandler(enabled = vm.paso == PasoDelCatalogo.Formulario, onBack = vm::volverALaLista)

    when (vm.paso) {
        PasoDelCatalogo.Lista -> ListaDeCosas(vm = vm, copy = copy, onCerrar = onCerrar)
        PasoDelCatalogo.Formulario -> FormularioDeCosa(vm = vm, copy = copy)
    }
}

@Composable
private fun ListaDeCosas(
    vm: CatalogoViewModel,
    copy: CopyCatalogo,
    onCerrar: () -> Unit,
) {
    val dimens = RbTheme.dimens

    Column(modifier = Modifier.fillMaxSize()) {
        // Sin botón de acción en la barra: el alta vive en un solo lugar, grande
        // y abajo, donde está el pulgar. Repetirla arriba en chico agregaría un
        // segundo control a una barra que al 200% ya se parte en dos líneas.
        RbTopBar(
            title = copy.titulo,
            subtitle = copy.subtitulo,
            onBack = onCerrar,
        )

        // El buscador aparece sólo cuando hay algo que buscar. Con tres cosas
        // cargadas es un campo que ocupa alto y no resuelve nada; con cuarenta,
        // es lo primero que se toca. Queda visible mientras haya texto escrito,
        // para no dejar a nadie encerrado en un filtro sin campo para borrarlo.
        if (vm.cosas.size >= MINIMO_PARA_BUSCAR || vm.consulta.isNotBlank()) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = dimens.space3, vertical = dimens.space2),
            ) {
                RbTextField(
                    value = vm.consulta,
                    onValueChange = vm::cambiarConsulta,
                    label = copy.etiquetaBuscar,
                    placeholder = copy.placeholderBuscar,
                    imeAction = ImeAction.Search,
                    onImeAction = vm::cargar,
                )
            }
            RbDivider()
        }

        // Lo recién guardado, dicho arriba de la lista. Es la única confirmación
        // que hay: la fila nueva aparece entre otras treinta y sin esto no se
        // sabe si se guardó. Se toca y se va.
        vm.recienGuardado?.let { guardado ->
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = dimens.space3, vertical = dimens.space2),
            ) {
                RbChip(
                    label = copyConfirmacionGuardado(
                        nombre = guardado,
                        feria = copy.esFeria,
                    ),
                    tone = RbChipTone.Brand,
                    onClick = vm::olvidarConfirmacion,
                )
            }
        }

        // Mesa del puesto: vacío con aire y un solo CTA; tarjetas cuando hay
        // cosas; loading/error con el mismo lenguaje del resto de la app.
        // No se reusa RbList acá porque esa fila trae divisor a borde y no da
        // el aire de tarjeta que pide la mesa.
        when {
            vm.errorDeLista != null -> {
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(dimens.space3),
                ) {
                    RbErrorState(
                        title = vm.errorDeLista!!.title,
                        message = vm.errorDeLista!!.message,
                        retryLabel = vm.errorDeLista!!.retryLabel,
                        onRetry = vm::cargar,
                    )
                }
            }

            vm.cargando && vm.cosas.isEmpty() -> {
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                ) {
                    RbLoadingState(
                        label = copyCargandoLista(copy.titulo),
                    )
                    RbSkeletonLines(
                        lines = 5,
                        modifier = Modifier.padding(dimens.space3),
                    )
                }
            }

            vm.cosas.isEmpty() -> {
                // `Box` con `contentAlignment = Center` y no un `Column` que
                // sólo scrollea: así el vacío se centra en toda la altura que
                // le tocó en vez de pegarse arriba con aire muerto abajo — la
                // "tarjeta arriba, resto en blanco liso" que encontró la
                // auditoría de capturas. Sigue siendo scrolleable para que al
                // 200% el botón no quede atrapado bajo el teclado o el borde.
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState()),
                    contentAlignment = Alignment.Center,
                ) {
                    if (vm.consulta.isBlank()) {
                        VacioDelCatalogo(
                            titulo = copy.vacioTitulo,
                            beneficio = copy.vacioBeneficio,
                            pista = copy.vacioPista,
                            accion = copy.agregar,
                            onAccion = vm::nuevaCosa,
                        )
                    } else {
                        VacioDelCatalogo(
                            titulo = copyVacioBusqueda(vm.consulta),
                            pista = copy.vacioBusquedaPista,
                            accion = copy.agregar,
                            onAccion = vm::nuevaCosa,
                        )
                    }
                }
            }

            else -> {
                LazyColumn(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                    contentPadding = PaddingValues(
                        horizontal = dimens.space3,
                        vertical = dimens.space2,
                    ),
                    verticalArrangement = Arrangement.spacedBy(dimens.space2),
                ) {
                    items(items = vm.cosas, key = { it.id }) { cosa ->
                        FilaDeCosa(
                            cosa = cosa,
                            unidad = vm.unidadDe(cosa),
                            precio = vm.moneda.formatear(cosa.price),
                            onEditar = { vm.editar(cosa) },
                        )
                    }

                    // Con una o dos cosas cargadas, la tarjeta ocupa una
                    // fracción chica del alto que le tocó y el resto queda en
                    // blanco hasta el botón anclado — el otro caso que marcó
                    // la auditoría ("una pantalla con un solo ítem también
                    // deja el 80% en blanco"). No es un segundo CTA — el botón
                    // que llena esto ya está abajo — sino la razón para
                    // seguir tocándolo.
                    if (vm.cosas.size < UMBRAL_SUGERIR_MAS) {
                        item("sugerencia-pocas-cosas") {
                            SugerenciaPocasCosas(texto = copy.sugerenciaPocasCosas)
                        }
                    }
                }

                // Con la lista llena el botón sigue abajo y anclado: cargar el
                // puesto es repetir este gesto veinte veces seguidas.
                RbDivider()
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(dimens.space3),
                ) {
                    RbButton(
                        label = copy.agregar,
                        onClick = vm::nuevaCosa,
                        fillWidth = true,
                    )
                }
            }
        }
    }
}

/**
 * Vacío de la mesa: la marca de [RbIcons.catalogoContorno], el título de lo
 * que falta, [beneficio] opcional (el «para qué» de cargarlo) y la pista que
 * enseña cómo (feria: «vendí tomates a 2000»), con un solo CTA primario. Sin
 * segunda acción ni ruido de sistema.
 *
 * Delegado por completo en [RbEmptyState] — antes esta función redibujaba a
 * mano el mismo mark/título/pista/botón que ya vive en `:design`, con un "+"
 * de texto en vez de un ícono real. Mantiene su propia firma (y no la de
 * [RbEmptyState] directamente) porque `CatalogoEscalaTest` la monta por
 * nombre para medir el CTA sin levantar toda la pantalla.
 */
@Composable
internal fun VacioDelCatalogo(
    titulo: String,
    pista: String,
    accion: String,
    onAccion: () -> Unit,
    modifier: Modifier = Modifier,
    beneficio: String? = null,
) {
    RbEmptyState(
        title = titulo,
        modifier = modifier,
        icon = RbIcons.catalogoContorno,
        benefit = beneficio,
        hint = pista,
        actionLabel = accion,
        onAction = onAccion,
    )
}

/**
 * Una cosa en la mesa del puesto.
 *
 * Nombre grande, precio numérico, superficie de tarjeta, mínimo 56dp. El
 * subtítulo dice **cómo se vende** y no cuánto stock hay: en un puesto el stock
 * no se cuenta y un "1 disponible" al lado de algo recién cargado se lee como
 * error. "Por kilo" es lo que hay que confirmar de un vistazo.
 */
@Composable
internal fun FilaDeCosa(
    cosa: ProductDto,
    unidad: String,
    precio: String,
    onEditar: () -> Unit,
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
            .rbClickable(
                onClick = onEditar,
                role = Role.Button,
                shape = shape,
            )
            .rbTouchTarget()
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        content = {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
                Text(
                    text = cosa.name,
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                )
                if (unidad.isNotBlank()) {
                    Text(
                        text = copyComoSeVende(unidad),
                        style = RbTheme.typography.support,
                        color = colors.textSecondary,
                    )
                }
            }
        },
        trailing = {
            RbAmount(
                amount = precio,
                emphasis = RbAmountEmphasis.Body,
            )
        },
    )
}

/**
 * El hueco entre la última tarjeta y el botón anclado, con un motivo en vez
 * de aire: no repite el CTA (el botón que llena esto ya está abajo, siempre
 * visible), sólo dice por qué seguir. Ícono chico y texto de apoyo — no
 * compite con las tarjetas de arriba, que son el contenido real.
 */
@Composable
private fun SugerenciaPocasCosas(texto: String, modifier: Modifier = Modifier) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    RbReflowRow(
        spacing = dimens.space2,
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surfaceVariant)
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        leading = {
            Icon(
                imageVector = RbIcons.catalogoContorno,
                contentDescription = null,
                tint = colors.textSecondary,
                modifier = Modifier.size(dimens.iconSize),
            )
        },
        content = {
            Text(
                text = texto,
                style = RbTheme.typography.support,
                color = colors.textSecondary,
            )
        },
    )
}

@Composable
private fun FormularioDeCosa(vm: CatalogoViewModel, copy: CopyCatalogo) {
    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = if (vm.editando == null) copy.tituloAlta else copy.tituloEdicion,
            subtitle = if (vm.editando == null) copy.subtituloAlta else null,
            onBack = vm::volverALaLista,
        )

        FormularioContenido(
            copy = copy,
            nombre = vm.nombre,
            onNombre = vm::cambiarNombre,
            errorDeNombre = vm.errorDeNombre,
            precio = vm.precio,
            onPrecio = vm::cambiarPrecio,
            errorDePrecio = vm.errorDePrecio,
            simbolo = vm.moneda.simbolo,
            unidad = vm.unidad,
            onUnidad = vm::cambiarUnidad,
            onElegirUnidad = vm::elegirUnidad,
            guardando = vm.guardando,
            errorAlGuardar = vm.errorAlGuardar,
            onGuardar = vm::guardar,
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * El formulario: nombre, precio y —si el pack lo pide— unidad.
 *
 * Sin `ViewModel` a propósito. Todo lo que decide el layout entra por parámetro
 * para poder medirlo en una prueba —que es donde vive la escala al 200% y el
 * panel recortado por el teclado— sin montar red ni sesión.
 *
 * Scrollea entero porque con el teclado abierto en un panel de 640dp quedan unos
 * 320dp, y al 200% tres campos con sus rótulos no entran ahí. El botón de
 * guardar va **dentro** del scroll y no anclado abajo: es el final del
 * formulario, y anclarlo le quitaría a los campos el poco alto que les queda
 * justo mientras se escribe.
 *
 * Jerarquía calmada: nombre y precio son el camino feliz; la unidad va después
 * con menos ruido de ayuda, para que no compita con lo que hay que llenar sí o sí.
 */
@Composable
internal fun FormularioContenido(
    copy: CopyCatalogo,
    nombre: String,
    onNombre: (String) -> Unit,
    errorDeNombre: String?,
    precio: String,
    onPrecio: (String) -> Unit,
    errorDePrecio: String?,
    simbolo: String,
    unidad: String,
    onUnidad: (String) -> Unit,
    onElegirUnidad: (String) -> Unit,
    guardando: Boolean,
    errorAlGuardar: RbErrorCopy?,
    onGuardar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space4),
    ) {
        // Camino feliz: nombre + precio, sin párrafos de ayuda compitiendo.
        Column(verticalArrangement = Arrangement.spacedBy(dimens.space3)) {
            RbTextField(
                value = nombre,
                onValueChange = onNombre,
                label = copy.etiquetaNombre,
                placeholder = copy.ejemploNombre,
                errorMessage = errorDeNombre,
                enabled = !guardando,
                imeAction = ImeAction.Next,
            )

            RbTextField(
                value = precio,
                onValueChange = onPrecio,
                label = copy.etiquetaPrecio,
                placeholder = "0",
                supportingText = "En $simbolo",
                errorMessage = errorDePrecio,
                enabled = !guardando,
                // Monoespaciado y teclado numérico: los dígitos de la plata se leen
                // en columna y el teclado abre directo en números.
                numeric = true,
                keyboardType = KeyboardType.Number,
            )
        }

        // El campo de unidad existe sólo si el pack lo declara. En farmacia o
        // minimarket el `AttrField` no está y la pantalla no lo inventa.
        if (copy.hayUnidad) {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
                RbTextField(
                    value = unidad,
                    onValueChange = onUnidad,
                    label = copy.etiquetaUnidad,
                    placeholder = PLACEHOLDER_UNIDAD,
                    supportingText = AYUDA_UNIDAD_OPCIONAL,
                    enabled = !guardando,
                    imeAction = ImeAction.Done,
                )

                // Sugerencias, no un menú cerrado: la señora que vende huevos
                // escribe "bandeja" y ninguna lista fija la habría tenido. Los
                // chips llenan el campo de arriba; el campo manda.
                RbChipRow {
                    UNIDADES_SUGERIDAS.forEach { sugerida ->
                        val elegida = unidad.equals(sugerida, ignoreCase = true)
                        RbChip(
                            label = sugerida,
                            tone = if (elegida) RbChipTone.Brand else RbChipTone.Neutral,
                            selected = elegida,
                            onClick = if (guardando) null else ({ onElegirUnidad(sugerida) }),
                        )
                    }
                }
            }
        }

        errorAlGuardar?.let { error ->
            RbErrorState(
                title = error.title,
                message = error.message,
                retryLabel = error.retryLabel,
                onRetry = if (error.retryLabel != null) onGuardar else null,
            )
        }

        RbButton(
            label = if (guardando) ETIQUETA_GUARDANDO else ETIQUETA_GUARDAR,
            onClick = onGuardar,
            enabled = !guardando,
            fillWidth = true,
        )

        // Lo que la pantalla no pide, dicho en voz baja. Sin esto el formulario
        // se lee como incompleto —"¿y el código de barras?"— y quien viene de
        // otro sistema se queda esperando más campos. Feria no nombra el código.
        Text(
            text = copy.pieFormulario,
            style = RbTheme.typography.support,
            color = colors.textSecondary,
        )
    }
}

/** Desde cuántas filas aparece el buscador. */
private const val MINIMO_PARA_BUSCAR = 8

/** Bajo cuántas filas se ofrece la sugerencia de seguir cargando. */
private const val UMBRAL_SUGERIR_MAS = 3
