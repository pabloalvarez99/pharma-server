package cl.rutbusiness.app.ui.caja

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.rubro.esFeria
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbEmptyState
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.components.RbSkeletonLines
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme

@Composable
fun CajaRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.Activa,
    onVolver: () -> Unit,
) {
    val offline = LocalOffline.current
    val vm: CajaViewModel = viewModel(
        key = "caja:${estado.baseUrl}",
        factory = viewModelFactory { initializer { CajaViewModel(sesion, offline) } },
    )
    CajaScreen(vm = vm, onVolver = onVolver)
}

/**
 * El ritual de todos los días: abrir la caja, moverle plata y cerrarla contando.
 *
 * Un solo destino con cinco momentos y no cinco pantallas sueltas, porque la
 * dueña no piensa "voy a la pantalla de movimientos": piensa "saqué plata para
 * el pan". Cada momento ocupa la pantalla entera — nada de diálogos con campos
 * adentro, que en un panel de 360dp con la letra al 200% quedan sin lugar para
 * el teclado.
 *
 * Regla de plata: acá no se suma nada. Lo que debería haber en el cajón y la
 * diferencia del cierre vienen resueltos del server y se escriben con la
 * [Moneda] del tenant, que también viene del server.
 */
@Composable
private fun CajaScreen(vm: CajaViewModel, onVolver: () -> Unit) {
    val dimens = RbTheme.dimens
    val feria = esFeria()

    // Feria: al entrar al paso Abrir se abre el puesto con $0 sin ritual de cajón.
    // 409 "ya tiene caja abierta" lo trata el VM como éxito.
    LaunchedEffect(feria, vm.paso, vm.cargando, vm.hayConexion, vm.guardando) {
        if (feria &&
            vm.paso == PasoDeCaja.Abrir &&
            !vm.cargando &&
            !vm.guardando &&
            vm.hayConexion &&
            vm.sesionDeCaja == null
        ) {
            vm.modoFeria(true)
            vm.abrirPuestoFeria()
        }
    }

    // Atrás deshace el último paso antes de salir de la caja, que es el orden en
    // que la gente espera que se deshagan las cosas. Se registra acá y no en
    // cada paso: un solo lugar donde mirar qué hace el botón físico.
    val atras: () -> Unit = when (vm.paso) {
        PasoDeCaja.Movimiento, PasoDeCaja.Arqueo -> vm::volverAlEstado
        PasoDeCaja.Cerrada -> {
            { vm.terminarElDia(); onVolver() }
        }

        else -> onVolver
    }
    BackHandler(onBack = atras)

    Column(modifier = Modifier.fillMaxSize()) {
        RbTopBar(
            title = tituloPasoCaja(vm.paso, feria || vm.esFeria, vm.tipoDeMovimiento),
            subtitle = subtituloDelPaso(vm, feria || vm.esFeria),
            onBack = atras,
            actions = {
                // No hay botón de actualizar mientras se está escribiendo un
                // monto: recargar ahí borraría lo tipeado sin avisar.
                if (vm.paso == PasoDeCaja.Abrir || vm.paso == PasoDeCaja.Abierta) {
                    RbButton(
                        label = "Actualizar",
                        onClick = vm::cargar,
                        variant = RbButtonVariant.Secondary,
                        enabled = !vm.cargando && !vm.guardando,
                    )
                }
            },
        )

        val fatal = vm.error
        when {
            // Se dice antes, no después de que falle. La caja es lo único de la
            // app que sin señal no se puede hacer ni a medias, y la dueña tiene
            // que enterarse al entrar y no al final del arqueo, con la plata ya
            // contada arriba del mostrador.
            !vm.hayConexion -> RbEmptyState(
                title = if (feria || vm.esFeria) {
                    "El puesto necesita el sistema prendido"
                } else {
                    "La caja necesita el sistema prendido"
                },
                hint = if (feria || vm.esFeria) {
                    "Sin conexión no se puede abrir el puesto ni cerrar el día: lo que " +
                        "debería haber lo calcula el sistema sumando las ventas. Mientras " +
                        "tanto puedes seguir cobrando en efectivo."
                } else {
                    "Sin conexión no se puede abrir la caja, anotar retiros ni cerrar con " +
                        "arqueo: lo que debería haber en el cajón lo calcula el sistema del " +
                        "negocio sumando las ventas del día, y desde el teléfono no hay contra " +
                        "qué comparar. Mientras tanto puedes seguir cobrando en efectivo."
                },
                actionLabel = "Volver",
                onAction = onVolver,
                modifier = Modifier.padding(dimens.space3),
            )

            fatal != null -> RbErrorState(
                title = fatal.title,
                message = fatal.message,
                modifier = Modifier.padding(dimens.space3),
                retryLabel = fatal.retryLabel,
                onRetry = if (fatal.retryLabel != null) vm::cargar else null,
            )

            vm.cargando && vm.sesionDeCaja == null -> Column {
                RbLoadingState(
                    label = if (feria || vm.esFeria) {
                        "Abriendo el puesto..."
                    } else {
                        "Viendo cómo está la caja..."
                    },
                )
                RbSkeletonLines(lines = 4, modifier = Modifier.padding(dimens.space3))
            }

            else -> when (vm.paso) {
                PasoDeCaja.Abrir -> PasoAbrirCaja(vm)
                PasoDeCaja.Abierta -> PasoCajaAbierta(vm)
                PasoDeCaja.Movimiento -> PasoMovimiento(vm)
                PasoDeCaja.Arqueo -> PasoArqueo(vm)
                PasoDeCaja.Cerrada -> PasoCajaCerrada(vm, onListo = { vm.terminarElDia(); onVolver() })
            }
        }
    }
}

private fun subtituloDelPaso(vm: CajaViewModel, feria: Boolean): String? = when (vm.paso) {
    PasoDeCaja.Abrir -> if (feria) "Sin contar monedas" else "Lo primero del día"
    PasoDeCaja.Abierta -> vm.sesionDeCaja?.nombreDeCaja?.ifBlank { null }
    PasoDeCaja.Movimiento -> "Queda anotado hasta el cierre"
    PasoDeCaja.Arqueo -> "Cuenta primero, después cierra"
    PasoDeCaja.Cerrada -> null
}

/**
 * Qué teclado se abre para escribir plata.
 *
 * En una moneda sin decimales el teclado de sólo números no ofrece la coma, y
 * eso está bien: en pesos no hay decimales que escribir. En una de dos
 * decimales hace falta el separador, o el monto se escribe mal y el server
 * recibe cien veces lo que era.
 */
internal fun tecladoDePlata(moneda: Moneda): KeyboardType =
    if (moneda.decimales > 0) KeyboardType.Decimal else KeyboardType.Number
