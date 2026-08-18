package cl.rutbusiness.app.ui.assist

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.toMutableStateList
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.rubro.RubroPack
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.launch

/** Un turno de la conversación. */
sealed interface Mensaje {
    val id: Long

    /** Lo que escribió la dueña. */
    data class Mia(override val id: Long, val texto: String) : Mensaje

    /** Una respuesta de lectura del agente. */
    /**
     * [noEntendido] es true cuando el server contestó `intent: "desconocido"`:
     * la pregunta no calzó con nada. Ese es el momento más importante de todo
     * el producto (encargo ola 30, paso 4) — es cuando la dueña decide si la
     * app le sirve o no — así que la pantalla lo marca para ofrecer algo
     * tocable en vez de dejarla frente a un texto sin salida.
     */
    data class DelAgente(override val id: Long, val texto: String, val noEntendido: Boolean = false) : Mensaje

    /**
     * El agente quiere escribir algo y espera confirmación.
     *
     * [pregunta] se guarda para poder volver a pedir lo mismo si el token
     * vence: la dueña no tiene por qué re-escribir la frase.
     */
    data class Propuesta(
        override val id: Long,
        val pregunta: String,
        val propuesta: PropuestaAccion,
        val estado: EstadoPropuesta,
    ) : Mensaje

    /**
     * Algo salió mal. [preguntaParaReintentar] no es null cuando reintentar
     * tiene sentido, y guarda la pregunta original para no perderla.
     *
     * Lleva [titulo] además del cuerpo porque se dibuja con el mismo bloque de
     * error que el resto de la app. Antes era un párrafo suelto en un fondo
     * rojo, sin título y sin anuncio para el lector de pantalla: la única
     * pantalla del producto que decía sus fallas distinto que las demás.
     */
    data class Problema(
        override val id: Long,
        val titulo: String,
        val texto: String,
        val preguntaParaReintentar: String?,
    ) : Mensaje
}

sealed interface EstadoPropuesta {
    /** Esperando que la dueña lea y decida. */
    data object Esperando : EstadoPropuesta

    /** Ya apretó confirmar y el server está trabajando. */
    data object Confirmando : EstadoPropuesta

    /** Corrió. [texto] es la confirmación en español que devolvió el server. */
    data class Hecha(val texto: String) : EstadoPropuesta

    /** La dueña dijo que no. */
    data object Cancelada : EstadoPropuesta

    /**
     * El token ya no sirve: se acabó el tiempo, o el server lo rechazó porque
     * ya se había usado. En los dos casos la salida es la misma —volver a
     * pedirlo— así que la pantalla no necesita distinguirlos.
     */
    data class YaNoSirve(val texto: String) : EstadoPropuesta
}

/**
 * La pantalla donde la dueña le habla al negocio.
 *
 * Dos reglas que valen más que el resto:
 *
 * **Nada se escribe sin que ella lo lea.** El server ya lo garantiza —propone
 * primero, ejecuta solo contra el token— y la pantalla no lo puede saltar
 * aunque quisiera: [confirmar] es lo único que llama a `/act`, y solo se llega
 * ahí apretando el botón de la tarjeta.
 *
 * **La conversación no se pierde.** Si falla la red, el turno fallido queda
 * como un mensaje con su botón de reintentar y la pregunta guardada adentro;
 * los mensajes anteriores no se tocan. El `ViewModel` sobrevive rotaciones y
 * el ir y venir entre pantallas.
 *
 * Lo que **no** hace: guardar la conversación en disco. Eso necesitaría el
 * almacenamiento de `:core`, que es de otro carril; hoy cerrar la app la
 * borra.
 */
/**
 * Título del top bar del agente.
 *
 * Sin nombre: feria dice **puesto**; farmacia/retail sigue con **negocio**.
 * Con nombre: siempre `"Pídele a $nombre"`.
 */
internal fun tituloAgente(nombre: String?, feria: Boolean): String {
    val limpio = nombre?.takeIf { it.isNotBlank() }
    return when {
        limpio != null -> "Pídele a $limpio"
        feria -> "Pídele a tu puesto"
        else -> "Pídele a tu negocio"
    }
}

class AssistViewModel(
    private val sesion: SessionRepository,
    /** El reloj, inyectable para poder probar el vencimiento sin esperar. */
    private val ahoraEpochSegundos: () -> Long = { System.currentTimeMillis() / 1000 },
) : ViewModel() {

    private var siguienteId = 1L

    val mensajes = emptyList<Mensaje>().toMutableStateList()

    /** Lo que hay tipeado en el campo de abajo. */
    var borrador by mutableStateOf("")
        private set

    /** True mientras el server contesta una pregunta. */
    var pensando by mutableStateOf(false)
        private set

    /** True hasta el primer mensaje: es cuando se enseña qué se puede pedir. */
    val recienEmpezando: Boolean get() = mensajes.isEmpty()

    /** Nombre del puesto, si el último login lo trajo. */
    var nombreDelPuesto by mutableStateOf<String?>(null)
        private set

    /**
     * Pack feria: la Screen llama [modoFeria]; el ViewModel no lee
     * CompositionLocals (`esFeria()`).
     */
    var esFeria by mutableStateOf(false)
        private set

    val titulo: String
        get() = tituloAgente(nombreDelPuesto, esFeria)

    /** Rubro activo, para las sugerencias contextuales (ver [AssistSugerenciasContexto]). */
    var pack by mutableStateOf<RubroPack?>(null)
        private set

    /** Etiqueta (`Action::label()`) de la última propuesta confirmada con éxito. */
    private var ultimaAccionConfirmada by mutableStateOf<String?>(null)

    /** Preguntas ya mandadas en esta conversación: no repetirlas como sugerencia. */
    private val sugerenciasUsadas = mutableSetOf<String>()

    /** True mientras se muestra la lista completa de qué se le puede preguntar. */
    var ayudaAbierta by mutableStateOf(false)
        private set

    /**
     * Qué ofrecer justo después del último turno — vacío mientras no haya
     * nada contextual que decir (recién empezando ya lo cubre la bienvenida).
     */
    val sugerenciasSiguientes: List<String>
        get() {
            val p = pack ?: return emptyList()
            if (mensajes.isEmpty() || ultimaAccionConfirmada == null) return emptyList()
            return AssistSugerenciasContexto.para(
                pack = p,
                recienEmpezando = false,
                ultimaAccionConfirmada = ultimaAccionConfirmada,
                yaUsadas = sugerenciasUsadas,
            )
        }

    /** Qué ofrecer justo debajo de un "no te entendí" (paso 4 del encargo). */
    fun sugerenciasCuandoNoEntiende(): List<String> {
        val p = pack ?: return emptyList()
        return AssistSugerenciasContexto.cuandoNoEntiende(
            pack = p,
            ultimaAccionConfirmada = ultimaAccionConfirmada,
            yaUsadas = sugerenciasUsadas,
        )
    }

    fun abrirAyuda() {
        ayudaAbierta = true
    }

    fun cerrarAyuda() {
        ayudaAbierta = false
    }

    init {
        viewModelScope.launch {
            nombreDelPuesto = sesion.nombreDelPuesto()
        }
    }

    /**
     * Baja el flag de feria desde la Screen (`esFeria()` no se puede llamar acá).
     * Solo cambia el copy del título cuando no hay nombre del puesto.
     */
    fun modoFeria(v: Boolean = true) {
        esFeria = v
    }

    /** Baja el rubro activo desde la Screen, igual que [modoFeria]. */
    fun actualizarPack(p: RubroPack) {
        pack = p
    }

    fun escribir(texto: String) {
        borrador = texto
    }

    fun enviarBorrador() {
        val texto = borrador.trim()
        if (texto.isEmpty() || pensando) return
        borrador = ""
        preguntar(texto)
    }

    /** Manda una pregunta, venga del campo o de una sugerencia. */
    fun preguntar(pregunta: String) {
        val limpia = pregunta.trim()
        if (limpia.isEmpty() || pensando) return

        mensajes += Mensaje.Mia(nuevoId(), limpia)
        sugerenciasUsadas += limpia
        pensando = true

        viewModelScope.launch {
            val api = sesion.apiActiva()
            if (api == null) {
                pensando = false
                val copy = AppError.SesionExpirada().aCopy("tu pregunta")
                mensajes += Mensaje.Problema(
                    id = nuevoId(),
                    titulo = copy.title,
                    texto = copy.message,
                    preguntaParaReintentar = null,
                )
                return@launch
            }

            when (val r = AssistApi(api).preguntar(limpia)) {
                is Resultado.Ok -> {
                    val propuesta = r.valor.action
                    mensajes += if (propuesta != null) {
                        Mensaje.Propuesta(
                            id = nuevoId(),
                            pregunta = limpia,
                            propuesta = propuesta,
                            estado = EstadoPropuesta.Esperando,
                        )
                    } else {
                        Mensaje.DelAgente(
                            id = nuevoId(),
                            texto = r.valor.text,
                            noEntendido = r.valor.intent == "desconocido",
                        )
                    }
                }

                is Resultado.Falla -> {
                    // `aCopy` y no `userMessage` a secas: el mensaje crudo de
                    // `ServidorNoResponde` trae la URL del server adentro, y la
                    // dueña no tiene por qué leer una dirección técnica en medio
                    // de una conversación. Es además lo que usan las otras
                    // cuatro pantallas para lo mismo.
                    val copy = r.error.aCopy("tu pregunta")
                    mensajes += Mensaje.Problema(
                        id = nuevoId(),
                        titulo = copy.title,
                        texto = copy.message,
                        // La pregunta se guarda entera: reintentar no le cuesta
                        // nada a quien ya la escribió una vez. Salvo cuando
                        // reintentar no puede servir —sesión vencida, sin
                        // permiso—, que es justo lo que dice `retryLabel`.
                        preguntaParaReintentar = limpia.takeIf { copy.retryLabel != null },
                    )
                }
            }
            pensando = false
        }
    }

    /**
     * Ejecuta la acción propuesta.
     *
     * El estado pasa a [EstadoPropuesta.Confirmando] **antes** de salir a la
     * red, y la tarjeta esconde sus botones en ese estado. Con eso, dos toques
     * seguidos no son dos ejecuciones. El server igual rechaza el segundo
     * token —es de un solo uso— pero eso es la red de atrás, no la de
     * adelante: la dueña nunca debería llegar a verla.
     */
    fun confirmar(mensajeId: Long) {
        val indice = mensajes.indexOfFirst { it.id == mensajeId }
        if (indice < 0) return
        val mensaje = mensajes[indice] as? Mensaje.Propuesta ?: return
        if (mensaje.estado != EstadoPropuesta.Esperando) return

        mensajes[indice] = mensaje.copy(estado = EstadoPropuesta.Confirmando)

        viewModelScope.launch {
            val api = sesion.apiActiva()
            if (api == null) {
                reemplazar(mensajeId) {
                    it.copy(
                        estado = EstadoPropuesta.YaNoSirve(
                            "Tu sesión venció, así que no alcancé a guardarlo. " +
                                "Vuelve a entrar y pídemelo de nuevo.",
                        ),
                    )
                }
                return@launch
            }

            when (val r = AssistApi(api).confirmar(mensaje.propuesta.confirmToken)) {
                // La confirmación queda DENTRO de la tarjeta y no se repite como
                // burbuja aparte. Agregarla dos veces se veía en el teléfono
                // como si el agente hubiera contestado dos veces —o peor, como
                // si hubiera registrado el gasto dos veces, que es exactamente
                // el susto que esta pantalla existe para evitar.
                is Resultado.Ok -> {
                    ultimaAccionConfirmada = mensaje.propuesta.name
                    reemplazar(mensajeId) {
                        it.copy(estado = EstadoPropuesta.Hecha(r.valor.text))
                    }
                }

                is Resultado.Falla -> reemplazar(mensajeId) {
                    it.copy(estado = EstadoPropuesta.YaNoSirve(mensajeDeRechazo(r.error.userMessage)))
                }
            }
        }
    }

    fun cancelar(mensajeId: Long) {
        reemplazar(mensajeId) {
            if (it.estado == EstadoPropuesta.Esperando) {
                it.copy(estado = EstadoPropuesta.Cancelada)
            } else {
                it
            }
        }
    }

    /**
     * Marca una propuesta como vencida sin llamar al server.
     *
     * La llama la pantalla cuando el contador local llega a cero, para que la
     * tarjeta deje de ofrecer un botón que ya no va a funcionar. Apretarlo
     * igual sería seguro —el server rechaza— pero ofrecer algo que va a
     * fallar es una promesa rota.
     */
    fun marcarVencida(mensajeId: Long) {
        reemplazar(mensajeId) {
            if (it.estado == EstadoPropuesta.Esperando) {
                it.copy(
                    estado = EstadoPropuesta.YaNoSirve(
                        "Pasó el tiempo para confirmarla, así que no guardé nada.",
                    ),
                )
            } else {
                it
            }
        }
    }

    /** Vuelve a pedir lo mismo: repite la pregunta original tal cual. */
    fun volverAPedir(pregunta: String) {
        preguntar(pregunta)
    }

    fun segundosRestantesDe(propuesta: PropuestaAccion): Long? =
        Vencimiento.segundosRestantes(propuesta.expiresAt, ahoraEpochSegundos())

    private fun reemplazar(mensajeId: Long, transformar: (Mensaje.Propuesta) -> Mensaje.Propuesta) {
        val indice = mensajes.indexOfFirst { it.id == mensajeId }
        if (indice < 0) return
        val actual = mensajes[indice] as? Mensaje.Propuesta ?: return
        mensajes[indice] = transformar(actual)
    }

    private fun nuevoId(): Long = siguienteId++

}

/**
 * Reescribe el rechazo del server para que lo entienda una persona — y sin
 * afirmar algo que no sabemos.
 *
 * El server dice «El token de confirmación es inválido o ya fue usado» o
 * «El token de confirmación expiró». "Token" es jerga: la dueña no sabe que
 * existe uno y no puede hacer nada con esa palabra.
 *
 * Pero el problema de fondo no es la jerga. **El server no distingue los
 * dos casos a propósito** (distinguirlos sería un oráculo para adivinar
 * tokens), y esos dos casos terminan distinto:
 *
 * - venció → no se guardó nada, volver a pedirlo es correcto;
 * - ya se usó → **sí se guardó**, y volver a pedirlo anota el gasto DOS
 *   veces.
 *
 * Con datos móviles intermitentes —el caso normal según el piso de
 * hardware— el segundo escenario no es teórico: la petición llega, el
 * server escribe, y la respuesta se pierde en el camino de vuelta. Desde el
 * teléfono eso se ve idéntico a un fallo.
 *
 * Por eso este mensaje **no promete** que no se guardó nada. Manda a
 * revisar primero. Un texto que suena más tranquilizador y que provoca un
 * gasto duplicado es peor que uno que pide un chequeo de diez segundos.
 *
 * El caso que sí sabemos con certeza —que el contador local llegó a cero
 * sin haber mandado nada— tiene su propio texto en [marcarVencida], y ese
 * sí afirma que no se guardó nada, porque ahí es verdad.
 *
 * Cualquier otro error se deja pasar tal cual: `AppError` ya trae mensajes
 * escritos para el usuario, y taparlos escondería el problema real.
 */
internal fun mensajeDeRechazo(mensajeDelServidor: String): String {
    val bajo = mensajeDelServidor.lowercase()
    val esDeToken = bajo.contains("token") &&
        (bajo.contains("expir") || bajo.contains("inválid") ||
            bajo.contains("invalid") || bajo.contains("usado"))

    return if (esDeToken) {
        "No me llegó tu confirmación a tiempo. Puede que igual haya alcanzado " +
            "a guardarse: revísalo antes de pedírmelo de nuevo, para no anotarlo " +
            "dos veces."
    } else {
        mensajeDelServidor
    }
}
