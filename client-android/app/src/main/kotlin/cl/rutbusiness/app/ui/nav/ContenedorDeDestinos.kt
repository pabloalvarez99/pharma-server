package cl.rutbusiness.app.ui.nav

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.systemBars
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import cl.rutbusiness.app.ui.catalogo.CatalogoRoute
import cl.rutbusiness.app.ui.catalogo.LocalAbrirCatalogo
import cl.rutbusiness.app.ui.offline.EstadoRespaldoUi
import cl.rutbusiness.app.ui.offline.FranjaDeConexion
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.offline.PantallaDeCola
import cl.rutbusiness.app.ui.offline.hayConexion
import cl.rutbusiness.app.ui.offline.ventasEnCola
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.app.ui.offline.ResultadoTraerNube
import cl.rutbusiness.core.backup.PruebaDeRetiro
import cl.rutbusiness.core.backup.UserBackupApi
import cl.rutbusiness.core.backup.conRehidratacion
import cl.rutbusiness.core.backup.envelopeToBase64
import cl.rutbusiness.core.backup.guardarSobreLocal
import cl.rutbusiness.core.backup.leerSobreLocalBase64
import cl.rutbusiness.core.backup.mensajeErrorMaterial
import cl.rutbusiness.core.backup.mensajeTrasSubida
import cl.rutbusiness.core.backup.parsearMaterialRecuperacion
import cl.rutbusiness.core.backup.prepararRespaldoDesdeCola
import cl.rutbusiness.core.backup.restaurarDesdeTextos
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import androidx.compose.runtime.LaunchedEffect

/**
 * Lo que se le agrega al mensaje de subida cuando el sobre viajó **sin** carril
 * de rescate.
 *
 * Pasa cuando no se pudo derivar la prueba de retiro: sin frase (respaldo sin
 * cifrar) o sin el slug del login. El sobre está guardado igual y se puede
 * abrir desde este teléfono; lo que no se puede es pedirlo desde uno nuevo. Se
 * dice, porque el silencio acá es justo la clase de confianza mal puesta que la
 * tarjeta impresa promete y el server no cumplía.
 */
private fun avisoSinRescate(hashRetiro: String?): String =
    if (hashRetiro != null) {
        ""
    } else {
        " · Ojo: este respaldo no se va a poder bajar desde un teléfono nuevo. " +
            "Preparalo con tus 12 palabras después de entrar con el nombre corto " +
            "del negocio."
    }

/**
 * El armazón de la navegación: qué pestaña está elegida, qué se dibuja arriba
 * de la barra, y qué hace el botón físico de atrás.
 *
 * No conoce ninguna pantalla - recibe [contenido] y le pasa el destino. Eso no
 * es abstracción por gusto: es lo que deja probar la parte que **puede
 * romperse** (que el estado de una pestaña sobreviva al cambio a otra) sin
 * levantar un servidor ni un carrito real. `NavegacionTest` monta este mismo
 * contenedor con pantallas de mentira y verifica lo que el producto promete.
 *
 * **Por qué el estado sobrevive.** El [rememberSaveableStateHolder] guarda el
 * `rememberSaveable` de cada destino bajo su propia llave, y lo devuelve al
 * volver. Los `ViewModel` no dependen de esto: `viewModel()` los cuelga del
 * `ViewModelStore` del `Activity` y salir de la composición no los limpia. Las
 * dos capas juntas son lo que hace que un carrito a medias siga ahí después de
 * ir a preguntarle algo al agente.
 */
@Composable
internal fun ContenedorDeDestinos(
    modifier: Modifier = Modifier,
    inicial: Destino = Destino.Agente,
    /**
     * Tenant para armar el snapshot de respaldo (ADR-0022). Puede ser el slug
     * de login o el id del JWT; si falta, se usa un placeholder local.
     */
    tenantId: String? = null,
    /**
     * Sesión activa para subir el sobre cifrado (nunca plaintext). Opcional
     * en tests de navegación sin red.
     */
    sesion: SessionRepository? = null,
    contenido: @Composable (Destino) -> Unit,
) {
    var destino by rememberSaveable { mutableStateOf(inicial) }
    val estados = rememberSaveableStateHolder()
    val alcance = rememberCoroutineScope()

    // El estado de la conexión y la cola de ventas viven arriba de las
    // pestañas porque no son de ninguna: la dueña tiene que ver "sin conexión"
    // esté donde esté, y contar las ventas que faltan mandar sin tener que
    // buscar en qué pantalla estaban.
    val offline = LocalOffline.current
    val conectado = hayConexion()
    val cola = ventasEnCola()?.value.orEmpty()
    var viendoCola by rememberSaveable { mutableStateOf(false) }
    // "Lo que vendo" se abre encima de cualquier pestaña, igual que la cola: es
    // algo que se hace **mientras** se está cobrando —"esto no lo tengo
    // cargado"— y no un lugar donde uno se queda.
    var viendoCatalogo by rememberSaveable { mutableStateOf(false) }
    // No saveable: es un mensaje efímero de la última preparación.
    var estadoRespaldo by remember { mutableStateOf<EstadoRespaldoUi?>(null) }
    var ultimoSobreBase64 by remember { mutableStateOf<String?>(null) }
    var subiendoRespaldo by remember { mutableStateOf(false) }
    val pack = packActual()

    // El slug del login, que es contra lo que el server busca en el rescate
    // (`tenant.slug`). No sirve `tenantId`: ése puede venir del JWT como
    // `tenant:abc`, y con eso la prueba de retiro se derivaría de un texto que
    // el server nunca va a comparar. Sin slug no se arma el carril de rescate,
    // y se dice — un respaldo que no se puede bajar desde un teléfono nuevo es
    // exactamente el agujero que este trabajo vino a tapar.
    var slugDelNegocio by remember { mutableStateOf<String?>(null) }

    // Prefill del último sobre cifrado en disco (nunca la frase).
    LaunchedEffect(sesion) {
        val s = sesion ?: return@LaunchedEffect
        ultimoSobreBase64 = leerSobreLocalBase64(s.disco)
        slugDelNegocio = s.ultimoTenant()
    }

    // Atrás desde una pestaña secundaria vuelve al inicio en vez de cerrar la
    // app de golpe. En [inicial] queda deshabilitado y el sistema hace lo suyo:
    // en la pantalla de entrada, atrás sí significa salir, y atraparlo ahí
    // dejaría a la dueña sin forma de cerrar la app.
    //
    // Las pantallas de adentro registran su propio `BackHandler` -- Cobrar lo
    // usa para volver de Pago a Buscar -- y el más profundo gana, así que atrás
    // deshace el último paso antes de cambiar de pestaña. Es el orden en que la
    // gente espera que se deshagan las cosas.
    BackHandler(enabled = destino != inicial) {
        destino = inicial
    }

    // Va después del anterior a propósito: el `BackHandler` registrado más
    // tarde es el que gana, así que atrás cierra primero la lista de la cola
    // -que es lo último que se abrió- y recién después cambia de pestaña.
    BackHandler(enabled = viendoCola) { viendoCola = false }

    // Y el del catálogo después, por lo mismo. El paso de adentro -el formulario
    // de alta- registra el suyo más profundo todavía, así que atrás deshace en
    // orden: primero el formulario, después la pantalla, después la pestaña.
    BackHandler(enabled = viendoCatalogo) { viendoCatalogo = false }

    // Identidad estable: `LocalAbrirCatalogo` es `static`, así que un lambda
    // nuevo en cada recomposición invalidaría el subárbol entero -o sea, la
    // pantalla que se está usando- por nada.
    val abrirCatalogo: (() -> Unit)? = remember(sesion) {
        if (sesion == null) null else ({ viendoCatalogo = true })
    }

    Column(modifier = modifier.fillMaxSize()) {
        FranjaDeConexion(
            conectado = conectado,
            cola = cola,
            onVerCola = { viendoCola = true },
        )

        // `weight(1f)` y no `fillMaxSize()`: el contenido ocupa lo que sobra
        // después de la barra. Con `fillMaxSize` la barra queda empujada fuera
        // de la pantalla, y al 200% eso pasa siempre.
        Column(
            modifier = Modifier
                .weight(1f)
                // La barra de abajo ya se separó de la barra de gestos del
                // sistema, y las pantallas de adentro piden ese mismo margen por
                // su cuenta -- el redactor del agente y la barra de cobrar de
                // `PasoBuscar` lo hacían cuando eran lo último de la pantalla.
                // Sin consumirlo acá, el margen se aplica dos veces y queda una
                // franja muerta de dos centímetros entre el botón de enviar y
                // las pestañas.
                //
                // Se consume `systemBars` y no `safeDrawing`: `safeDrawing`
                // incluye el teclado, y consumirlo dejaría el campo de texto
                // tapado justo mientras se escribe.
                .consumeWindowInsets(WindowInsets.systemBars.only(WindowInsetsSides.Bottom)),
        ) {
            // La cola tapa el destino pero **no** la barra de pestañas ni la
            // franja: mirar qué ventas faltan mandar no puede sacar a la dueña
            // de donde estaba. Al cerrarla vuelve a la misma pantalla con el
            // mismo estado, porque el `SaveableStateProvider` sigue montado.
            if (viendoCola && offline != null) {
                PantallaDeCola(
                    cola = cola,
                    conectado = conectado,
                    ahora = offline.reloj(),
                    onIntentarAhora = { offline.despachador.intentarAhora() },
                    onDescartar = { clave -> offline.cola.descartar(clave) },
                    onCerrar = { viendoCola = false },
                    // Feria: PBKDF2 + AES-GCM en el aparato; sube solo ciphertext.
                    onPrepararRespaldo = { materialRaw ->
                        val material = if (materialRaw.isBlank()) {
                            null
                        } else {
                            val parsed = parsearMaterialRecuperacion(materialRaw)
                            if (parsed.isFailure) {
                                estadoRespaldo = EstadoRespaldoUi(
                                    mensaje = mensajeErrorMaterial(materialRaw),
                                    bytes = 0,
                                    ventas = 0,
                                )
                                null
                            } else {
                                parsed.getOrThrow()
                            }
                        }
                        // Parse falló → ya se seteo el mensaje; no rearmar.
                        if (materialRaw.isNotBlank() && material == null) {
                            // leave estadoRespaldo as set above
                        } else {
                            val prep = prepararRespaldoDesdeCola(
                                tenantId = tenantId?.takeIf { it.isNotBlank() } ?: "local",
                                cola = cola,
                                createdAtUnix = offline.reloj() / 1000L,
                                rubro = pack.rubro,
                                materialRecuperacion = material,
                            )
                            prep.fold(
                                onSuccess = { ok ->
                                    val sobre = ok.sobre
                                    val b64 = sobre?.let { envelopeToBase64(it.envelopeBytes) }
                                    if (b64 != null) ultimoSobreBase64 = b64
                                    estadoRespaldo = EstadoRespaldoUi(
                                        mensaje = ok.mensaje,
                                        bytes = sobre?.envelopeBytes?.size
                                            ?: ok.bytesPlaintext,
                                        ventas = ok.ventasEnCola,
                                    )
                                    // Disco local + upload ciphertext (nunca frase).
                                    if (sobre != null) {
                                        subiendoRespaldo = true
                                        alcance.launch {
                                            try {
                                                sesion?.disco?.let {
                                                    guardarSobreLocal(it, sobre.envelopeBytes)
                                                }
                                                if (sesion != null && conectado) {
                                                    val api = sesion.apiActiva()
                                                    if (api == null) {
                                                        estadoRespaldo = EstadoRespaldoUi(
                                                            mensaje = ok.mensaje +
                                                                " · Sin sesión para subir; " +
                                                                "el cifrado quedó en el teléfono.",
                                                            bytes = sobre.envelopeBytes.size,
                                                            ventas = ok.ventasEnCola,
                                                        )
                                                        return@launch
                                                    }
                                                    // La prueba de retiro se
                                                    // deriva acá y no en la
                                                    // preparación: son otros
                                                    // ~210.000 PBKDF2, y en el
                                                    // hilo de la composición
                                                    // eso es un segundo de
                                                    // pantalla congelada.
                                                    val slug = slugDelNegocio
                                                    val hashRetiro = if (
                                                        material != null && !slug.isNullOrBlank()
                                                    ) {
                                                        withContext(Dispatchers.Default) {
                                                            PruebaDeRetiro
                                                                .derivar(material, slug)
                                                                .map { PruebaDeRetiro.hashHex(it) }
                                                                .getOrNull()
                                                        }
                                                    } else {
                                                        null
                                                    }
                                                    when (
                                                        val r = UserBackupApi(api).subirSobre(
                                                            sobre,
                                                            hashRetiro,
                                                        )
                                                    ) {
                                                        is Resultado.Ok -> {
                                                            estadoRespaldo = EstadoRespaldoUi(
                                                                mensaje = mensajeTrasSubida(
                                                                    ok,
                                                                    r.valor,
                                                                ) + avisoSinRescate(hashRetiro),
                                                                bytes = sobre.envelopeBytes.size,
                                                                ventas = ok.ventasEnCola,
                                                            )
                                                        }
                                                        is Resultado.Falla -> {
                                                            estadoRespaldo = EstadoRespaldoUi(
                                                                mensaje = ok.mensaje +
                                                                    " · No se pudo subir: " +
                                                                    r.error.userMessage +
                                                                    " El sobre cifrado sigue acá.",
                                                                bytes = sobre.envelopeBytes.size,
                                                                ventas = ok.ventasEnCola,
                                                            )
                                                        }
                                                    }
                                                }
                                            } finally {
                                                subiendoRespaldo = false
                                            }
                                        }
                                    }
                                },
                                onFailure = {
                                    estadoRespaldo = EstadoRespaldoUi(
                                        mensaje = "No se pudo armar el paquete: ${it.message}",
                                        bytes = 0,
                                        ventas = 0,
                                    )
                                },
                            )
                        }
                    },
                    onRestaurarRespaldo = { materialRaw, sobreB64 ->
                        alcance.launch {
                            val r = restaurarDesdeTextos(materialRaw, sobreB64)
                            estadoRespaldo = r.fold(
                                onSuccess = { abierta ->
                                    val agregadas = offline.cola.fusionarDesdeRespaldo(
                                        abierta.snapshot.pendingSales,
                                    )
                                    val final = abierta.conRehidratacion(agregadas)
                                    EstadoRespaldoUi(
                                        mensaje = final.mensaje,
                                        bytes = 0,
                                        ventas = final.ventasEnCola,
                                    )
                                },
                                onFailure = {
                                    EstadoRespaldoUi(
                                        mensaje = it.message
                                            ?: "No se pudo abrir el respaldo.",
                                        bytes = 0,
                                        ventas = 0,
                                    )
                                },
                            )
                        }
                    },
                    onTraerDeLaNube = if (sesion != null) {
                        {
                            val api = sesion.apiActiva()
                            if (api == null) {
                                estadoRespaldo = EstadoRespaldoUi(
                                    mensaje = "Sin sesión para bajar de la nube.",
                                    bytes = 0,
                                    ventas = 0,
                                )
                                ResultadoTraerNube.Error("sin sesión")
                            } else {
                                val backupApi = UserBackupApi(api)
                                when (val listado = backupApi.listar()) {
                                    is Resultado.Falla -> {
                                        val msg = "No se pudo listar: ${listado.error.userMessage}"
                                        estadoRespaldo = EstadoRespaldoUi(
                                            mensaje = msg,
                                            bytes = 0,
                                            ventas = 0,
                                        )
                                        ResultadoTraerNube.Error(msg)
                                    }
                                    is Resultado.Ok -> {
                                        if (listado.valor.isEmpty()) {
                                            val msg =
                                                "No hay respaldos en este nodo. " +
                                                    "En lab: RUTBUSINESS_USER_BACKUP_MEMORY=1 " +
                                                    "y Preparar con red. " +
                                                    "En prod: falta el bucket."
                                            estadoRespaldo = EstadoRespaldoUi(
                                                mensaje = msg,
                                                bytes = 0,
                                                ventas = 0,
                                            )
                                            ResultadoTraerNube.Vacio(msg)
                                        } else {
                                            val meta = listado.valor.maxByOrNull {
                                                it.uploadedAtUnix
                                            }!!
                                            val id = meta.backupId?.takeIf { it.isNotBlank() }
                                            if (id == null) {
                                                val msg =
                                                    "Hay ${listado.valor.size} meta(s) " +
                                                        "sin backup_id (server viejo). " +
                                                        "Actualizá el nodo o usá el sobre local."
                                                estadoRespaldo = EstadoRespaldoUi(
                                                    mensaje = msg,
                                                    bytes = 0,
                                                    ventas = 0,
                                                )
                                                ResultadoTraerNube.Error(msg)
                                            } else {
                                                when (val d = backupApi.descargar(id)) {
                                                    is Resultado.Ok -> {
                                                        ultimoSobreBase64 =
                                                            d.valor.ciphertextBase64
                                                        estadoRespaldo = EstadoRespaldoUi(
                                                            mensaje =
                                                                "Bajé el sobre ${d.valor.backupId}. " +
                                                                    "Escribí la llave del cuaderno " +
                                                                    "y tocá Abrir respaldo.",
                                                            bytes = d.valor.meta.sizeBytes.toInt(),
                                                            ventas = 0,
                                                        )
                                                        ResultadoTraerNube.Ok(
                                                            d.valor.ciphertextBase64,
                                                            d.valor.backupId,
                                                        )
                                                    }
                                                    is Resultado.Falla -> {
                                                        val msg =
                                                            "No se pudo bajar $id: " +
                                                                d.error.userMessage
                                                        estadoRespaldo = EstadoRespaldoUi(
                                                            mensaje = msg,
                                                            bytes = 0,
                                                            ventas = 0,
                                                        )
                                                        ResultadoTraerNube.Error(msg)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        null
                    },
                    ultimoSobreBase64 = ultimoSobreBase64,
                    estadoRespaldo = estadoRespaldo,
                    subiendoRespaldo = subiendoRespaldo,
                )
            } else if (viendoCatalogo && sesion != null) {
                // Tapa el destino y **no** la barra de pestañas, igual que la
                // cola: cargar una cosa en medio de una venta no puede sacar a
                // la dueña de donde estaba. Al cerrarla vuelve a la misma
                // pantalla con el carrito intacto, porque el `ViewModel` de
                // Cobrar cuelga del `Activity` y el `SaveableStateProvider`
                // devuelve lo demás.
                CatalogoRoute(
                    sesion = sesion,
                    onCerrar = { viendoCatalogo = false },
                )
            } else {
                CompositionLocalProvider(LocalAbrirCatalogo provides abrirCatalogo) {
                    estados.SaveableStateProvider(destino.name) {
                        contenido(destino)
                    }
                }
            }
        }

        BarraDeNavegacion(
            actual = destino,
            onElegir = { elegido -> destino = elegido },
            agentHome = packActual().features.agentHome,
        )
    }
}
