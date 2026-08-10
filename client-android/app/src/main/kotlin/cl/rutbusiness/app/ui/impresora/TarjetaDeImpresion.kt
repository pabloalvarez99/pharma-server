package cl.rutbusiness.app.ui.impresora

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbDivider
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Lo que el teléfono le presta a la UI para imprimir.
 *
 * Va por `CompositionLocal` y no por parámetro a través de media app: la
 * impresora la usa una tarjeta en el fondo de la pantalla de cobro, y hacerla
 * viajar por `RutBusinessApp` → `ConSesion` → `CobrarRoute` → `CobrarScreen` →
 * `PasoComprobante` obligaría a tocar cinco archivos que no tienen nada que ver
 * con imprimir.
 */
class ServiciosDeImpresora(
    /** Para pedirle el comprobante al server con el token de la sesión activa. */
    val sesion: SessionRepository,
    val enlace: EnlaceDeImpresora,
    val preferencias: PreferenciasDeImpresora,
)

/**
 * `null` cuando nadie lo proveyó — en una prueba de otra pantalla, por ejemplo.
 * La tarjeta simplemente no se dibuja, y la venta sigue andando igual.
 */
val LocalImpresora: ProvidableCompositionLocal<ServiciosDeImpresora?> =
    compositionLocalOf { null }

@Composable
fun ProveerImpresora(
    servicios: ServiciosDeImpresora,
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(LocalImpresora provides servicios, content = content)
}

/**
 * El `ViewModel` de impresión, colgado del `Activity`.
 *
 * Una sola instancia para toda la app, con clave fija: la impresora elegida y
 * la última boleta son del negocio, no de la pantalla. Así "reimprimir" desde
 * el mostrador sabe exactamente lo mismo que sabía la pantalla de comprobante.
 *
 * Devuelve `null` si nadie proveyó [LocalImpresora]. Quien lo llame dibuja lo
 * que pueda y sigue: que no haya impresora en el build no puede impedir cobrar,
 * y las pruebas de las otras pantallas no tienen que montar un Bluetooth falso
 * para poder correr.
 */
@Composable
fun impresoraViewModel(): ImpresoraViewModel? {
    val servicios = LocalImpresora.current ?: return null
    return viewModel(
        key = "impresora",
        factory = viewModelFactory {
            initializer {
                ImpresoraViewModel(
                    boletas = BoletasDelServidor(servicios.sesion),
                    enlace = servicios.enlace,
                    preferencias = servicios.preferencias,
                    sesion = servicios.sesion,
                )
            }
        },
    )
}

/**
 * La boleta, en la pantalla de "la venta quedó".
 *
 * **La venta ya se cobró antes de que esto se dibuje.** Toda la tarjeta está
 * escrita alrededor de esa frase: no hay un solo camino que bloquee la
 * pantalla, que exija resolver la impresora para seguir, o que sugiera que la
 * venta corre peligro. En cada estado de falla hay una salida que no es
 * arreglar la impresora — "Seguir sin boleta" —, y el botón de cobrar la venta
 * siguiente vive **afuera** de esta tarjeta y nunca se apaga.
 *
 * @param ordenId la venta a imprimir. `null` mientras el comprobante no llegó:
 *   sin id no hay nada que pedirle al server, y se dice en vez de fallar.
 */
@Composable
fun TarjetaDeImpresion(
    vm: ImpresoraViewModel,
    ordenId: String?,
    modifier: Modifier = Modifier,
) {
    RbCard(title = "Boleta", modifier = modifier) {
        CuerpoDeImpresion(vm = vm) {
            EnReposoParaImprimir(vm = vm, ordenId = ordenId)
        }
    }
}

/**
 * Reimprimir la última.
 *
 * Se pide todo el tiempo y por motivos que no son errores: se cortó el papel,
 * salió corrida, el cliente la quiere de nuevo, el dueño la quiere para su
 * carpeta. Vuelve a pedirle el comprobante al server con el id guardado, así
 * que anda con la app recién abierta y después de tres ventas más.
 */
@Composable
fun TarjetaDeReimpresion(
    vm: ImpresoraViewModel,
    onCerrar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens

    RbCard(title = "Reimprimir la última boleta", modifier = modifier) {
        CuerpoDeImpresion(vm = vm, onListo = onCerrar) {
            Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
                if (vm.hayUltimaBoleta) {
                    RbButton(
                        label = "Imprimir de nuevo",
                        onClick = vm::reimprimir,
                        fillWidth = true,
                    )
                    Text(
                        text = "Sale la misma boleta de la última venta. Reimprimir no vuelve " +
                            "a cobrar.",
                        style = RbTheme.typography.support,
                        color = RbTheme.colors.textSecondary,
                    )
                } else {
                    Text(
                        text = "Todavía no imprimiste ninguna boleta desde este teléfono. " +
                            "Cobra una venta y en la pantalla de «Listo» va a aparecer el botón " +
                            "de imprimir; después vuelve a estar acá para repetirla.",
                        style = RbTheme.typography.body,
                        color = RbTheme.colors.textSecondary,
                    )
                }
                RbButton(
                    label = "Cerrar",
                    onClick = onCerrar,
                    variant = RbButtonVariant.Secondary,
                    fillWidth = true,
                )
            }
        }
    }
}

/**
 * Los estados que las dos tarjetas comparten.
 *
 * Sólo cambia qué se ofrece en reposo — imprimir esta venta, o reimprimir la
 * última —; elegir impresora, elegir ancho, esperar, fallar y recuperarse es
 * exactamente lo mismo, y tenerlo escrito dos veces sería garantizar que un día
 * un camino de falla se arregle en una tarjeta y no en la otra.
 */
@Composable
private fun CuerpoDeImpresion(
    vm: ImpresoraViewModel,
    onListo: (() -> Unit)? = null,
    enReposo: @Composable () -> Unit,
) {
    val dimens = RbTheme.dimens

    // El lanzador de permisos vive acá porque un `ActivityResultLauncher` sólo
    // se puede recordar dentro de una composición. Lo que la pantalla sabe es
    // que hay una lista de permisos que pedir; **cuáles** son, y si este
    // Android los pide o no, lo contesta `EnlaceDeImpresora`.
    val pedirPermisos = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) {
        // Se reintenta pase lo que pase, sin mirar el resultado. Si concedió,
        // la impresión sale; si negó, el enlace vuelve a devolver
        // `FaltaPermiso` y la dueña lee **el mismo** texto que ya leyó, que
        // además explica qué hacer cuando Android dejó de preguntar. Dos
        // mensajes distintos para el mismo problema sólo confunden.
        vm.reintentar()
    }

    when (val estado = vm.estado) {
        EstadoDeImpresion.Reposo -> enReposo()

        is EstadoDeImpresion.Eligiendo -> ListaDeImpresoras(vm, estado.disponibles)

        is EstadoDeImpresion.EligiendoAncho -> ElegirAncho(vm, estado.impresora)

        EstadoDeImpresion.Imprimiendo -> RbLoadingState(
            label = "Mandando la boleta a «${vm.impresora?.nombre ?: "la impresora"}»...",
        )

        EstadoDeImpresion.Impresa -> Impresa(vm, onListo)

        is EstadoDeImpresion.Fallida -> Fallida(
            vm = vm,
            falla = estado.falla,
            onDarPermiso = { pedirPermisos.launch(vm.permisosQuePedir.toTypedArray()) },
        )

        EstadoDeImpresion.SinBoleta -> Column(
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            Text(
                text = "Seguiste sin boleta. La venta quedó guardada igual.",
                style = RbTheme.typography.body,
                color = RbTheme.colors.textSecondary,
            )
            RbButton(
                label = "Probar de imprimir",
                onClick = vm::reintentar,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

@Composable
private fun EnReposoParaImprimir(vm: ImpresoraViewModel, ordenId: String?) {
    val dimens = RbTheme.dimens
    val elegida = vm.impresora

    Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
        if (ordenId == null) {
            Text(
                text = "Todavía no tenemos el detalle de esta venta para imprimirlo. " +
                    "La venta sí quedó registrada.",
                style = RbTheme.typography.body,
                color = RbTheme.colors.textSecondary,
            )
            return@Column
        }

        RbButton(
            label = "Imprimir boleta",
            onClick = { vm.imprimir(ordenId) },
            fillWidth = true,
        )

        if (elegida == null) {
            // Se avisa antes de tocar, no después: la primera vez el botón abre
            // una lista en vez de imprimir, y una sorpresa en el mostrador con
            // el cliente al frente es una sorpresa de más.
            Text(
                text = "La primera vez te vamos a preguntar cuál es tu impresora. Después " +
                    "sale sola.",
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
        } else {
            Text(
                text = "Sale por «${elegida.nombre}», rollo de ${elegida.ancho.etiqueta}.",
                style = RbTheme.typography.support,
                color = RbTheme.colors.textSecondary,
            )
            RbButton(
                label = "Cambiar impresora",
                onClick = vm::cambiarImpresora,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

@Composable
private fun Impresa(vm: ImpresoraViewModel, onListo: (() -> Unit)?) {
    val dimens = RbTheme.dimens

    Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
        Text(
            text = "Boleta impresa.",
            style = RbTheme.typography.bodyStrong,
            color = RbTheme.colors.textPrimary,
        )
        RbButton(
            label = "Imprimir otra copia",
            onClick = vm::reintentar,
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )
        if (onListo != null) {
            RbButton(
                label = "Listo",
                onClick = onListo,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }
    }
}

/**
 * Qué ve la dueña cuando algo falló.
 *
 * Tres salidas, siempre, y en este orden: arreglar (dar permiso o reintentar),
 * cambiar de impresora, o seguir sin boleta. La última existe porque la venta
 * ya está cobrada y nadie puede quedar encerrado por un rollo de papel.
 */
@Composable
private fun Fallida(
    vm: ImpresoraViewModel,
    falla: FallaDeImpresion,
    onDarPermiso: () -> Unit,
) {
    val dimens = RbTheme.dimens

    Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
        RbErrorState(
            title = falla.titulo,
            message = falla.queHacer,
            retryLabel = null,
            onRetry = null,
        )

        if (falla is FallaDeImpresion.FaltaPermiso && vm.permisosQuePedir.isNotEmpty()) {
            RbButton(label = "Dar permiso", onClick = onDarPermiso, fillWidth = true)
        } else if (falla.sePuedeReintentar) {
            RbButton(label = "Reintentar", onClick = vm::reintentar, fillWidth = true)
        }

        if (falla !is FallaDeImpresion.SinBluetooth) {
            RbButton(
                label = "Elegir otra impresora",
                onClick = vm::cambiarImpresora,
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }

        RbButton(
            label = "Seguir sin boleta",
            onClick = vm::seguirSinBoleta,
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )

        Text(
            text = "La venta ya está cobrada y guardada. Nada de esto la deshace.",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )
    }
}

/**
 * Las impresoras que el teléfono ya tiene emparejadas.
 *
 * `Column` y no `LazyColumn`, y es la única excepción a la regla de listas
 * virtualizadas de toda la app: esto vive dentro de una pantalla que ya
 * scrollea —un `LazyColumn` adentro de un scroll vertical revienta por
 * restricciones infinitas— y la lista está acotada por cuántos aparatos puede
 * tener emparejados un teléfono, que son unos pocos y no crecen con el negocio.
 */
@Composable
private fun ListaDeImpresoras(vm: ImpresoraViewModel, disponibles: List<ImpresoraConocida>) {
    val dimens = RbTheme.dimens

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(dimens.space1),
    ) {
        Text(
            text = "¿Cuál es tu impresora?",
            style = RbTheme.typography.bodyStrong,
            color = RbTheme.colors.textPrimary,
        )
        Text(
            text = "Estas son las que este teléfono ya tiene emparejadas. Si la tuya no está, " +
                "enciéndela y emparéjala desde Ajustes › Bluetooth.",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )

        disponibles.forEach { impresora ->
            RbListRow(
                title = impresora.nombre,
                subtitle = impresora.direccion,
                onClick = { vm.elegirImpresora(impresora) },
            )
            RbDivider()
        }

        RbButton(
            label = "Ahora no",
            onClick = vm::cerrar,
            variant = RbButtonVariant.Secondary,
            fillWidth = true,
        )
    }
}

/**
 * El ancho del rollo.
 *
 * Se pregunta con la regla en la mano, no con el número: "58 mm" no le dice
 * nada a quien nunca compró papel térmico, y del ancho depende que la boleta
 * salga cortada o con la mitad en blanco.
 */
@Composable
private fun ElegirAncho(vm: ImpresoraViewModel, impresora: ImpresoraConocida) {
    val dimens = RbTheme.dimens

    Column(verticalArrangement = Arrangement.spacedBy(dimens.space2)) {
        Text(
            text = "¿De qué ancho es el papel de «${impresora.nombre}»?",
            style = RbTheme.typography.bodyStrong,
            color = RbTheme.colors.textPrimary,
        )

        AnchoDePapel.entries.forEach { ancho ->
            RbButton(
                label = "${ancho.etiqueta} — ${ancho.descripcion}",
                onClick = { vm.elegirAncho(impresora, ancho) },
                variant = RbButtonVariant.Secondary,
                fillWidth = true,
            )
        }

        Text(
            text = "Si te equivocas no pasa nada: se cambia después desde «Cambiar impresora».",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )
    }
}
