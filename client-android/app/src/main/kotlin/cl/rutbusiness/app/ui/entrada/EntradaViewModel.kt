package cl.rutbusiness.app.ui.entrada

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.ServerUrl
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.launch

/**
 * La cabeza de la pantalla de entrada.
 *
 * Lo que hace distinto a la versión anterior: **antes de mandar la contraseña,
 * averigua si hay a dónde mandarla.** La versión anterior tiraba el login y
 * traducía lo que volviera a un solo mensaje que decía las tres cosas a la vez
 * —"revisa que tengas internet, que el PC esté prendido y que la dirección esté
 * bien escrita"—. Eso no es un mensaje: es una lista de tareas para alguien que
 * ya no sabe qué hacer.
 *
 * El orden del diagnóstico va de lo más barato de descartar a lo más caro, y
 * cada paso que pasa deja de ser candidato:
 *
 * 1. ¿La dirección se entiende? Si no, se dice en el campo mismo mientras se
 *    escribe, sin gastar un viaje a la red.
 * 2. ¿Hay red en el teléfono? Es instantáneo, y descartarlo acá evita diez
 *    segundos de reloj girando para terminar culpando al computador del local.
 * 3. ¿Contesta el sistema del negocio en esa dirección? Se pregunta con
 *    [ProbadorDeServidor], que no manda credenciales.
 * 4. Recién ahí se intenta entrar. Si falla, ya sabemos que la falla es de los
 *    datos y no de la conexión, y se puede decir así.
 */
class EntradaViewModel(
    private val sesion: SessionRepository,
    private val probador: ProbadorDeServidor,
    private val red: RedDelTelefono?,
    inicial: EstadoSesion.SinSesion,
) : ViewModel() {

    /**
     * La dirección guardada, mostrada como ella la escribió.
     *
     * Se le saca el `http://` que le agregó `ServerUrl.normalizar`, para que al
     * volver a esta pantalla vea lo mismo que tecleó y no una versión con
     * maquinaria adelante. **Sólo `http://`**: un `https://` lo escribió ella y
     * sacárselo cambiaría a dónde nos conectamos, porque el normalizador asume
     * `http` cuando no hay esquema.
     */
    var url by mutableStateOf(inicial.baseUrl?.removePrefix("http://") ?: "")
        private set
    var negocio by mutableStateOf(inicial.tenant.orEmpty())
        private set
    var email by mutableStateOf(inicial.email.orEmpty())
        private set
    var password by mutableStateOf("")
        private set

    var enviando by mutableStateOf(false)
        private set
    var probando by mutableStateOf(false)
        private set

    /** La falla de la última vez que se intentó. Ver [FallaDeConexion]. */
    var falla by mutableStateOf<FallaDeConexion?>(null)
        private set

    /** Se prende cuando el sondeo encontró el sistema del negocio. */
    var conexionConfirmada by mutableStateOf(false)
        private set

    /**
     * La dirección en su forma canónica, o `null` si no hay forma de entenderla.
     *
     * Se recalcula en cada tecla: es una función pura sobre una cadena corta y
     * cuesta menos que decidir cuándo recalcularla.
     */
    val direccion: String?
        get() = ServerUrl.normalizar(url)

    /**
     * Qué decirle al campo de la dirección mientras se escribe.
     *
     * Mientras está vacío no dice nada: marcar en rojo un campo que la persona
     * todavía no llenó la acusa de un error que no cometió. Sólo cuando hay
     * algo escrito y no se entiende aparece el reclamo, y aparece con un
     * ejemplo, no con una regla.
     */
    val errorDeDireccion: String?
        get() = when {
            url.isBlank() -> null
            direccion == null -> "Así no se entiende. Tiene que verse como 192.168.1.10:8080"
            else -> null
        }

    /** Qué dice el campo cuando la dirección sí se entiende. */
    val ayudaDeDireccion: String
        get() = direccion?.let { "Nos vamos a conectar a ${comoSeLee(it)}" }
            ?: "El número que te dio quien instaló el sistema, con los dos puntos y todo."

    val puedeProbar: Boolean
        get() = !enviando && !probando && direccion != null

    val puedeEntrar: Boolean
        get() = !enviando && !probando && direccion != null &&
            negocio.isNotBlank() && email.isNotBlank() && password.isNotBlank()

    /** Qué falta para poder entrar, dicho una cosa a la vez. */
    fun impedimentoParaEntrar(): String? = when {
        url.isBlank() -> "Escribe la dirección del computador del negocio."
        direccion == null -> "Revisa la dirección: todavía no se entiende."
        negocio.isBlank() -> "Falta el nombre corto de tu negocio."
        email.isBlank() -> "Falta tu correo."
        password.isBlank() -> "Falta tu clave."
        else -> null
    }

    fun cambiarUrl(valor: String) {
        url = valor
        olvidarElIntentoAnterior()
    }

    fun cambiarNegocio(valor: String) {
        negocio = valor
        falla = null
    }

    fun cambiarEmail(valor: String) {
        email = valor
        falla = null
    }

    fun cambiarPassword(valor: String) {
        password = valor
        falla = null
    }

    /**
     * Probar la dirección sola, sin credenciales.
     *
     * Existe como botón propio porque los dos datos no llegan juntos: la
     * dirección se la da quien instaló el sistema y la clave se la sabe ella.
     * Poder confirmar la dirección apenas la escribe, sin tener que inventar una
     * clave para que la app le conteste algo, es la diferencia entre resolverlo
     * en un minuto y esperar a que vuelva el técnico.
     */
    fun probarConexion() {
        val destino = direccion ?: return
        if (probando || enviando) return

        probando = true
        falla = null
        conexionConfirmada = false

        viewModelScope.launch {
            val problema = diagnosticar(destino)
            falla = problema
            conexionConfirmada = problema == null
            probando = false
        }
    }

    fun entrar() {
        if (!puedeEntrar) return
        val destino = direccion ?: return

        enviando = true
        falla = null

        viewModelScope.launch {
            val problema = diagnosticar(destino)
            if (problema != null) {
                falla = problema
                conexionConfirmada = false
                enviando = false
                return@launch
            }
            conexionConfirmada = true

            when (val r = sesion.entrar(destino, negocio, email, password)) {
                is Resultado.Ok -> {
                    // La contraseña no sobrevive al login: ni en memoria.
                    password = ""
                    enviando = false
                }

                is Resultado.Falla -> {
                    falla = fallaDeLogin(r.error, destino)
                    enviando = false
                }
            }
        }
    }

    private suspend fun diagnosticar(destino: String): FallaDeConexion? = diagnosticarLaEntrada(
        direccion = destino,
        hayRed = red?.let { { it.hayRed() } },
        sondear = probador::sondear,
    )

    /**
     * Cambiar la dirección invalida lo que sabíamos del intento anterior.
     *
     * Dejar el «conexión confirmada» prendido después de que la dueña editó la
     * dirección sería mentirle sobre una dirección que nunca probamos.
     */
    private fun olvidarElIntentoAnterior() {
        falla = null
        conexionConfirmada = false
    }
}
