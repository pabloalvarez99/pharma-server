package cl.rutbusiness.app.ui.caja

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.common.esMasQueCero
import cl.rutbusiness.app.ui.common.montoParaElServidor
import cl.rutbusiness.app.ui.common.soloPlata
import cl.rutbusiness.app.ui.offline.ServiciosOffline
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.launch

/** Los momentos del día de una caja. */
enum class PasoDeCaja {
    /** Todavía no hay caja abierta: el ritual empieza acá. */
    Abrir,

    /** La caja está abierta y se puede vender. */
    Abierta,

    /** Sacando o metiendo plata, con motivo. */
    Movimiento,

    /** Contando la plata del cajón. */
    Arqueo,

    /** Ya se cerró y se está mirando la diferencia. */
    Cerrada,
}

/**
 * La caja del día.
 *
 * **Ningún monto se calcula acá.** Lo que debería haber en el cajón y la
 * diferencia del cierre los manda el server ya resueltos; lo único que este
 * `ViewModel` hace con un monto es leerlo con
 * [cl.rutbusiness.app.ui.common.montoParaElServidor] para chequear que lo que la
 * dueña escribió sea un decimal de verdad y no esté en cero. Validar no es
 * calcular: de esa lectura no sale ningún número que se muestre.
 *
 * Las escrituras no se reintentan solas. `POST /cash-sessions/{id}/movements`
 * no acepta clave de idempotencia, así que un reintento automático después de
 * un timeout podría dejar el retiro anotado dos veces — y dos retiros de $2.500
 * son $5.000 que faltan en el arqueo. Cuando una escritura falla se recarga la
 * lista de movimientos y se le dice a la dueña que mire antes de repetir.
 */
class CajaViewModel(
    private val sesion: SessionRepository,
    /** El estado de la conexión. `null` en las pruebas de pantalla. */
    private val offline: ServiciosOffline? = null,
) : ViewModel() {

    /**
     * `true` mientras se esté llegando al sistema del negocio.
     *
     * **La caja entera necesita server, y no hay forma honesta de sortearlo.**
     * Abrir, sacar plata, meter plata, contar y cerrar son todas escrituras sin
     * clave de idempotencia, así que no se pueden encolar como la venta: un
     * reintento automático anotaría el retiro dos veces, y dos retiros de
     * $2.500 son $5.000 que van a faltar en el arqueo. Y el "debería haber"
     * contra el que se cuenta lo calcula el server sumando las ventas del día —
     * sin él, contar la plata no se compara contra nada.
     *
     * Por eso la pantalla no se degrada: se planta y lo dice.
     */
    val hayConexion: Boolean get() = offline?.conexion?.hayConexion?.value ?: true

    var paso by mutableStateOf(PasoDeCaja.Abrir)
        private set

    /** Con qué moneda se escribe la plata. Sale del server. */
    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    var cargando by mutableStateOf(true)
        private set

    /** Falla que deja la pantalla sin nada que mostrar. */
    var error by mutableStateOf<RbErrorCopy?>(null)
        private set

    /** Falla de la acción en curso: abrir, mover plata, cerrar. */
    var errorDeAccion by mutableStateOf<RbErrorCopy?>(null)
        private set

    var guardando by mutableStateOf(false)
        private set

    /**
     * Pack feria (ADR-0022): la Screen llama [modoFeria]; el ViewModel no lee
     * CompositionLocals. Con feria el puesto arranca en $0 sin ritual de cajón.
     */
    var esFeria by mutableStateOf(false)
        private set

    /** Evita reintentar el auto-open de feria en cada recomposición. */
    private var puestoFeriaIntentado = false

    // --- abrir ---------------------------------------------------------------

    var cajas by mutableStateOf<List<CajaFisicaDto>>(emptyList())
        private set
    var cajaElegida by mutableStateOf<CajaFisicaDto?>(null)
        private set
    var montoDeApertura by mutableStateOf("")
        private set
    var notaDeApertura by mutableStateOf("")
        private set

    // --- caja abierta --------------------------------------------------------

    var arqueo by mutableStateOf<ArqueoDeCajaDto?>(null)
        private set
    var movimientos by mutableStateOf<List<MovimientoDeCajaDto>>(emptyList())
        private set

    /** La caja abierta, o `null` si el día todavía no empezó. */
    val sesionDeCaja: SesionDeCajaDto?
        get() = arqueo?.session?.takeIf { it.id.isNotBlank() }

    // --- movimiento ----------------------------------------------------------

    var tipoDeMovimiento by mutableStateOf(NuevoMovimiento.RETIRO)
        private set
    var montoDelMovimiento by mutableStateOf("")
        private set
    var motivoDelMovimiento by mutableStateOf("")
        private set

    // --- arqueo y cierre -----------------------------------------------------

    var montoContado by mutableStateOf("")
        private set
    var notaDeCierre by mutableStateOf("")
        private set

    /** El cierre tal como lo devolvió el server, con su `discrepancia`. */
    var cierre by mutableStateOf<ArqueoDeCajaDto?>(null)
        private set

    init {
        cargar()
    }

    // --- carga ---------------------------------------------------------------

    fun cargar() {
        val api = sesion.apiActiva()
        if (api == null) {
            cargando = false
            error = AppError.SesionExpirada().aCopy("tu caja")
            return
        }

        cargando = true
        error = null
        errorDeAccion = null

        viewModelScope.launch {
            moneda = MonedaRepository(api).resolver()
            val caja = CajaApi(api)

            when (val abierta = caja.sesionAbierta()) {
                is Resultado.Falla -> {
                    error = abierta.error.aCopy("tu caja")
                    cargando = false
                    return@launch
                }

                is Resultado.Ok -> {
                    val activa = abierta.valor
                    if (activa == null) {
                        arqueo = null
                        movimientos = emptyList()
                        // Las cajas físicas sólo hacen falta para abrir. Que no
                        // se puedan traer no impide abrir: se cae a la caja
                        // suelta con nombre por defecto.
                        cajas = (caja.cajas() as? Resultado.Ok)?.valor.orEmpty()
                        cajaElegida = cajas.firstOrNull()
                        paso = PasoDeCaja.Abrir
                    } else {
                        cargarEstadoDe(caja, activa.id)
                        paso = PasoDeCaja.Abierta
                    }
                }
            }

            cargando = false
        }
    }

    /**
     * El estado de una caja abierta: cuánto debería haber, y qué se movió.
     *
     * Si el arqueo falla, la sesión que ya tenemos se muestra igual — sin el
     * "debería haber", que es justamente el dato que no se puede inventar.
     */
    private suspend fun cargarEstadoDe(caja: CajaApi, sesionId: String) {
        when (val estado = caja.arqueo(sesionId)) {
            is Resultado.Ok -> arqueo = estado.valor
            is Resultado.Falla -> if (arqueo == null) {
                error = estado.error.aCopy("el estado de tu caja")
            }
        }
        movimientos = (caja.movimientos(sesionId) as? Resultado.Ok)?.valor.orEmpty()
    }

    // --- abrir ---------------------------------------------------------------

    /** La Screen lee `esFeria()` y lo baja acá: el VM no toca CompositionLocal. */
    fun modoFeria(v: Boolean) {
        esFeria = v
    }

    fun elegirCaja(caja: CajaFisicaDto?) {
        cajaElegida = caja
        errorDeAccion = null
    }

    fun cambiarMontoDeApertura(valor: String) {
        montoDeApertura = soloPlata(valor)
        errorDeAccion = null
    }

    fun cambiarNotaDeApertura(valor: String) {
        notaDeApertura = valor
    }

    /**
     * Monto efectivo de apertura: en feria el blank vale `"0"` (sin ritual).
     */
    private fun montoDeAperturaParaServidor(): String? {
        val escrito = if (esFeria && montoDeApertura.isBlank()) "0" else montoDeApertura
        return montoParaElServidor(escrito)
    }

    /** `null` = se puede abrir. Si no, el motivo ya escrito para mostrar. */
    fun impedimentoParaAbrir(): String? = when {
        // Feria: blank = $0, no se bloquea. Farmacia sigue pidiendo el monto.
        montoDeApertura.isBlank() && !esFeria ->
            "Escribe con cuánta plata parte el cajón."

        montoDeAperturaParaServidor() == null ->
            "Ese monto no se entiende. Escribe sólo números."

        else -> null
    }

    /**
     * Abre el puesto de feria con $0 sin pedir cajón.
     *
     * Si el server contesta 409 ("ya tiene una caja abierta") es éxito: otro
     * camino (Cobrar / agente) la abrió. Se recarga y se muestra Abierta.
     */
    fun abrirPuestoFeria() {
        if (sesionDeCaja != null || guardando || puestoFeriaIntentado) return
        esFeria = true
        puestoFeriaIntentado = true
        montoDeApertura = "0"
        if (notaDeApertura.isBlank()) notaDeApertura = NOTA_DIA_DE_FERIA
        abrirCaja(nombreSuelto = NOMBRE_PUESTO_FERIA, tratarConflictComoAbierta = true)
    }

    fun abrirCaja() {
        abrirCaja(
            nombreSuelto = if (esFeria) NOMBRE_PUESTO_FERIA else NOMBRE_DE_CAJA_SUELTA,
            tratarConflictComoAbierta = esFeria,
        )
    }

    private fun abrirCaja(nombreSuelto: String, tratarConflictComoAbierta: Boolean) {
        if (guardando || impedimentoParaAbrir() != null) return
        val api = sesion.apiActiva() ?: return
        val monto = montoDeAperturaParaServidor() ?: return

        guardando = true
        errorDeAccion = null

        viewModelScope.launch {
            val caja = CajaApi(api)
            val elegida = cajaElegida
            val apertura = AperturaDeCaja(
                apertura = monto,
                register = elegida?.id,
                // Sólo cuando no hay caja física: con `register`, el server toma
                // el nombre del record y así el arqueo dice lo mismo que la
                // configuración.
                nombreDeCaja = if (elegida == null) nombreSuelto else null,
                notes = notaDeApertura.trim().ifBlank { null },
            )

            when (val abierta = caja.abrir(apertura)) {
                is Resultado.Ok -> {
                    cargarEstadoDe(caja, abierta.valor.id)
                    montoDeApertura = ""
                    notaDeApertura = ""
                    paso = PasoDeCaja.Abierta
                }

                is Resultado.Falla -> {
                    if (tratarConflictComoAbierta && esCajaYaAbierta(abierta.error)) {
                        // Éxito: el puesto ya estaba abierto (p. ej. desde Cobrar).
                        errorDeAccion = null
                        cargar()
                    } else {
                        errorDeAccion = copyDeAperturaFallida(abierta.error)
                    }
                }
            }

            guardando = false
        }
    }

    // --- movimientos ---------------------------------------------------------

    fun irASacarPlata() = irAMovimiento(NuevoMovimiento.RETIRO)

    fun irAMeterPlata() = irAMovimiento(NuevoMovimiento.INGRESO)

    private fun irAMovimiento(tipo: String) {
        tipoDeMovimiento = tipo
        montoDelMovimiento = ""
        motivoDelMovimiento = ""
        errorDeAccion = null
        paso = PasoDeCaja.Movimiento
    }

    fun cambiarMontoDelMovimiento(valor: String) {
        montoDelMovimiento = soloPlata(valor)
        errorDeAccion = null
    }

    fun cambiarMotivoDelMovimiento(valor: String) {
        motivoDelMovimiento = valor
        errorDeAccion = null
    }

    fun impedimentoParaMover(): String? {
        return when {
            montoDelMovimiento.isBlank() -> "Escribe cuánta plata ${verboDelMovimiento()}."
            montoParaElServidor(montoDelMovimiento) == null ->
                "Ese monto no se entiende. Escribe sólo números."

            !esMasQueCero(montoDelMovimiento) -> "El monto tiene que ser más que cero."
            motivoDelMovimiento.isBlank() ->
                "Escribe por qué. Sin motivo, en el cierre no vas a saber qué pasó."

            else -> null
        }
    }

    fun guardarMovimiento() {
        if (guardando || impedimentoParaMover() != null) return
        val api = sesion.apiActiva() ?: return
        val sesionId = sesionDeCaja?.id ?: return
        val monto = montoParaElServidor(montoDelMovimiento) ?: return

        guardando = true
        errorDeAccion = null

        viewModelScope.launch {
            val caja = CajaApi(api)
            val movimiento = NuevoMovimiento(
                tipo = tipoDeMovimiento,
                amount = monto,
                reason = motivoDelMovimiento.trim(),
            )

            when (val guardado = caja.moverPlata(sesionId, movimiento)) {
                is Resultado.Ok -> {
                    cargarEstadoDe(caja, sesionId)
                    montoDelMovimiento = ""
                    motivoDelMovimiento = ""
                    paso = PasoDeCaja.Abierta
                }

                is Resultado.Falla -> {
                    // La lista se recarga aunque haya fallado: si el server
                    // alcanzó a grabarlo, la dueña lo ve ahí y no lo anota de
                    // nuevo. Este endpoint no tiene clave de idempotencia.
                    cargarEstadoDe(caja, sesionId)
                    errorDeAccion = copyDeMovimientoFallido(guardado.error)
                }
            }

            guardando = false
        }
    }

    // --- arqueo y cierre -----------------------------------------------------

    fun irAlArqueo() {
        montoContado = ""
        notaDeCierre = ""
        errorDeAccion = null
        paso = PasoDeCaja.Arqueo
    }

    fun cambiarMontoContado(valor: String) {
        montoContado = soloPlata(valor)
        errorDeAccion = null
    }

    fun cambiarNotaDeCierre(valor: String) {
        notaDeCierre = valor
    }

    fun impedimentoParaCerrar(): String? = when {
        montoContado.isBlank() -> "Escribe cuánto contaste."

        montoParaElServidor(montoContado) == null ->
            "Ese monto no se entiende. Escribe sólo números."

        else -> null
    }

    fun cerrarCaja() {
        if (guardando || impedimentoParaCerrar() != null) return
        val api = sesion.apiActiva() ?: return
        val sesionId = sesionDeCaja?.id ?: return
        val contado = montoParaElServidor(montoContado) ?: return

        guardando = true
        errorDeAccion = null

        viewModelScope.launch {
            val caja = CajaApi(api)
            val cerrado = caja.cerrar(
                sesionId,
                CierreDeCaja(contado = contado, notes = notaDeCierre.trim().ifBlank { null }),
            )

            when (cerrado) {
                is Resultado.Ok -> {
                    cierre = cerrado.valor
                    arqueo = cerrado.valor
                    paso = PasoDeCaja.Cerrada
                }

                is Resultado.Falla -> {
                    errorDeAccion = copyDeCierreFallido(cerrado.error)
                }
            }

            guardando = false
        }
    }

    /** Termina de mirar la diferencia y vuelve al principio del ritual. */
    fun terminarElDia() {
        cierre = null
        arqueo = null
        movimientos = emptyList()
        montoContado = ""
        notaDeCierre = ""
        cargar()
    }

    fun volverAlEstado() {
        errorDeAccion = null
        paso = PasoDeCaja.Abierta
    }

    // --- plata ---------------------------------------------------------------

    private fun verboDelMovimiento(): String =
        if (tipoDeMovimiento == NuevoMovimiento.RETIRO) "sacas" else "metes"

    // --- copy de fallas ------------------------------------------------------

    /**
     * El texto de una apertura que no salió.
     *
     * El 409 tiene nombre propio: significa que esta persona ya tiene una caja
     * abierta, casi siempre porque la abrió en otro aparato o porque anoche no
     * la cerró. "No se pudo completar" la dejaría buscando un error que no
     * existe.
     */
    private fun copyDeAperturaFallida(error: AppError): RbErrorCopy = when {
        error is AppError.ErrorDelServidor && error.status == 409 -> copyYaAbierta(esFeria)

        error is AppError.ServidorNoResponde -> RbErrorCopy(
            title = if (esFeria) "No pudimos abrir el puesto" else "No pudimos abrir la caja",
            message = copyErrorRed(feria = esFeria, accion = AccionErrorRed.Apertura),
            retryLabel = "Reintentar",
        )

        else -> error.aCopy(nombreDeCaja(esFeria))
    }

    /**
     * El texto de un movimiento que no salió.
     *
     * La pregunta que se hace la dueña cuando esto falla es "¿lo anoté o no?", y
     * la respuesta honesta es que no lo sabemos: el endpoint no tiene clave de
     * idempotencia. Se la manda a mirar la lista, que es la única forma de estar
     * segura, en vez de invitarla a repetir a ciegas.
     */
    private fun copyDeMovimientoFallido(error: AppError): RbErrorCopy = when (error) {
        is AppError.ServidorNoResponde -> RbErrorCopy(
            title = "No sabemos si quedó anotado",
            message = "Se cortó antes de que el sistema confirmara. Mira la lista de " +
                "movimientos: si ya está ahí, no lo anotes de nuevo.",
            retryLabel = null,
        )

        else -> error.aCopy("el movimiento")
    }

    private fun copyDeCierreFallido(error: AppError): RbErrorCopy = when {
        error is AppError.ErrorDelServidor && error.status == 409 -> copyYaCerrada(esFeria)

        error is AppError.ServidorNoResponde -> RbErrorCopy(
            title = if (esFeria) "No pudimos cerrar el día" else "No pudimos cerrar la caja",
            message = copyErrorRed(feria = esFeria, accion = AccionErrorRed.Cierre),
            retryLabel = "Reintentar",
        )

        else -> error.aCopy(if (esFeria) "el cierre del día" else "el cierre de caja")
    }

    private companion object {
        /**
         * El mismo `default_register_name` de
         * `crates/domain/src/cash_register/model.rs`. Se manda explícito para
         * que el nombre que ve la dueña salga de un solo lado.
         */
        const val NOMBRE_DE_CAJA_SUELTA = "caja-1"
    }
}

