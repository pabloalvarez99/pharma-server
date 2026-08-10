package cl.rutbusiness.app.ui.alta

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.entrada.FallaDeConexion
import cl.rutbusiness.app.ui.entrada.ProbadorDeServidor
import cl.rutbusiness.app.ui.entrada.RedDelTelefono
import cl.rutbusiness.app.ui.entrada.diagnosticarLaEntrada
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.ServerUrl
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.launch

/** Las preguntas del alta, una por pantalla. */
enum class PasoDelAlta { Donde, Negocio, Rubro, Cuenta }

/**
 * En qué está el alta ahora mismo.
 *
 * Son estados de la pantalla entera y no banderas sueltas: cada uno se dibuja
 * distinto y sólo puede haber uno. Con booleanos separados
 * (`revisando`/`ocupado`/`enviando`) siempre existe la combinación imposible
 * que alguien termina pintando.
 */
sealed interface EstadoDelAlta {
    /** Preguntando. [PasoDelAlta] dice cuál. */
    data object Preguntando : EstadoDelAlta

    /** Averiguando si el lugar está libre, antes de pedir un solo dato. */
    data object RevisandoElLugar : EstadoDelAlta

    /**
     * El lugar ya tiene un negocio: no hay nada que crear acá.
     *
     * Se descubre **antes** del formulario, con `GET /api/v1/setup/status`. Es
     * la diferencia entre decírselo en dos segundos y decírselo después de que
     * escribió el nombre del local, eligió el rubro e inventó una clave.
     */
    data object LugarOcupado : EstadoDelAlta

    /** Mandando el alta. */
    data object Creando : EstadoDelAlta
}

/**
 * La cabeza de "crear mi negocio".
 *
 * El orden del diagnóstico es el mismo que el de la entrada, y por la misma
 * razón: de lo más barato de descartar a lo más caro, y cada paso que pasa deja
 * de ser candidato. Primero si el teléfono tiene red, después si en la
 * dirección contesta un RutBusiness sano, después si ese lugar está libre, y
 * recién ahí se manda el alta.
 *
 * @param nube la dirección compilada en el APK (`BuildConfig.URL_NUBE`), o
 *   `null` cuando este APK no trae ninguna. Decide una sola cosa, pero pesada:
 *   si el alta pregunta dónde guardar o no lo pregunta nunca.
 */
class AltaViewModel(
    private val sesion: SessionRepository,
    private val probador: ProbadorDeServidor,
    private val red: RedDelTelefono?,
    nube: String?,
) : ViewModel() {

    /** La nube en forma canónica, o `null` si este APK no trae ninguna. */
    private val nube: String? = nube?.let(ServerUrl::normalizar)

    /**
     * Las preguntas que hay que hacer, en orden.
     *
     * Con nube compilada son tres y ninguna es una dirección: ése es el punto
     * entero del camino de la nube. Sin nube se agrega la de dónde guardar,
     * porque la alternativa —inventarle un destino— es peor que preguntar.
     */
    val pasos: List<PasoDelAlta> = buildList {
        if (this@AltaViewModel.nube == null) add(PasoDelAlta.Donde)
        add(PasoDelAlta.Negocio)
        add(PasoDelAlta.Rubro)
        add(PasoDelAlta.Cuenta)
    }

    var indice by mutableStateOf(0)
        private set

    var estado by mutableStateOf<EstadoDelAlta>(EstadoDelAlta.Preguntando)
        private set

    var url by mutableStateOf("")
        private set
    var nombre by mutableStateOf("")
        private set
    var rubro by mutableStateOf<Rubro?>(null)
        private set
    var email by mutableStateOf("")
        private set
    var clave by mutableStateOf("")
        private set

    var falla by mutableStateOf<FallaDeAlta?>(null)
        private set

    val paso: PasoDelAlta get() = pasos[indice]

    /** A dónde va el alta, o `null` mientras no se entienda la dirección. */
    val destino: String? get() = nube ?: ServerUrl.normalizar(url)

    /** Igual que en la entrada: no se marca en rojo un campo todavía vacío. */
    val errorDeDireccion: String?
        get() = if (url.isNotBlank() && destino == null) {
            "Así no se entiende. Tiene que verse como 192.168.1.10:8080"
        } else {
            null
        }

    val ayudaDeDireccion: String
        get() = destino?.let { "Ahí vamos a crear tu negocio: ${comoSeLee(it)}" }
            ?: "El número que te dio quien instaló el sistema, con los dos puntos y todo."

    /** El correo, con la misma forma mínima que exige `crates/api/src/setup.rs`. */
    val errorDeCorreo: String?
        get() = when {
            email.isBlank() -> null
            !email.contains('@') || !email.contains('.') || email.contains(' ') ->
                "Así no se entiende. Tiene que verse como dueno@minegocio.cl"
            else -> null
        }

    val errorDeClave: String?
        get() = if (clave.isNotEmpty() && clave.length < LARGO_MINIMO_DE_CLAVE) {
            "Muy corta: tiene que tener al menos $LARGO_MINIMO_DE_CLAVE letras o números."
        } else {
            null
        }

    /**
     * Si se puede pasar a la pregunta siguiente.
     *
     * Una regla por paso y no una sola grande: lo que falta para avanzar
     * depende de lo que se está preguntando, y juntarlo todo en un booleano
     * deja al botón apagado sin que se entienda por qué.
     */
    val puedeAvanzar: Boolean
        get() = estado == EstadoDelAlta.Preguntando && when (paso) {
            PasoDelAlta.Donde -> destino != null
            PasoDelAlta.Negocio -> nombre.isNotBlank()
            PasoDelAlta.Rubro -> rubro != null
            PasoDelAlta.Cuenta ->
                email.isNotBlank() && errorDeCorreo == null &&
                    clave.length >= LARGO_MINIMO_DE_CLAVE
        }

    /** Qué falta, dicho de a una cosa. `null` cuando no falta nada. */
    fun impedimento(): String? = when {
        estado != EstadoDelAlta.Preguntando -> null
        paso == PasoDelAlta.Donde && url.isBlank() ->
            "Escribe la dirección del computador donde se va a guardar todo."
        paso == PasoDelAlta.Donde && destino == null ->
            "Revisa la dirección: todavía no se entiende."
        paso == PasoDelAlta.Negocio && nombre.isBlank() -> "Escribe el nombre de tu negocio."
        paso == PasoDelAlta.Rubro && rubro == null -> "Toca el rubro que más se parezca al tuyo."
        paso == PasoDelAlta.Cuenta && email.isBlank() -> "Falta tu correo."
        paso == PasoDelAlta.Cuenta && errorDeCorreo != null -> "Revisa el correo."
        paso == PasoDelAlta.Cuenta && clave.length < LARGO_MINIMO_DE_CLAVE ->
            "La clave tiene que tener al menos $LARGO_MINIMO_DE_CLAVE letras o números."
        else -> null
    }

    fun cambiarUrl(valor: String) { url = valor; falla = null }
    fun cambiarNombre(valor: String) { nombre = valor; falla = null }
    fun elegirRubro(valor: Rubro) { rubro = valor; falla = null }
    fun cambiarEmail(valor: String) { email = valor; falla = null }
    fun cambiarClave(valor: String) { clave = valor; falla = null }

    /**
     * Vuelve a la pregunta anterior. `null` en la primera: ahí el botón de
     * atrás no es de este flujo sino la salida del alta, y quien decide qué
     * hacer con eso es la pantalla.
     */
    fun retroceder(): Boolean {
        if (estado != EstadoDelAlta.Preguntando || indice == 0) return false
        indice -= 1
        falla = null
        return true
    }

    /**
     * Arranca el alta.
     *
     * Con nube compilada la primera pregunta ya se puede hacer, pero antes hay
     * que saber si ese lugar sirve: preguntar el nombre del negocio y el rubro
     * para después decir "acá no se puede" es hacerle perder el tiempo a
     * alguien que ya estaba desconfiando de la app.
     */
    fun empezar() {
        if (nube != null && estado == EstadoDelAlta.Preguntando && indice == 0) {
            revisarElLugar(alTerminar = {})
        }
    }

    /** Siguiente pregunta. En la de la dirección, primero comprueba el lugar. */
    fun avanzar() {
        if (!puedeAvanzar) return
        if (paso == PasoDelAlta.Donde) {
            revisarElLugar(alTerminar = { indice += 1 })
            return
        }
        if (indice < pasos.lastIndex) indice += 1
    }

    /**
     * ¿Hay a dónde, y ese lugar está libre?
     *
     * Los dos viajes van juntos porque las dos respuestas se necesitan en el
     * mismo momento y ninguna sirve sin la otra: un lugar que no contesta no
     * puede estar libre, y uno ocupado no vale la pena aunque conteste.
     */
    private fun revisarElLugar(alTerminar: () -> Unit) {
        val donde = destino ?: return
        estado = EstadoDelAlta.RevisandoElLugar
        falla = null

        viewModelScope.launch {
            val problema = diagnosticar(donde)
            if (problema != null) {
                falla = deLaConexion(problema, donde)
                estado = EstadoDelAlta.Preguntando
                return@launch
            }

            when (val r = estadoDelServidor(sesion.apiPara(donde))) {
                is Resultado.Falla -> {
                    falla = fallaDeAlta(r.error, donde, email)
                    estado = EstadoDelAlta.Preguntando
                }

                is Resultado.Ok -> if (r.valor.necesitaAlta) {
                    estado = EstadoDelAlta.Preguntando
                    alTerminar()
                } else {
                    estado = EstadoDelAlta.LugarOcupado
                }
            }
        }
    }

    /**
     * Crea el negocio y entra.
     *
     * La clave se borra de memoria apenas la sesión queda abierta, igual que en
     * la entrada. Si el alta funcionó pero el login no, **no** se borra: la
     * dueña la va a necesitar en la pantalla de al lado, y hacerla adivinar la
     * clave que acaba de inventar sería el peor final posible de este flujo.
     */
    fun crear() {
        if (estado != EstadoDelAlta.Preguntando || paso != PasoDelAlta.Cuenta) return
        val donde = destino ?: return

        // La clave se revisa **antes** que `puedeAvanzar`, y no es redundante:
        // a `crear()` también se llega con la tecla de acción del teclado, que
        // no mira si el botón está apagado. Sin esto, apretar «Listo» con una
        // clave de cuatro letras no hace absolutamente nada y la pantalla se ve
        // colgada. Con esto contesta la tarjeta que dice cuántas letras faltan.
        revisarLaClave(clave)?.let { falla = it; return }

        // Lo que quede por corregir ya está dicho en el campo que lo causó y en
        // la línea de abajo; agregar una tarjeta encima sería decirlo dos veces.
        if (!puedeAvanzar) return

        estado = EstadoDelAlta.Creando
        falla = null

        viewModelScope.launch {
            val problema = diagnosticar(donde)
            if (problema != null) {
                falla = deLaConexion(problema, donde)
                estado = EstadoDelAlta.Preguntando
                return@launch
            }

            val creado = crearNegocio(
                api = sesion.apiPara(donde),
                nombreDelNegocio = nombre.trim(),
                rubro = (rubro ?: RUBROS.last()).clave,
                email = email.trim().lowercase(),
                clave = clave,
            )
            when (creado) {
                is Resultado.Falla -> {
                    falla = fallaDeAlta(creado.error, donde, email.trim().lowercase())
                    estado = EstadoDelAlta.Preguntando
                }

                is Resultado.Ok -> entrarAlNegocioReciencreado(donde, creado.valor)
            }
        }
    }

    private suspend fun entrarAlNegocioReciencreado(donde: String, creado: NegocioCreado) {
        val r = sesion.entrar(donde, creado.nombreCorto, email.trim().lowercase(), clave)
        when (r) {
            is Resultado.Ok -> {
                // La sesión quedó abierta: el árbol de arriba cambia solo de
                // pantalla. Lo único que queda por hacer acá es soltar la clave.
                clave = ""
                estado = EstadoDelAlta.Preguntando
            }

            is Resultado.Falla -> {
                falla = FallaDeAlta.CreadoPeroNoEntro(creado.nombreCorto, r.error.technical)
                estado = EstadoDelAlta.Preguntando
            }
        }
    }

    private suspend fun diagnosticar(donde: String): FallaDeConexion? = diagnosticarLaEntrada(
        direccion = donde,
        hayRed = red?.let { { it.hayRed() } },
        sondear = probador::sondear,
    )
}
