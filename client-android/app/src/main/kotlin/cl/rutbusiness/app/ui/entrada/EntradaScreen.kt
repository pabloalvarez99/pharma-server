package cl.rutbusiness.app.ui.entrada

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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbErrorState
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme

/**
 * La puerta de entrada: la explicación del primer uso, y después el formulario.
 *
 * Quién decide cuál de las dos se ve: la bandera de [PreferenciasDeEntrada],
 * que se prende con el primer login que funciona. Quien ya entró alguna vez
 * nunca más ve la explicación, ni siquiera después de cerrar sesión — a esa
 * altura ya sabe lo que dice.
 */
@Composable
fun EntradaRoute(sesion: SessionRepository, estado: EstadoSesion.SinSesion) {
    val servicios = LocalEntrada.current

    val vm: EntradaViewModel = viewModel(
        key = "entrada",
        factory = viewModelFactory {
            initializer {
                EntradaViewModel(
                    sesion = sesion,
                    probador = ServidorPorHttp(sesion),
                    red = servicios?.red,
                    inicial = estado,
                )
            }
        },
    )

    // Arranca en la explicación sólo si este teléfono nunca entró. Sin
    // servicios de plataforma —una prueba de otra pantalla— se salta: montar
    // una bienvenida que nadie pidió rompería esas pruebas sin proteger a nadie.
    val primeraVez = servicios?.preferencias?.yaEntroAlgunaVez() == false
    var explicando by rememberSaveable(servicios) {
        mutableStateOf(primeraVez)
    }
    // Tras el copy: elegir rubro (feria first) si aún no hay preferencia.
    var eligiendoRubro by rememberSaveable(servicios) {
        mutableStateOf(
            primeraVez && servicios?.preferencias?.rubroElegido() == null,
        )
    }

    if (explicando) {
        PrimerUso(onListo = {
            explicando = false
            // Si ya eligió rubro en una visita anterior, salta esta pantalla.
            if (servicios?.preferencias?.rubroElegido() == null) {
                eligiendoRubro = true
            }
        })
        return
    }

    if (eligiendoRubro) {
        ElegirRubro(onElegir = { rubro ->
            servicios?.preferencias?.guardarRubroElegido(rubro)
            eligiendoRubro = false
        })
        return
    }

    FormularioDeEntrada(
        url = vm.url,
        onUrl = vm::cambiarUrl,
        ayudaDeDireccion = vm.ayudaDeDireccion,
        errorDeDireccion = vm.errorDeDireccion,
        negocio = vm.negocio,
        onNegocio = vm::cambiarNegocio,
        email = vm.email,
        onEmail = vm::cambiarEmail,
        password = vm.password,
        onPassword = vm::cambiarPassword,
        conexionConfirmada = vm.conexionConfirmada,
        falla = vm.falla,
        impedimento = vm.impedimentoParaEntrar(),
        probando = vm.probando,
        enviando = vm.enviando,
        puedeProbar = vm.puedeProbar,
        puedeEntrar = vm.puedeEntrar,
        onProbar = vm::probarConexion,
        onEntrar = vm::entrar,
        onVerExplicacion = { explicando = true },
    )
}

/**
 * Dónde está tu negocio y quién eres.
 *
 * Recibe datos y callbacks, sin `ViewModel` detrás — mismo criterio que
 * [cl.rutbusiness.app.ui.caja.FormularioDeArqueo]. Acá pesa doble: ésta es la
 * primera pantalla que ve alguien que nunca usó la app, o sea la que hay que
 * poder medir al 200% de escala y con el teclado arriba, y montarla de verdad
 * necesitaría un servidor y un almacén cifrado.
 *
 * Lo que cambió respecto de la pantalla de login anterior no es el layout, es
 * **qué pasa cuando algo falla**:
 *
 * - La dirección se valida mientras se escribe, contra el mismo normalizador
 *   que va a usar la conexión. Escribir `192.168.1.10:8080` sin `http://` está
 *   bien y la pantalla lo confirma en vez de rechazarlo.
 * - Se puede probar la dirección **sin** tener la clave a mano, porque los dos
 *   datos no llegan juntos: la dirección la da quien instaló el sistema.
 * - Al fallar, se dice cuál de las cuatro cosas falló. Ver [FallaDeConexion].
 *
 * El copy no dice "servidor" en ninguna etiqueta: dice "el computador del
 * negocio", que es el objeto que la dueña puede ver y tocar. La palabra
 * "servidor" se enseña una vez en [PrimerUso], para que entienda a quien se la
 * diga, y no se repite acá.
 */
@Composable
internal fun FormularioDeEntrada(
    url: String,
    onUrl: (String) -> Unit,
    ayudaDeDireccion: String,
    errorDeDireccion: String?,
    negocio: String,
    onNegocio: (String) -> Unit,
    email: String,
    onEmail: (String) -> Unit,
    password: String,
    onPassword: (String) -> Unit,
    conexionConfirmada: Boolean,
    falla: FallaDeConexion?,
    impedimento: String?,
    probando: Boolean,
    enviando: Boolean,
    puedeProbar: Boolean,
    puedeEntrar: Boolean,
    onProbar: () -> Unit,
    onEntrar: () -> Unit,
    onVerExplicacion: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val ocupado = probando || enviando
    val scroll = rememberScrollState()

    // Cuando aparece una falla, se baja hasta ella.
    //
    // Sin esto la pantalla no contesta: los cuatro campos llenan el alto del
    // aparato, así que la respuesta a «Probar la dirección» —que está al
    // final— nace fuera de cuadro. La dueña toca el botón, no ve cambiar nada,
    // y vuelve a tocar. Es el mismo bug de siempre disfrazado de mensaje bien
    // escrito: un mensaje que no se ve no dice nada.
    //
    // `withFrameNanos` antes de leer `maxValue`: en el frame en que la falla se
    // agrega, la columna todavía mide lo de antes y el scroll llegaría corto.
    LaunchedEffect(falla) {
        if (falla != null) {
            withFrameNanos { }
            scroll.animateScrollTo(scroll.maxValue)
        }
    }

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Entrar a tu negocio",
            subtitle = "Una sola vez: después la app se acuerda",
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(scroll)
                .imePadding()
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            RbCard(title = "¿Dónde está el computador del negocio?") {
                RbTextField(
                    value = url,
                    onValueChange = onUrl,
                    label = "Dirección del computador",
                    placeholder = "192.168.1.10:8080",
                    supportingText = ayudaDeDireccion,
                    errorMessage = errorDeDireccion,
                    keyboardType = KeyboardType.Uri,
                    enabled = !ocupado,
                )

                if (conexionConfirmada) {
                    RbChip(label = "Contestó tu negocio", tone = RbChipTone.Brand)
                }

                RbButton(
                    label = if (probando) "Probando..." else "Probar la dirección",
                    onClick = onProbar,
                    variant = RbButtonVariant.Secondary,
                    enabled = puedeProbar,
                    fillWidth = true,
                )
                Text(
                    text = "Puedes probar la dirección ahora, sin escribir tu clave.",
                    style = RbTheme.typography.support,
                    color = RbTheme.colors.textSecondary,
                )
            }

            RbCard(title = "¿Quién eres?") {
                RbTextField(
                    value = negocio,
                    onValueChange = onNegocio,
                    label = "Nombre corto del negocio",
                    supportingText = "El que te dieron al crear el negocio. Suele ser una sola " +
                        "palabra, sin espacios.",
                    enabled = !ocupado,
                    imeAction = ImeAction.Next,
                )
                RbTextField(
                    value = email,
                    onValueChange = onEmail,
                    label = "Correo",
                    keyboardType = KeyboardType.Email,
                    enabled = !ocupado,
                    imeAction = ImeAction.Next,
                )
                RbTextField(
                    value = password,
                    onValueChange = onPassword,
                    label = "Clave",
                    supportingText = "Distingue mayúsculas de minúsculas.",
                    keyboardType = KeyboardType.Password,
                    visualTransformation = PasswordVisualTransformation(),
                    enabled = !ocupado,
                    imeAction = ImeAction.Go,
                    onImeAction = onEntrar,
                )
            }

            // El bloque de la falla va **antes** del botón y no debajo del campo
            // que la causó: la dirección puede estar perfecta y la falla ser del
            // wifi, y colgar el reclamo de un campo manda a corregir donde no era.
            falla?.let {
                RbErrorState(
                    title = it.titulo,
                    message = it.queHacer,
                    retryLabel = null,
                    onRetry = null,
                )
            }

            impedimento?.let { motivo ->
                Text(
                    text = motivo,
                    style = RbTheme.typography.support,
                    color = RbTheme.colors.textSecondary,
                )
            }

            RbButton(
                label = if (enviando) "Entrando..." else "Entrar",
                onClick = onEntrar,
                enabled = puedeEntrar,
                fillWidth = true,
            )

            RbButton(
                label = "Ver la explicación de nuevo",
                onClick = onVerExplicacion,
                variant = RbButtonVariant.Secondary,
                enabled = !ocupado,
                fillWidth = true,
            )
        }
    }
}
