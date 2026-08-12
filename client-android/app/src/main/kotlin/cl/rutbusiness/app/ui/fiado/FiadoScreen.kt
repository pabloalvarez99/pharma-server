package cl.rutbusiness.app.ui.fiado

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.app.ui.agente.ASI_SE_FIA
import cl.rutbusiness.app.ui.agente.irAlAgente
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.offline.Fechado
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbAmount
import cl.rutbusiness.ui.components.RbAmountEmphasis
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme

@Composable
fun FiadoRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.Activa,
    onVolver: () -> Unit,
) {
    val offline = LocalOffline.current
    val vm: FiadoViewModel = viewModel(
        key = "fiado:${estado.baseUrl}",
        factory = viewModelFactory { initializer { FiadoViewModel(sesion, offline) } },
    )
    FiadoScreen(vm = vm, onVolver = onVolver)
}

/**
 * "¿Quién me debe?".
 *
 * Es la otra pregunta de todos los días, y la pantalla la contesta en el orden
 * en que importa: el total arriba, después los nombres ordenados por cuánto
 * deben. El buscador queda fijo sobre la lista porque el caso de uso es alguien
 * parado en el mostrador con la plata en la mano — tener que scrollear hasta el
 * buscador es exactamente el momento en que se pierde.
 *
 * Regla de plata: ni el total ni los saldos se suman acá. Vienen del server.
 */
@Composable
private fun FiadoScreen(vm: FiadoViewModel, onVolver: () -> Unit) {
    val dimens = RbTheme.dimens

    val atras: () -> Unit = when (vm.paso) {
        PasoDeFiado.Detalle -> vm::volverALaLista
        PasoDeFiado.Abono -> vm::volverAlDetalle
        PasoDeFiado.Lista -> onVolver
    }
    BackHandler(onBack = atras)

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = when (vm.paso) {
                PasoDeFiado.Lista -> "Quién me debe"
                PasoDeFiado.Detalle -> vm.elegido?.name.orEmpty().ifBlank { "La cuenta" }
                PasoDeFiado.Abono -> "Me está pagando"
            },
            subtitle = when (vm.paso) {
                // Cuando la lista salió del teléfono, la fecha va en el
                // subtítulo: los saldos son plata, y un saldo viejo sin fecha
                // hace que la dueña cobre de menos o de más.
                PasoDeFiado.Lista -> vm.guardadoEn?.let { guardadoEn ->
                    "Saldos guardados en el teléfono, " +
                        Fechado(Unit, guardadoEn).antiguedad(vm.relojDePared)
                } ?: "El fiado del negocio"
                PasoDeFiado.Detalle -> "Lo que se llevó y lo que fue pagando"
                PasoDeFiado.Abono -> vm.elegido?.name
            },
            onBack = atras,
            actions = {
                if (vm.paso == PasoDeFiado.Lista) {
                    RbButton(
                        label = "Actualizar",
                        onClick = vm::cargar,
                        variant = RbButtonVariant.Secondary,
                        enabled = !vm.cargando,
                    )
                }
            },
        )

        val fatal = vm.error
        when {
            fatal != null && vm.paso == PasoDeFiado.Lista -> RbErrorState(
                title = fatal.title,
                message = fatal.message,
                modifier = Modifier.padding(dimens.space3),
                retryLabel = fatal.retryLabel,
                onRetry = if (fatal.retryLabel != null) vm::cargar else null,
            )

            vm.cargando && vm.deudores == null -> Column {
                RbLoadingState(label = "Viendo quién te debe...")
                RbSkeletonLines(lines = 5, modifier = Modifier.padding(dimens.space3))
            }

            else -> when (vm.paso) {
                PasoDeFiado.Lista -> ListaDeDeudores(
                    moneda = vm.moneda,
                    deudores = vm.deudores,
                    visibles = vm.deudoresVisibles,
                    consulta = vm.consulta,
                    onConsulta = vm::cambiarConsulta,
                    onElegir = vm::abrirCuenta,
                )

                PasoDeFiado.Detalle -> DetalleDeCuenta(vm)
                PasoDeFiado.Abono -> PasoAbono(vm)
            }
        }
    }
}

/**
 * La lista de quién debe, sin `ViewModel` detrás para poder medirla.
 *
 * `LazyColumn` y no una columna que scrollea: los deudores de un almacén que
 * lleva años fiando son cientos, y el piso de hardware presupuesta 1-2 GB de RAM
 * para todo el aparato.
 *
 * @param deudores lo que mandó el server, con su total ya sumado. `null`
 *   mientras no llegó.
 * @param visibles las filas que pasan el filtro, en el orden del server.
 */
@Composable
internal fun ListaDeDeudores(
    moneda: Moneda,
    deudores: DeudoresDto?,
    visibles: List<DeudorDto>,
    consulta: String,
    onConsulta: (String) -> Unit,
    onElegir: (DeudorDto) -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val feria = esFeria()
    val alAgente = irAlAgente()

    Column(modifier = modifier.fillMaxSize()) {
        // Fijo sobre la lista y no como primer ítem de ella: con cincuenta
        // deudores, un buscador que se scrollea se pierde justo cuando alguien
        // está esperando en el mostrador. Además, quedando arriba, el teclado
        // tapa la lista y nunca el campo donde se escribe.
        RbTextField(
            value = consulta,
            onValueChange = onConsulta,
            label = "Buscar a quien vino a pagar",
            placeholder = "Nombre o teléfono",
            modifier = Modifier.padding(dimens.space3),
            keyboardType = KeyboardType.Text,
            imeAction = ImeAction.Search,
        )

        when {
            deudores == null -> Unit

            // El vacío que más veces se va a ver: el primer día nadie debe nada.
            // En feria tiene que enseñar **cómo se fía**, que es una frase y no
            // un formulario; el botón lleva a decirla y desaparece si desde acá
            // no se puede llegar al agente.
            deudores.cuantos == 0 || esCero(deudores.total) -> RbEmptyState(
                title = if (feria) "Nadie te debe" else "Nadie te debe plata",
                hint = if (feria) {
                    "Cuando fíes, díselo al agente: «$ASI_SE_FIA». Queda acá hasta que te pague."
                } else {
                    "Cuando fíes una venta, la deuda de esa persona aparece acá hasta que " +
                        "te la termine de pagar."
                },
                actionLabel = if (feria) alAgente?.let { "Hablarle al agente" } else null,
                onAction = if (feria) alAgente else null,
            )

            visibles.isEmpty() -> RbEmptyState(
                title = "Nadie con ese nombre",
                hint = "Revisa cómo lo escribiste, o borra la búsqueda para ver a todos los " +
                    "que te deben.",
                actionLabel = "Ver a todos",
                onAction = { onConsulta("") },
            )

            // `weight(1f)` y no `fillMaxSize()`: la lista es hermana del
            // buscador dentro de esta columna, y `fillMaxSize` le pediría el
            // alto completo sin descontar lo que el buscador ya ocupó — se
            // dibujaría encima de él. Con la letra al 200% y el teclado abierto
            // eso deja de ser teórico.
            else -> LazyColumn(
                modifier = Modifier.weight(1f),
                contentPadding = PaddingValues(
                    start = dimens.space3,
                    end = dimens.space3,
                    bottom = dimens.space3,
                ),
                verticalArrangement = Arrangement.spacedBy(dimens.space2),
            ) {
                item("total") { TotalPorCobrar(moneda = moneda, deudores = deudores) }

                items(items = visibles, key = { it.customer }) { deudor ->
                    RbCard {
                        RbListRow(
                            title = deudor.name,
                            subtitle = subtituloDeDeudor(deudor),
                            value = moneda.formatear(deudor.balance),
                            onClick = { onElegir(deudor) },
                        )
                    }
                }

                if (consulta.isNotBlank()) {
                    item("cuantos-se-ven") {
                        Text(
                            text = "Se ven ${visibles.size} de ${deudores.cuantos}.",
                            style = RbTheme.typography.support,
                            color = colors.textSecondary,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun TotalPorCobrar(moneda: Moneda, deudores: DeudoresDto) {
    val colors = RbTheme.colors

    RbCard(title = "Te deben en total") {
        RbAmount(
            amount = moneda.formatear(deudores.total),
            emphasis = RbAmountEmphasis.Headline,
        )
        Text(
            text = if (deudores.cuantos == 1) {
                "1 persona con cuenta pendiente."
            } else {
                "${deudores.cuantos} personas con cuenta pendiente."
            },
            style = RbTheme.typography.support,
            color = colors.textSecondary,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

private fun subtituloDeDeudor(deudor: DeudorDto): String {
    val telefono = deudor.phone?.takeIf { it.isNotBlank() }
    val fecha = fechaCorta(deudor.ultimoMovimiento)
    // "última vez" y no "último movimiento": es más corto -- la fila entra en un
    // renglón en vez de dos junto al teléfono -- y además es como lo diría una
    // persona. El encargo pide cero jerga contable.
    return listOfNotNull(
        telefono,
        fecha?.let { "última vez: $it" },
    ).joinToString(" · ").ifBlank { "Sin más datos" }
}

/**
 * La fecha de un instante ISO-8601, en el orden en que se lee en Chile.
 *
 * Se recorta la cadena en vez de usar `java.time`: la API de fechas de Java sólo
 * existe desde Android 8 y el piso de hardware de este producto es Android 5.
 * Traer el desugaring completo por mostrar tres números no se justifica.
 */
internal fun fechaCorta(iso: String): String? {
    val fecha = iso.substringBefore('T')
    val partes = fecha.split('-')
    if (partes.size != 3) return null
    if (partes.any { parte -> parte.isEmpty() || parte.any { !it.isDigit() } }) return null
    return "${partes[2]}-${partes[1]}-${partes[0]}"
}

/**
 * Si un monto del server es cero.
 *
 * Se pregunta por el valor y no por el texto: el server puede mandar `"0"`,
 * `"0.00"` o `"0.0000"` para la misma nada.
 */
internal fun esCero(montoDelServidor: String): Boolean =
    Dinero.deTextoDeServidor(montoDelServidor)?.let { it.unidades == 0L } ?: false
