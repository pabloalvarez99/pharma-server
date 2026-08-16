package cl.rutbusiness.app.ui.catalogo

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipRow
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorCopy
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbList
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme

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
            subtitle = "Lo que puedes cobrar",
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
                    label = "Buscar",
                    placeholder = "Nombre",
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
                    label = "Guardado: $guardado",
                    tone = RbChipTone.Brand,
                    onClick = vm::olvidarConfirmacion,
                )
            }
        }

        RbList(
            items = vm.cosas,
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
            loading = vm.cargando && vm.cosas.isEmpty(),
            loadingLabel = "Cargando ${copy.titulo.replaceFirstChar { it.lowercase() }}",
            error = vm.errorDeLista,
            onRetry = vm::cargar,
            emptyTitle = if (vm.consulta.isBlank()) {
                copy.vacioTitulo
            } else {
                "Nada con \"${vm.consulta.trim()}\""
            },
            emptyHint = if (vm.consulta.isBlank()) {
                copy.vacioPista
            } else {
                "Fíjate cómo se escribe, o agrégalo si todavía no está."
            },
            // El vacío no es una lista vacía: es la pantalla diciendo qué hacer,
            // con el botón grande adentro. `RbEmptyState` ya lo dibuja primario.
            emptyActionLabel = copy.agregar,
            onEmptyAction = vm::nuevaCosa,
            key = { it.id },
        ) { cosa ->
            FilaDeCosa(
                cosa = cosa,
                unidad = vm.unidadDe(cosa),
                precio = vm.moneda.formatear(cosa.price),
                onEditar = { vm.editar(cosa) },
            )
        }

        // Con la lista llena el botón sigue abajo y anclado: cargar el puesto es
        // repetir este gesto veinte veces seguidas, y mandarlo a una barra de
        // arriba costaría un viaje del pulgar por cada alta.
        if (vm.cosas.isNotEmpty()) {
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

/**
 * Una fila.
 *
 * El subtítulo dice **cómo se vende** y no cuánto stock hay. En un puesto el
 * stock no se cuenta —nadie sabe cuántos tomates quedan— y un "1 disponible" al
 * lado de algo recién cargado es ruido que además se lee como un error. "Por
 * kilo" es lo que hay que confirmar de un vistazo.
 */
@Composable
private fun FilaDeCosa(
    cosa: ProductDto,
    unidad: String,
    precio: String,
    onEditar: () -> Unit,
) {
    RbListRow(
        title = cosa.name,
        subtitle = if (unidad.isNotBlank()) "Por $unidad" else null,
        value = precio,
        onClick = onEditar,
    )
}

@Composable
private fun FormularioDeCosa(vm: CatalogoViewModel, copy: CopyCatalogo) {
    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = if (vm.editando == null) copy.tituloAlta else copy.tituloEdicion,
            subtitle = if (vm.editando == null) "Con esto ya lo puedes cobrar" else null,
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
 * El formulario: tres campos y un botón.
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

    Column(
        modifier = modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
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
            supportingText = "En $simbolo. Después lo puedes cambiar cuando quieras.",
            errorMessage = errorDePrecio,
            enabled = !guardando,
            // Monoespaciado y teclado numérico: los dígitos de la plata se leen
            // en columna y el teclado abre directo en números.
            numeric = true,
            keyboardType = KeyboardType.Number,
        )

        // El campo de unidad existe sólo si el pack lo declara. En farmacia o
        // minimarket el `AttrField` no está y la pantalla no lo inventa.
        if (copy.hayUnidad) {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
                RbTextField(
                    value = unidad,
                    onValueChange = onUnidad,
                    label = copy.etiquetaUnidad,
                    placeholder = "kilo, atado, bolsa…",
                    supportingText = "Toca una o escribe la tuya. También puedes dejarlo vacío.",
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
            label = if (guardando) "Guardando…" else "Guardar",
            onClick = onGuardar,
            enabled = !guardando,
            fillWidth = true,
        )

        // Lo que la pantalla no pide, dicho para que nadie lo vaya a buscar. Sin
        // esto el formulario se lee como incompleto —"¿y el código de barras?"—
        // y quien viene de otro sistema se queda esperando más campos.
        Text(
            text = "Con el nombre y el precio alcanza para cobrar. " +
                "No hace falta código de barras.",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )
    }
}

/** Desde cuántas filas aparece el buscador. */
private const val MINIMO_PARA_BUSCAR = 8
