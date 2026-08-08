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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import cl.rutbusiness.app.ui.offline.EstadoRespaldoUi
import cl.rutbusiness.app.ui.offline.FranjaDeConexion
import cl.rutbusiness.app.ui.offline.LocalOffline
import cl.rutbusiness.app.ui.offline.PantallaDeCola
import cl.rutbusiness.app.ui.offline.hayConexion
import cl.rutbusiness.app.ui.offline.ventasEnCola
import cl.rutbusiness.app.ui.rubro.packActual
import cl.rutbusiness.core.backup.UserBackupApi
import cl.rutbusiness.core.backup.envelopeToBase64
import cl.rutbusiness.core.backup.mensajeErrorMaterial
import cl.rutbusiness.core.backup.mensajeTrasSubida
import cl.rutbusiness.core.backup.parsearMaterialRecuperacion
import cl.rutbusiness.core.backup.prepararRespaldoDesdeCola
import cl.rutbusiness.core.backup.restaurarDesdeTextos
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.launch

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
    // No saveable: es un mensaje efímero de la última preparación.
    var estadoRespaldo by remember { mutableStateOf<EstadoRespaldoUi?>(null) }
    var ultimoSobreBase64 by remember { mutableStateOf<String?>(null) }
    var subiendoRespaldo by remember { mutableStateOf(false) }
    val pack = packActual()

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
                                    val b64 = ok.sobre?.let { envelopeToBase64(it.envelopeBytes) }
                                    if (b64 != null) ultimoSobreBase64 = b64
                                    estadoRespaldo = EstadoRespaldoUi(
                                        mensaje = ok.mensaje,
                                        bytes = ok.sobre?.envelopeBytes?.size
                                            ?: ok.bytesPlaintext,
                                        ventas = ok.ventasEnCola,
                                    )
                                    // Intentar subir ciphertext si hay sesión y red.
                                    val sobre = ok.sobre
                                    if (sobre != null && sesion != null && conectado) {
                                        subiendoRespaldo = true
                                        alcance.launch {
                                            try {
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
                                                when (
                                                    val r = UserBackupApi(api).subirSobre(sobre)
                                                ) {
                                                    is Resultado.Ok -> {
                                                        estadoRespaldo = EstadoRespaldoUi(
                                                            mensaje = mensajeTrasSubida(
                                                                ok,
                                                                r.valor,
                                                            ),
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
                        val r = restaurarDesdeTextos(materialRaw, sobreB64)
                        estadoRespaldo = r.fold(
                            onSuccess = {
                                EstadoRespaldoUi(
                                    mensaje = it.mensaje,
                                    bytes = 0,
                                    ventas = it.ventasEnCola,
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
                    },
                    ultimoSobreBase64 = ultimoSobreBase64,
                    estadoRespaldo = estadoRespaldo,
                    subiendoRespaldo = subiendoRespaldo,
                )
            } else {
                estados.SaveableStateProvider(destino.name) {
                    contenido(destino)
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
