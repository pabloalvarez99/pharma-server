package cl.rutbusiness.app.ui.menu

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import cl.rutbusiness.app.ui.caja.CajaRoute
import cl.rutbusiness.app.ui.entrada.LocalEntrada
import cl.rutbusiness.app.ui.impresora.EstadoDeImpresion
import cl.rutbusiness.app.ui.impresora.TarjetaDeReimpresion
import cl.rutbusiness.app.ui.impresora.impresoraViewModel
import cl.rutbusiness.app.ui.rubro.LocalRubro
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.rubro.SettingsRubroApi
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbConfirmDialog
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import kotlinx.coroutines.launch

/**
 * "Tu puesto" / "Más opciones" — la puerta a lo que ya funciona pero no tenía
 * ningún camino desde la interfaz.
 *
 * La auditoría de la ola encontró código probado y funcionando sin ninguna
 * forma de llegar: la impresora (sólo aparecía después de cerrar una venta,
 * nunca antes), la caja en feria (el resumen la esconde a propósito, ver
 * `ordenDeBloquesHoy` en `CopyResumen.kt`), las ventas que faltan subir
 * (`FranjaDeConexion` sólo se dibuja cuando hay algo pendiente o falla la
 * conexión — con todo al día, no hay ningún botón) y cambiar de rubro
 * (`ElegirRubro` sólo se llama en el alta; su propia frase promete "Después
 * se puede cambiar" y nunca lo cumple).
 *
 * Lo que **no** está acá porque ya tiene camino: quién me debe (siempre en
 * "Hoy") y el escáner (adentro de Cobrar). Meterlos también sería llenar el
 * menú de puertas repetidas.
 *
 * No es una pestaña ni una pantalla de ajustes de sistema: es la cuarta
 * rutina de "Tu día", exactamente al lado de la caja y el fiado
 * (`TuDiaRoute.kt`), así que no hace falta un cuarto destino en la barra de
 * abajo — que además está tope en tres (`BarraDeNavegacion.kt`).
 *
 * @param onVolver vuelve a "Hoy". Lo pasa `TuDiaRoute`, igual que en caja y
 *   fiado.
 */
@Composable
fun MenuDelPuestoRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.Activa,
    onVolver: () -> Unit,
) {
    var seccion by rememberSaveable { mutableStateOf(SeccionDelMenu.Lista) }
    val volverALista: () -> Unit = { seccion = SeccionDelMenu.Lista }

    when (seccion) {
        SeccionDelMenu.Lista -> ListaDelMenu(
            onVolver = onVolver,
            onAbrirCaja = { seccion = SeccionDelMenu.Caja },
            onAbrirImpresora = { seccion = SeccionDelMenu.Impresora },
            onAbrirRubro = { seccion = SeccionDelMenu.Rubro },
        )

        SeccionDelMenu.Caja -> CajaRoute(sesion = sesion, estado = estado, onVolver = volverALista)

        SeccionDelMenu.Impresora -> PantallaDeImpresoraDelMenu(onVolver = volverALista)

        SeccionDelMenu.Rubro -> PantallaDeCambiarRubro(sesion = sesion, onVolver = volverALista)
    }
}

/** Las cuatro secciones del menú. `Lista` es la portada. */
private enum class SeccionDelMenu { Lista, Caja, Impresora, Rubro }

/** Una fila tocable del menú. */
private data class EntradaDeMenu(
    val titulo: String,
    val subtitulo: String,
    val onClick: () -> Unit,
)

/**
 * La portada: la lista de puertas.
 *
 * Gatea por pack (ADR-0022): la impresora sólo se ofrece cuando el rubro la
 * trae (`features.printer`, `false` en feria), y "ventas que faltan subir"
 * sólo cuando [abrirVentasPendientes] devolvió algo — si `ContenedorDeDestinos`
 * todavía no provee [LocalAbrirVentasPendientes], la entrada simplemente no
 * se dibuja, en vez de ofrecer un botón que no lleva a ninguna parte.
 */
@Composable
private fun ListaDelMenu(
    onVolver: () -> Unit,
    onAbrirCaja: () -> Unit,
    onAbrirImpresora: () -> Unit,
    onAbrirRubro: () -> Unit,
) {
    val dimens = RbTheme.dimens
    val feria = esFeria()
    val pack = packActual()
    val abrirPendientes = abrirVentasPendientes()

    val entradas = remember(feria, pack.features.printer, abrirPendientes) {
        buildList {
            add(EntradaDeMenu(tituloCaja(feria), subtituloCaja(feria), onAbrirCaja))
            if (pack.features.printer) {
                add(EntradaDeMenu(tituloImpresora(), subtituloImpresora(), onAbrirImpresora))
            }
            if (abrirPendientes != null) {
                add(EntradaDeMenu(tituloVentasPendientes(), subtituloVentasPendientes(), abrirPendientes))
            }
            add(EntradaDeMenu(tituloRubro(feria), subtituloRubro(), onAbrirRubro))
        }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(title = tituloMenu(feria), subtitle = subtituloMenu(), onBack = onVolver)
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(dimens.space3),
        ) {
            RbCard {
                entradas.forEachIndexed { indice, entrada ->
                    RbListRow(
                        title = entrada.titulo,
                        subtitle = entrada.subtitulo,
                        onClick = entrada.onClick,
                    )
                    if (indice != entradas.lastIndex) RbDivider()
                }
            }
        }
    }
}

/**
 * La impresora, abierta desde el menú en vez de después de una venta.
 *
 * Reusa por completo [TarjetaDeReimpresion] — misma tarjeta, mismo
 * `ImpresoraViewModel` compartido con la pantalla de comprobante — así que no
 * hay una segunda copia del estado de emparejar/elegir ancho/reintentar. Al
 * entrar, si el estado está en reposo se dispara [ImpresoraViewModel.cambiarImpresora]
 * una vez: eso abre la lista de emparejadas de una, en vez de mostrar primero
 * la tarjeta de "reimprimir la última" — la dueña vino a configurar la
 * impresora, no a reimprimir una boleta vieja.
 */
@Composable
private fun PantallaDeImpresoraDelMenu(onVolver: () -> Unit) {
    val dimens = RbTheme.dimens
    val vm = impresoraViewModel()

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(title = tituloImpresora(), subtitle = subtituloImpresora(), onBack = onVolver)

        if (vm == null) {
            Column(modifier = Modifier.padding(dimens.space3)) {
                RbErrorState(
                    title = "No se pudo abrir la impresora",
                    message = "Este teléfono no tiene la impresora conectada ahora.",
                    retryLabel = null,
                    onRetry = null,
                )
            }
            return@Column
        }

        LaunchedEffect(Unit) {
            if (vm.estado == EstadoDeImpresion.Reposo) vm.cambiarImpresora()
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(dimens.space3),
        ) {
            TarjetaDeReimpresion(vm = vm, onCerrar = onVolver)
        }
    }
}

/**
 * Cambiar de rubro después del alta.
 *
 * `ElegirRubro` sólo vive en el alta y no habla con el server (guarda en
 * preferencias locales y el primer login empuja `business.vertical`). Acá ya
 * hay sesión, así que el camino correcto es escribir directo con
 * [SettingsRubroApi.guardarVertical] — la misma llamada que hoy sólo dispara
 * el efecto de sincronización de `RutBusinessApp.kt`, y sólo cuando el
 * servidor todavía no tiene nada guardado. Ésta es la primera puerta que la
 * escribe con intención de **reemplazar** lo que ya había.
 *
 * Nunca escribe sin confirmar: cambiar de rubro cambia qué ve la dueña en
 * toda la app (agente vs. impresora, fiado vs. recetas), así que
 * [RbConfirmDialog] media el golpe antes de tocar el server.
 */
@Composable
private fun PantallaDeCambiarRubro(sesion: SessionRepository, onVolver: () -> Unit) {
    val dimens = RbTheme.dimens
    val colors = RbTheme.colors
    val scope = rememberCoroutineScope()
    val entrada = LocalEntrada.current
    val repo = LocalRubro.current?.repositorio
    val actual = packActual()
    val feria = esFeria()

    var pendiente by rememberSaveable { mutableStateOf<String?>(null) }
    var guardando by rememberSaveable { mutableStateOf(false) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }

    val opciones = remember(actual.rubro) { OPCIONES_DE_RUBRO.filter { it.rubro != actual.rubro } }

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(title = tituloRubro(feria), subtitle = subtituloRubro(), onBack = onVolver)

        when {
            guardando -> RbLoadingState(
                label = guardandoRubro(),
                modifier = Modifier.padding(dimens.space3),
            )

            else -> Column(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(dimens.space3),
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                Text(
                    text = etiquetaRubroActual(actual.label),
                    style = RbTheme.typography.bodyStrong,
                    color = colors.textPrimary,
                )

                if (error != null) {
                    RbErrorState(title = "No se pudo guardar", message = error!!)
                }

                RbCard {
                    opciones.forEachIndexed { indice, opcion ->
                        RbListRow(
                            title = opcion.titulo,
                            subtitle = opcion.tagline,
                            onClick = { pendiente = opcion.rubro },
                        )
                        if (indice != opciones.lastIndex) RbDivider()
                    }
                }
            }
        }
    }

    val elegido = pendiente
    if (elegido != null) {
        val nombreNuevo = OPCIONES_DE_RUBRO.firstOrNull { it.rubro == elegido }?.titulo ?: elegido
        RbConfirmDialog(
            title = tituloConfirmarRubro(),
            message = mensajeConfirmarRubro(nombreNuevo),
            confirmLabel = botonConfirmarRubro(),
            onDismiss = { pendiente = null },
            onConfirm = {
                pendiente = null
                val api = sesion.apiActiva()
                if (api != null && repo != null) {
                    guardando = true
                    scope.launch {
                        when (val resultado = SettingsRubroApi(api).guardarVertical(elegido)) {
                            is Resultado.Ok -> {
                                entrada?.preferencias?.guardarRubroElegido(elegido)
                                repo.cargar(api, fallbackRubro = elegido)
                                guardando = false
                                onVolver()
                            }

                            is Resultado.Falla -> {
                                error = resultado.error.userMessage
                                guardando = false
                            }
                        }
                    }
                } else {
                    error = mensajeSinConexionRubro()
                }
            },
        )
    }
}

private data class OpcionDeRubro(val rubro: String, val titulo: String, val tagline: String)

/**
 * Mismos cuatro rubros y mismas frases que `ElegirRubro` (alta), para que la
 * dueña no lea una descripción distinta del mismo rubro según por dónde entró.
 * No se reusa el composable de `ElegirRubro` porque esa pantalla dibuja su
 * propio `RbTopBar` sin `onBack` y su propio "Decidir después" — ninguno de
 * los dos tiene sentido acá, donde ya hay un rubro elegido y un `onVolver`.
 */
private val OPCIONES_DE_RUBRO = listOf(
    OpcionDeRubro(
        rubro = "feria",
        titulo = "Feria / Calle",
        tagline = "Puesto, voz, fiado. Sin escáner ni impresora el día 1.",
    ),
    OpcionDeRubro(
        rubro = "minimarket",
        titulo = "Minimarket / Almacén",
        tagline = "Caja, productos y stock del local.",
    ),
    OpcionDeRubro(
        rubro = "farmacia",
        titulo = "Farmacia",
        tagline = "Recetas, lotes, códigos de barra y boleta.",
    ),
    OpcionDeRubro(
        rubro = "tienda",
        titulo = "Tienda / Retail",
        tagline = "Ropa, accesorios, tallas y códigos.",
    ),
)
