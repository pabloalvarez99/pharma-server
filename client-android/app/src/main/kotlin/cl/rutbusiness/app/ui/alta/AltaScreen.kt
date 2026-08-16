package cl.rutbusiness.app.ui.alta

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
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.entrada.LocalEntrada
import cl.rutbusiness.app.ui.entrada.ServidorPorHttp
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbTopBar
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbAssertive
import cl.rutbusiness.ui.theme.rbHeading

/**
 * "Crear mi negocio": de una app recién instalada a estar adentro, sin que
 * nadie tenga que dictar nada.
 *
 * Se siente un **cartel**, no un wizard: una pregunta grande a la vez, mucho
 * aire, un solo CTA de marca abajo y las fallas como tarjeta legible (no un
 * muro rojo). El piso de hardware —persona mayor, 720p, 200% de letra— manda
 * sobre el adorno.
 *
 * Al terminar no navega a ninguna parte: abre la sesión, y el árbol de arriba
 * ([cl.rutbusiness.app.ui.RutBusinessApp]) cambia solo de pantalla porque está
 * mirando el estado de la sesión. Es la misma mecánica del login y evita tener
 * dos fuentes de verdad sobre "¿ya está adentro?".
 *
 * @param onVolver salir del alta sin crear nada, de vuelta a la elección de
 *   camino ([cl.rutbusiness.app.ui.entrada.Puerta]).
 * @param onIrAEntrar irse **derecho** al formulario de entrada, sin pasar por
 *   la elección de camino. Es un destino aparte y no un alias de [onVolver]
 *   porque el botón que lo dispara dice "Entrar a ese negocio": devolver a la
 *   pantalla anterior y hacer que la dueña vuelva a elegir es prometer una
 *   cosa y hacer otra, justo cuando ya se le dijo que no puede crear nada.
 */
@Composable
fun AltaRoute(
    sesion: SessionRepository,
    onVolver: () -> Unit,
    onIrAEntrar: () -> Unit,
) {
    val servicios = LocalEntrada.current

    val vm: AltaViewModel = viewModel(
        key = "alta",
        factory = viewModelFactory {
            initializer {
                AltaViewModel(
                    sesion = sesion,
                    probador = ServidorPorHttp(sesion),
                    red = servicios?.red,
                    nube = servicios?.nube,
                    esFeria = servicios?.preferencias?.rubroElegido() == "feria",
                )
            }
        },
    )

    LaunchedEffect(vm) { vm.empezar() }

    PantallaDeAlta(
        paso = vm.paso,
        indice = vm.indice,
        total = vm.pasos.size,
        estado = vm.estado,
        falla = vm.falla,
        impedimento = vm.impedimento(),
        puedeAvanzar = vm.puedeAvanzar,
        url = vm.url,
        onUrl = vm::cambiarUrl,
        ayudaDeDireccion = vm.ayudaDeDireccion,
        errorDeDireccion = vm.errorDeDireccion,
        nombre = vm.nombre,
        onNombre = vm::cambiarNombre,
        rubro = vm.rubro,
        onRubro = vm::elegirRubro,
        email = vm.email,
        onEmail = vm::cambiarEmail,
        errorDeCorreo = vm.errorDeCorreo,
        clave = vm.clave,
        onClave = vm::cambiarClave,
        errorDeClave = vm.errorDeClave,
        onAtras = { if (!vm.retroceder()) onVolver() },
        onAvanzar = vm::avanzar,
        onCrear = vm::crear,
        onIrAEntrar = onIrAEntrar,
        esFeria = servicios?.preferencias?.rubroElegido() == "feria",
        nube = servicios?.nube != null,
    )
}

/**
 * El armazón del alta: la barra, la pregunta de turno, y un solo botón abajo.
 *
 * Recibe datos y callbacks, sin `ViewModel` detrás, por lo mismo que
 * [cl.rutbusiness.app.ui.entrada.FormularioDeEntrada]: es la pantalla que hay
 * que poder medir al 200% de escala y con el teclado arriba, y montarla de
 * verdad necesitaría un servidor y un almacén cifrado.
 *
 * **El botón no scrollea.** Vive en un panel pegado abajo, igual que en el
 * primer uso: al 200% el cuerpo de varios pasos no entra, y un botón que hay
 * que ir a buscar scrolleando es un botón que la persona mayor no encuentra. El
 * panel se levanta con el teclado —`safeDrawing` incluye el IME— así que el
 * botón sigue a la vista mientras se escribe el correo o la clave.
 */
@Composable
internal fun PantallaDeAlta(
    paso: PasoDelAlta,
    indice: Int,
    total: Int,
    estado: EstadoDelAlta,
    falla: FallaDeAlta?,
    impedimento: String?,
    puedeAvanzar: Boolean,
    url: String,
    onUrl: (String) -> Unit,
    ayudaDeDireccion: String,
    errorDeDireccion: String?,
    nombre: String,
    onNombre: (String) -> Unit,
    rubro: Rubro?,
    onRubro: (Rubro) -> Unit,
    email: String,
    onEmail: (String) -> Unit,
    errorDeCorreo: String?,
    clave: String,
    onClave: (String) -> Unit,
    errorDeClave: String?,
    onAtras: () -> Unit,
    onAvanzar: () -> Unit,
    onCrear: () -> Unit,
    onIrAEntrar: () -> Unit,
    esFeria: Boolean = false,
    nube: Boolean = false,
    modifier: Modifier = Modifier,
) {
    if (estado == EstadoDelAlta.LugarOcupado) {
        LugarOcupado(
            onIrAEntrar = onIrAEntrar,
            onAtras = onAtras,
            nube = nube,
            modifier = modifier,
        )
        return
    }

    val dimens = RbTheme.dimens
    val ocupado = estado != EstadoDelAlta.Preguntando
    val ultimo = indice == total - 1
    val scroll = rememberScrollState()

    // Cuando aparece una falla, se baja hasta ella: nace debajo de la pregunta y
    // al 200% eso puede estar fuera de cuadro. Un mensaje que no se ve no dice
    // nada, y la dueña vuelve a tocar el botón creyendo que no pasó nada.
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
            title = tituloDelPaso(paso, esFeria),
            subtitle = "Paso ${indice + 1} de $total",
            onBack = if (ocupado) null else onAtras,
        )

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(scroll)
                // space4 vertical: cartel con aire, no formulario apretado.
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            when (paso) {
                PasoDelAlta.Donde -> PasoDonde(
                    url = url,
                    onUrl = onUrl,
                    ayuda = ayudaDeDireccion,
                    error = errorDeDireccion,
                    habilitado = !ocupado,
                )

                PasoDelAlta.Negocio -> PasoNegocio(
                    nombre = nombre,
                    onNombre = onNombre,
                    habilitado = !ocupado,
                    onListo = onAvanzar,
                    esFeria = esFeria,
                )

                PasoDelAlta.Rubro -> PasoRubro(
                    elegido = rubro,
                    onElegir = onRubro,
                    habilitado = !ocupado,
                )

                PasoDelAlta.Cuenta -> PasoCuenta(
                    email = email,
                    onEmail = onEmail,
                    errorDeCorreo = errorDeCorreo,
                    clave = clave,
                    onClave = onClave,
                    errorDeClave = errorDeClave,
                    habilitado = !ocupado,
                    onListo = onCrear,
                )
            }

            // Tarjeta, no muro rojo: la instrucción se lee como el resto del
            // cartel, no como un banner de sistema gritando.
            falla?.let {
                TarjetaDeFalla(
                    titulo = it.titulo,
                    queHacer = it.queHacer,
                )
            }

            impedimento?.let { motivo ->
                Text(
                    text = motivo,
                    style = RbTheme.typography.support,
                    color = RbTheme.colors.textSecondary,
                )
            }
        }

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(RbTheme.colors.outline),
        )

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(RbTheme.colors.surfaceRaised)
                .windowInsetsPadding(
                    WindowInsets.safeDrawing.only(
                        WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal,
                    ),
                )
                .padding(dimens.space3),
        ) {
            // Primary = brand fill (56dp vía RbButton). Un solo verbo abajo.
            RbButton(
                label = etiquetaDelBoton(estado, ultimo, esFeria),
                onClick = if (ultimo) onCrear else onAvanzar,
                enabled = puedeAvanzar,
                fillWidth = true,
            )
        }
    }
}

/**
 * Este lugar ya tiene un negocio.
 *
 * No es una falla del alta sino su respuesta correcta: no hay nada que crear
 * acá. Se dice antes de pedir un solo dato y se ofrece la puerta que sí sirve.
 * Mismo lenguaje de cartel: una tarjeta con aire y un CTA de marca.
 *
 * @param nube en el APK de la nube no hay "computador" ni "dirección" que
 *   revisar: el feriante entra con correo y clave.
 */
@Composable
private fun LugarOcupado(
    onIrAEntrar: () -> Unit,
    onAtras: () -> Unit,
    nube: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card
    val tituloBarra = if (nube) "Ahí ya hay un puesto" else "Ahí ya hay un negocio"
    val titulo = if (nube) {
        "Ese puesto ya está creado"
    } else {
        "Ese computador ya tiene un negocio creado"
    }
    val cuerpo = if (nube) {
        "No se puede crear otro encima. Si ese puesto es tuyo, entra con tu correo y tu clave."
    } else {
        "No se puede crear otro encima. Si ese negocio es tuyo, entra con tu " +
            "correo y tu clave. Si nunca creaste uno ahí, revisa la dirección con quien " +
            "instaló el sistema."
    }
    val boton = if (nube) "Entrar a ese puesto" else "Entrar a ese negocio"

    Column(modifier = modifier.fillMaxSize()) {
        RbTopBar(
            title = tituloBarra,
            onBack = onAtras,
        )
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = dimens.space3, vertical = dimens.space4),
            verticalArrangement = Arrangement.spacedBy(dimens.space4),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(shape)
                    .background(colors.surfaceRaised)
                    .border(dimens.focusRing, colors.outlineStrong, shape)
                    .padding(horizontal = dimens.space3, vertical = dimens.space4),
                verticalArrangement = Arrangement.spacedBy(dimens.space3),
            ) {
                Text(
                    text = titulo,
                    style = RbTheme.typography.title,
                    color = colors.textPrimary,
                    modifier = Modifier.rbHeading(),
                )
                Text(
                    text = cuerpo,
                    style = RbTheme.typography.body,
                    color = colors.textPrimary,
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
        ) {
            RbButton(
                label = boton,
                onClick = onIrAEntrar,
                fillWidth = true,
            )
        }
    }
}

/**
 * Falla del alta dibujada como **tarjeta**, no como muro rojo.
 *
 * Título + qué hacer, con el mismo peso visual que el cartel del paso. El
 * borde grueso y la superficie elevada la hacen visible sin pintar la pantalla
 * de dangerContainer (que al sol y al 200% se lee como alarma, no como ayuda).
 */
@Composable
internal fun TarjetaDeFalla(
    titulo: String,
    queHacer: String,
    modifier: Modifier = Modifier,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surfaceRaised)
            .border(dimens.focusRing, colors.outlineStrong, shape)
            .padding(dimens.space3)
            .rbAssertive(),
        verticalArrangement = Arrangement.spacedBy(dimens.space2),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        Text(
            text = queHacer,
            style = RbTheme.typography.body,
            color = colors.textPrimary,
        )
    }
}

internal fun tituloDelPaso(paso: PasoDelAlta, esFeria: Boolean = false): String = when (paso) {
    PasoDelAlta.Donde -> "¿Dónde se guarda todo?"
    PasoDelAlta.Negocio ->
        if (esFeria) "¿Cómo te dicen en la feria?" else "¿Cómo se llama tu negocio?"
    PasoDelAlta.Rubro -> "¿A qué se dedica?"
    PasoDelAlta.Cuenta -> "Tu cuenta"
}

/**
 * Lo que dice el botón.
 *
 * Mientras se está trabajando dice qué se está haciendo, no "Espere": una app
 * que dice "Creando tu negocio..." está contando algo; una que dice "Cargando"
 * sólo está pidiendo paciencia.
 */
internal fun etiquetaDelBoton(
    estado: EstadoDelAlta,
    ultimo: Boolean,
    esFeria: Boolean = false,
): String = when (estado) {
    EstadoDelAlta.RevisandoElLugar -> "Revisando..."
    EstadoDelAlta.Creando ->
        if (esFeria) "Creando tu puesto..." else "Creando tu negocio..."
    else -> when {
        ultimo && esFeria -> "Crear mi puesto"
        ultimo -> "Crear mi negocio"
        else -> "Siguiente"
    }
}
