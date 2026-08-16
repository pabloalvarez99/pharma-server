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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.core.backup.UserBackupApi
import cl.rutbusiness.core.backup.conRehidratacion
import cl.rutbusiness.core.backup.rescatarRespaldoDesdeCero
import cl.rutbusiness.core.net.ServerUrl
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Teléfono nuevo, app recién instalada, la tarjeta del cuaderno en la mano
 * (ADR-0023).
 *
 * **Por qué esta pantalla vive acá y no adentro de la app.** El otro camino
 * para bajar un respaldo —"Traer de la nube", en la pantalla de la cola— exige
 * sesión, y para tener sesión hay que entrar. Quien perdió el teléfono no puede
 * entrar: no tiene token guardado y muchas veces tampoco se acuerda de la clave
 * que eligió hace meses, porque hasta ahora nunca se la habían pedido de nuevo.
 * Un respaldo que sólo se puede bajar desde un aparato que todavía funciona no
 * es un respaldo. Por eso el rescate está **antes** del login, que es donde
 * está parada la persona que lo necesita.
 *
 * Lo que pide es exactamente lo que dice la tarjeta impresa: el nombre corto
 * del negocio y las 12 palabras (o los 5 bloques, que son los mismos 84 bits).
 * La dirección se pregunta porque un teléfono recién instalado tampoco la sabe;
 * si el APK trae una por defecto, ya viene escrita.
 *
 * Los dos PBKDF2 —el de la prueba de retiro y el de la llave del sobre— corren
 * en [Dispatchers.Default]. Son ~210.000 iteraciones cada uno: en el aparato
 * lento eso es alrededor de un segundo por vuelta, y en el hilo de la UI
 * significa la pantalla congelada justo en el momento en que la persona está
 * más nerviosa.
 */
@Composable
internal fun RescateRoute(
    sesion: SessionRepository,
    estado: EstadoSesion.SinSesion,
    onVolver: () -> Unit,
    /** Sigue al login con la dirección y el negocio ya escritos. */
    onEntrarConDatos: (url: String, negocio: String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val servicios = LocalEntrada.current
    val offline = LocalOffline.current
    val alcance = rememberCoroutineScope()

    var url by rememberSaveable {
        mutableStateOf(estado.baseUrl ?: servicios?.nube.orEmpty())
    }
    var negocio by rememberSaveable { mutableStateOf(estado.tenant.orEmpty()) }
    // La tarjeta **no** es `rememberSaveable`: el estado guardado va al bundle
    // del sistema y de ahí a disco. Es el secreto que abre todo el negocio; no
    // tiene por qué sobrevivir a que la pantalla gire.
    var tarjeta by remember { mutableStateOf("") }
    var trabajando by remember { mutableStateOf(false) }
    var mensaje by rememberSaveable { mutableStateOf<String?>(null) }
    var listo by rememberSaveable { mutableStateOf(false) }

    val esFeria = servicios?.preferencias?.rubroElegido() == "feria"
    val pideDireccion = servicios?.nube.isNullOrBlank()

    RescatePantalla(
        url = url,
        onUrl = { url = it; mensaje = null },
        pideDireccion = pideDireccion,
        esFeria = esFeria,
        negocio = negocio,
        onNegocio = { negocio = it; mensaje = null },
        tarjeta = tarjeta,
        onTarjeta = { tarjeta = it; mensaje = null },
        trabajando = trabajando,
        mensaje = mensaje,
        listo = listo,
        onVolver = onVolver.takeIf { !trabajando },
        onEntrar = { onEntrarConDatos(url, negocio) },
        onRescatar = {
            val destino = ServerUrl.normalizar(url)
            if (destino == null) {
                mensaje = if (!pideDireccion) {
                    "RutAgent no responde. Reintentá en un momento."
                } else {
                    "Revisá la dirección: algo como 192.168.1.10:8080 o " +
                        "app.rutbusiness.cl."
                }
            } else {
                trabajando = true
                mensaje = null
                alcance.launch {
                    try {
                        val api = UserBackupApi(sesion.apiPara(destino))
                        val r = withContext(Dispatchers.Default) {
                            rescatarRespaldoDesdeCero(api, negocio, tarjeta)
                        }
                        mensaje = r.fold(
                            onSuccess = { abierta ->
                                // El rubro vuelve del snapshot: sin esto la app
                                // recién instalada arranca preguntando a qué se
                                // dedica el negocio, que es una pregunta que
                                // esta persona ya contestó una vez.
                                abierta.snapshot.rubro?.takeIf { it.isNotBlank() }?.let {
                                    servicios?.preferencias?.guardarRubroElegido(it)
                                }
                                val final = if (offline != null) {
                                    abierta.conRehidratacion(
                                        offline.cola.fusionarDesdeRespaldo(
                                            abierta.snapshot.pendingSales,
                                        ),
                                    )
                                } else {
                                    abierta
                                }
                                listo = true
                                final.mensaje
                            },
                            onFailure = {
                                it.message ?: "No se pudo traer el respaldo."
                            },
                        )
                    } finally {
                        trabajando = false
                    }
                }
            }
        },
        modifier = modifier,
    )
}

/**
 * La parte dibujable, sin sesión ni red detrás — mismo criterio que
 * [FormularioDeEntrada]: es una pantalla que hay que poder medir al 200% de
 * escala y con el teclado arriba, y montarla de verdad necesitaría un servidor.
 */
@Composable
internal fun RescatePantalla(
    url: String,
    onUrl: (String) -> Unit,
    pideDireccion: Boolean = true,
    /** Copy feria: "puesto" en títulos y campos. */
    esFeria: Boolean = false,
    negocio: String,
    onNegocio: (String) -> Unit,
    tarjeta: String,
    onTarjeta: (String) -> Unit,
    trabajando: Boolean,
    mensaje: String?,
    listo: Boolean,
    onVolver: (() -> Unit)?,
    onEntrar: () -> Unit,
    onRescatar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val dimens = RbTheme.dimens
    val cosa = if (esFeria) "puesto" else "negocio"
    // En nube la URL ya viene del APK: no bloquear el botón por el campo oculto.
    val puede = !trabajando &&
        (url.isNotBlank() || !pideDireccion) &&
        negocio.isNotBlank() &&
        tarjeta.isNotBlank()

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = "Recuperar mi $cosa",
            subtitle = "Con la tarjeta del cuaderno",
            onBack = onVolver,
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .imePadding()
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space3),
        ) {
            RbCard(title = "Antes de empezar") {
                Text(
                    text = "Esto sirve si perdiste el teléfono o lo cambiaste, y guardaste la " +
                        "tarjeta que la app te pidió anotar. No hace falta tu clave: la " +
                        "tarjeta es la llave de tu $cosa.",
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textPrimary,
                )
            }

            RbCard(title = "¿Dónde está tu $cosa?") {
                if (pideDireccion) {
                    RbTextField(
                        value = url,
                        onValueChange = onUrl,
                        label = "Dirección del computador",
                        placeholder = "192.168.1.10:8080",
                        supportingText = "La misma que usabas para entrar.",
                        keyboardType = KeyboardType.Uri,
                        enabled = !trabajando,
                        imeAction = ImeAction.Next,
                    )
                }
                RbTextField(
                    value = negocio,
                    onValueChange = onNegocio,
                    label = "Nombre corto del $cosa",
                    supportingText = "El que está impreso arriba en la tarjeta.",
                    enabled = !trabajando,
                    imeAction = ImeAction.Next,
                )
            }

            RbCard(title = "La tarjeta") {
                RbTextField(
                    value = tarjeta,
                    onValueChange = onTarjeta,
                    label = "Las 12 palabras (o los 5 bloques)",
                    supportingText = "Copialas separadas por espacios, en el mismo orden. " +
                        "No importan mayúsculas ni tildes.",
                    enabled = !trabajando,
                    imeAction = ImeAction.Go,
                    onImeAction = onRescatar,
                )
                Text(
                    text = "La tarjeta no sale de este teléfono. Lo que viaja es una prueba " +
                        "derivada de ella, que sólo sirve para pedir tu paquete y no para " +
                        "abrirlo.",
                    style = RbTheme.typography.support,
                    color = RbTheme.colors.textSecondary,
                )
            }

            mensaje?.let {
                Text(
                    text = it,
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textPrimary,
                )
            }

            RbButton(
                label = if (trabajando) "Buscando tu respaldo..." else "Traer mi respaldo",
                onClick = onRescatar,
                enabled = puede,
                fillWidth = true,
            )

            if (listo) {
                RbButton(
                    label = "Entrar a mi $cosa",
                    onClick = onEntrar,
                    variant = RbButtonVariant.Secondary,
                    enabled = !trabajando,
                    fillWidth = true,
                )
            }
        }
    }
}
