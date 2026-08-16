package cl.rutbusiness.app.ui.fiado

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.agente.ASI_SE_FIA
import cl.rutbusiness.app.ui.agente.irAlAgente
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda
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
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbReflowRow
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbClickable
import cl.rutbusiness.ui.theme.rbHeading
import cl.rutbusiness.ui.theme.rbTouchTarget

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
 * Se lee como gente que te debe, no como un ledger: nombres grandes, plata a
 * la vista, vacío que enseña la frase del fiado.
 *
 * Regla de plata: ni el total ni los saldos se suman acá. Vienen del server.
 */
@Composable
private fun FiadoScreen(vm: FiadoViewModel, onVolver: () -> Unit) {
    val dimens = RbTheme.dimens

    // Feria: al entrar a Fiado se abre el puesto con $0 (o 409 = ya abierto).
    // La Screen lee esFeria(); el VM no puede tocar CompositionLocal.
    val feria = esFeria()
    LaunchedEffect(feria) {
        if (feria) vm.modoFeria(true)
    }

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
                    "Guardado en el teléfono, " +
                        Fechado(Unit, guardadoEn).antiguedad(vm.relojDePared)
                } ?: "Gente que te debe"
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
    val sinDeuda = deudores != null && (deudores.cuantos == 0 || esCero(deudores.total))

    Column(modifier = modifier.fillMaxSize()) {
        // Con gente que debe, el buscador queda fijo: alguien viene a pagar y
        // no se puede perder scrolleando. Sin deuda no hay a quién filtrar —
        // el vacío enseña a fiar, no a buscar.
        if (!sinDeuda) {
            RbTextField(
                value = consulta,
                onValueChange = onConsulta,
                label = "Buscar a quien vino a pagar",
                placeholder = "Nombre o teléfono",
                modifier = Modifier.padding(dimens.space3),
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Search,
            )
        }

        when {
            deudores == null -> Unit

            // El vacío del primer día: nadie debe. Se lee como gente (no como
            // ledger en cero). En feria enseña la frase completa del fiado.
            sinDeuda -> Column(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(dimens.space3),
            ) {
                VacioDeFiado(
                    feria = feria,
                    onHablarleAlAgente = if (feria) alAgente else null,
                )
            }

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
                // space3 entre tarjetas: se leen de a una persona, no como
                // filas de un extracto.
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                item("total") { TotalPorCobrar(moneda = moneda, deudores = deudores) }

                items(items = visibles, key = { it.customer }) { deudor ->
                    FilaDeDeudor(
                        nombre = deudor.name,
                        detalle = subtituloDeDeudor(deudor),
                        saldo = moneda.formatear(deudor.balance),
                        onClick = { onElegir(deudor) },
                    )
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

/**
 * Nadie te debe todavía.
 *
 * Misma vara que el vacío de "Hoy": aire, un ejemplo hablado (feria) y un solo
 * CTA. Sin asset de ilustración — el APK del teléfono viejo no paga eso.
 */
@Composable
internal fun VacioDeFiado(
    feria: Boolean,
    onHablarleAlAgente: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    RbCard(modifier = modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = dimens.space4),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            Text(
                text = if (feria) "Nadie te debe" else "Nadie te debe plata",
                style = RbTheme.typography.heading,
                color = colors.textPrimary,
                textAlign = TextAlign.Center,
                modifier = Modifier.rbHeading(),
            )
            Text(
                text = if (feria) {
                    "Dile al agente: «$ASI_SE_FIA». " +
                        "Queda acá hasta que te pague."
                } else {
                    "Cuando fíes una venta, la deuda de esa persona aparece acá hasta que " +
                        "te la termine de pagar."
                },
                style = RbTheme.typography.body,
                color = colors.textSecondary,
                textAlign = TextAlign.Center,
            )
            if (onHablarleAlAgente != null) {
                RbButton(
                    label = "Hablarle al agente",
                    onClick = onHablarleAlAgente,
                )
            }
        }
    }
}

/**
 * Una persona que te debe.
 *
 * Nombre grande, plata numérica, tarjeta entera tocable (≥56 dp). No es una
 * fila de extracto: es gente del barrio con un monto.
 */
@Composable
internal fun FilaDeDeudor(
    nombre: String,
    detalle: String,
    saldo: String,
    onClick: () -> Unit,
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
                onClick = onClick,
                role = Role.Button,
                shape = shape,
            )
            .rbTouchTarget()
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        content = {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space1)) {
                Text(
                    text = nombre,
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                )
                Text(
                    text = detalle,
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }
        },
        trailing = {
            RbAmount(
                amount = saldo,
                emphasis = RbAmountEmphasis.Body,
            )
        },
    )
}

@Composable
private fun TotalPorCobrar(moneda: Moneda, deudores: DeudoresDto) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens

    // Franja humana, no ERP: plata de personas, no "cuentas por cobrar".
    RbCard(title = "Te deben") {
        RbAmount(
            amount = moneda.formatear(deudores.total),
            emphasis = RbAmountEmphasis.Headline,
            modifier = Modifier.padding(vertical = dimens.space1),
        )
        Text(
            text = if (deudores.cuantos == 1) {
                "1 persona te debe."
            } else {
                "${deudores.cuantos} personas te deben."
            },
            style = RbTheme.typography.body,
            color = colors.textSecondary,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

private fun subtituloDeDeudor(deudor: DeudorDto): String {
    val telefono = deudor.phone?.takeIf { it.isNotBlank() }
    val fecha = fechaCorta(deudor.ultimoMovimiento)
    // "Vino el …" y no "último movimiento": es cómo se dice en el puesto, no
    // en un extracto. Junto al teléfono entra en un renglón.
    return listOfNotNull(
        telefono,
        fecha?.let { "vino el $it" },
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
