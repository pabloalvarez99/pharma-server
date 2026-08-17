package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.core.backup.UserBackupApi
import cl.rutbusiness.core.backup.conRehidratacion
import cl.rutbusiness.core.backup.rescatarRespaldoDesdeCero
import cl.rutbusiness.core.net.ServerUrl
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbButtonVariant
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading
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
 * Lo que pide es exactamente lo que dice la tarjeta de papel: el nombre corto
 * del puesto/negocio y las 12 palabras (o los 5 bloques, que son los mismos
 * 84 bits). La dirección del computador solo se pregunta en on-prem LAN; en
 * nube el APK ya trae el destino y la pantalla no nombra IP ni computador.
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
                // Nube: sin IP ni computador. On-prem: se puede decir la dirección.
                mensaje = copyRescate(
                    pideDireccion = pideDireccion,
                    esFeria = esFeria,
                ).errorDireccion
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
 *
 * Se siente un **rescate del cuaderno**, no un formulario técnico: un cartel
 * de calma arriba, campos con aire, CTA anclado abajo y el mensaje como
 * tarjeta (no muro rojo).
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
    val colors = RbTheme.colors
    val shape = RbTheme.shapes.card
    val copy = copyRescate(pideDireccion = pideDireccion, esFeria = esFeria)
    // En nube la URL ya viene del APK: no bloquear el botón por el campo oculto.
    val puede = !trabajando &&
        (url.isNotBlank() || !pideDireccion) &&
        negocio.isNotBlank() &&
        tarjeta.isNotBlank()

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = copy.tituloBarra,
            subtitle = copy.subtituloBarra,
            onBack = onVolver,
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .imePadding()
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            // Cartel de calma: la persona llega nerviosa; no se le grita.
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(colors.surfaceRaised)
                    .border(dimens.focusRing, colors.outlineStrong, shape)
                    .padding(horizontal = dimens.space3, vertical = dimens.space4),
                verticalArrangement = Arrangement.spacedBy(dimens.space2),
            ) {
                Text(
                    text = copy.tituloCartel,
                    style = RbTheme.typography.heading,
                    color = colors.textPrimary,
                    modifier = Modifier.rbHeading(),
                )
                Text(
                    text = copy.cuerpoCartel,
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
                )
            }

            // Una pregunta a la vez: ¿dónde/cómo se llama? y después las 12 palabras.
            CartelDeCampos(titulo = copy.tituloCampos) {
                // Solo on-prem: dirección del computador. Nube nunca muestra IP.
                if (pideDireccion && copy.labelDireccion != null) {
                    RbTextField(
                        value = url,
                        onValueChange = onUrl,
                        label = copy.labelDireccion,
                        placeholder = copy.placeholderDireccion.orEmpty(),
                        supportingText = copy.ayudaDireccion,
                        keyboardType = KeyboardType.Uri,
                        enabled = !trabajando,
                        imeAction = ImeAction.Next,
                    )
                }
                RbTextField(
                    value = negocio,
                    onValueChange = onNegocio,
                    label = copy.labelNombre,
                    supportingText = copy.ayudaNombre,
                    enabled = !trabajando,
                    imeAction = ImeAction.Next,
                )
            }

            CartelDeCampos(titulo = copy.tituloPalabras) {
                RbTextField(
                    value = tarjeta,
                    onValueChange = onTarjeta,
                    label = copy.labelPalabras,
                    supportingText = copy.ayudaPalabras,
                    enabled = !trabajando,
                    imeAction = ImeAction.Go,
                    onImeAction = onRescatar,
                )
                Text(
                    text = copy.avisoPrivacidad,
                    style = RbTheme.typography.support,
                    color = colors.textSecondary,
                )
            }

            mensaje?.let { texto ->
                TarjetaDeFallaEntrada(
                    titulo = if (listo) copy.tituloListo else copy.tituloFalla,
                    queHacer = texto,
                )
            }
        }

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(colors.outline),
        )

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(colors.surfaceRaised)
                .windowInsetsPadding(
                    WindowInsets.safeDrawing.only(
                        WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                    ),
                )
                .padding(dimens.space3),
            verticalArrangement = Arrangement.spacedBy(dimens.space2),
        ) {
            RbButton(
                label = if (trabajando) copy.ctaBuscando else copy.ctaRescatar,
                onClick = onRescatar,
                enabled = puede,
                fillWidth = true,
            )

            if (listo) {
                RbButton(
                    label = copy.ctaEntrar,
                    onClick = onEntrar,
                    variant = RbButtonVariant.Secondary,
                    enabled = !trabajando,
                    fillWidth = true,
                )
            }
        }
    }
}

/** Marco de un bloque de campos: superficie del puesto, no formulario apretado. */
@Composable
private fun CartelDeCampos(
    titulo: String,
    content: @Composable () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surface)
            .border(dimens.border, colors.outline, shape)
            .padding(horizontal = dimens.space3, vertical = dimens.space3),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        content()
    }
}
