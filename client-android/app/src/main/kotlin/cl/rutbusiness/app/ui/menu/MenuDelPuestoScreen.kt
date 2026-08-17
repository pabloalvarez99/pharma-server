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
import cl.rutbusiness.app.ui.entrada.LocalEntrada
import cl.rutbusiness.app.ui.impresora.EstadoDeImpresion
import cl.rutbusiness.app.ui.impresora.TarjetaDeReimpresion
import cl.rutbusiness.app.ui.impresora.impresoraViewModel
import cl.rutbusiness.app.ui.rubro.LocalRubro
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.app.ui.ventas.subtituloEntradaHistorial
import cl.rutbusiness.app.ui.ventas.tituloEntradaHistorial
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.rubro.SettingsRubroApi
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
 * No es una pestaña ni una pantalla de ajustes de sistema: es una rama más
 * de la pila de "Tu día" (`Pantalla.kt`), exactamente al lado de la caja y
 * el fiado, así que no hace falta un cuarto destino en la barra de abajo —
 * que además está tope en tres (`BarraDeNavegacion.kt`).
 *
 * Ya no maneja su propia sub-navegación. Hasta la ola 27 tenía adentro un
 * segundo `enum SeccionDelMenu` con su propio `when` — un segundo dueño de
 * la navegación, aparte de `RutinaDelDia` en `TuDiaRoute.kt`, y la
 * consecuencia era que `CajaRoute` se alcanzaba desde los dos con
 * semánticas de "volver" distintas. Impresora y cambiar de rubro ahora son
 * ramas más de la misma pila que maneja `TuDiaRoute` (`Pantalla.Impresora`,
 * `Pantalla.Rubro`); acá sólo queda la portada.
 *
 * @param onVolver vuelve a "Hoy". Lo pasa `TuDiaRoute`, igual que en caja y
 *   fiado.
 * @param onAbrirCaja empuja `Pantalla.Caja` en la pila de `TuDiaRoute`.
 * @param onAbrirImpresora empuja `Pantalla.Impresora`.
 * @param onAbrirRubro empuja `Pantalla.Rubro`.
 * @param onAbrirHistorialVentas empuja `Pantalla.HistorialVentas` (ola 29).
 */
@Composable
fun MenuDelPuestoRoute(
    onVolver: () -> Unit,
    onAbrirCaja: () -> Unit,
    onAbrirImpresora: () -> Unit,
    onAbrirRubro: () -> Unit,
    onAbrirHistorialVentas: () -> Unit,
) {
    ListaDelMenu(
        onVolver = onVolver,
        onAbrirCaja = onAbrirCaja,
        onAbrirImpresora = onAbrirImpresora,
        onAbrirRubro = onAbrirRubro,
        onAbrirHistorialVentas = onAbrirHistorialVentas,
    )
}

/**
 * Los dos ritmos en que la dueña usa este menú, para que doce entradas no se
 * lean como una guía telefónica.
 *
 * [Diario]: lo que se toca en medio de un día de puesto — contar la plata,
 * la impresora, ventas que faltan subir, anotar un gasto. [Mensual]: lo que
 * se toca una vez y se olvida por semanas — cambiar de rubro, ver márgenes,
 * preparar un respaldo. El orden de declaración es el orden en que se
 * dibujan los grupos: [Diario] siempre arriba.
 *
 * Dónde cae cada pantalla nueva de las olas 29 a 33 — detalle completo del
 * empuje a la pila en el KDoc de `Pantalla` — por ritmo de uso:
 * - Historial de ventas (ola 29): [Diario].
 * - Devoluciones (ola 29): [Diario] — pasa en el puesto, no una vez al mes.
 * - Márgenes (ola 31): [Mensual] — se revisa, no se anota en el momento.
 * - Gastos (ola 32): [Diario] — se anota cada vez que sale plata.
 * - Respaldo (ola 33): [Mensual].
 * - Fiado por persona (ola 30) no entra acá: cuelga de [Pantalla.Fiado], no
 *   de este menú (ver KDoc de `Pantalla`).
 */
private enum class GrupoDeMenu { Diario, Mensual }

/** La etiqueta visible de cada [GrupoDeMenu], fijada por `CopyMenuTest`. */
private fun tituloDeGrupo(grupo: GrupoDeMenu): String = when (grupo) {
    GrupoDeMenu.Diario -> tituloGrupoDiario()
    GrupoDeMenu.Mensual -> tituloGrupoMensual()
}

/** Una fila tocable del menú. */
private data class EntradaDeMenu(
    val titulo: String,
    val subtitulo: String,
    val grupo: GrupoDeMenu,
    val onClick: () -> Unit,
)

/**
 * La portada: la lista de puertas, agrupada por [GrupoDeMenu] en vez de una
 * sola lista plana — con cuatro entradas no se nota, con las diez o doce que
 * llegan en las olas 29 a 33 es la diferencia entre un menú y una guía
 * telefónica.
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
    onAbrirHistorialVentas: () -> Unit,
) {
    val dimens = RbTheme.dimens
    val feria = esFeria()
    val pack = packActual()
    val abrirPendientes = abrirVentasPendientes()

    val entradas = remember(feria, pack.features.printer, abrirPendientes) {
        buildList {
            add(
                EntradaDeMenu(
                    tituloCaja(feria),
                    subtituloCaja(feria),
                    GrupoDeMenu.Diario,
                    onAbrirCaja,
                ),
            )
            add(
                EntradaDeMenu(
                    tituloEntradaHistorial(feria),
                    subtituloEntradaHistorial(feria),
                    GrupoDeMenu.Diario,
                    onAbrirHistorialVentas,
                ),
            )
            if (pack.features.printer) {
                add(
                    EntradaDeMenu(
                        tituloImpresora(),
                        subtituloImpresora(),
                        GrupoDeMenu.Diario,
                        onAbrirImpresora,
                    ),
                )
            }
            if (abrirPendientes != null) {
                add(
                    EntradaDeMenu(
                        tituloVentasPendientes(),
                        subtituloVentasPendientes(),
                        GrupoDeMenu.Diario,
                        abrirPendientes,
                    ),
                )
            }
            add(EntradaDeMenu(tituloRubro(feria), subtituloRubro(), GrupoDeMenu.Mensual, onAbrirRubro))
        }
    }
    val porGrupo = remember(entradas) { entradas.groupBy { it.grupo } }

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(title = tituloMenu(feria), subtitle = subtituloMenu(), onBack = onVolver)
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            GrupoDeMenu.entries.forEach { grupo ->
                val deEsteGrupo = porGrupo[grupo].orEmpty()
                if (deEsteGrupo.isNotEmpty()) {
                    RbCard(title = tituloDeGrupo(grupo)) {
                        deEsteGrupo.forEachIndexed { indice, entrada ->
                            RbListRow(
                                title = entrada.titulo,
                                subtitle = entrada.subtitulo,
                                onClick = entrada.onClick,
                            )
                            if (indice != deEsteGrupo.lastIndex) RbDivider()
                        }
                    }
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
 *
 * `internal` y no `private`: es una rama de la pila que maneja `TuDiaRoute`
 * (`Pantalla.Impresora`), que la llama directo — no pasa por
 * `MenuDelPuestoRoute`.
 */
@Composable
internal fun PantallaDeImpresoraDelMenu(onVolver: () -> Unit) {
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
 *
 * `internal` y no `private`: es una rama de la pila que maneja `TuDiaRoute`
 * (`Pantalla.Rubro`), que la llama directo — no pasa por `MenuDelPuestoRoute`.
 */
@Composable
internal fun PantallaDeCambiarRubro(sesion: SessionRepository, onVolver: () -> Unit) {
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
