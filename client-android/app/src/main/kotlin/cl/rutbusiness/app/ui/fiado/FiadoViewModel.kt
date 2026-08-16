package cl.rutbusiness.app.ui.fiado

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.caja.CajaApi
import cl.rutbusiness.app.ui.caja.ResultadoPuesto
import cl.rutbusiness.app.ui.caja.SesionDeCajaDto
import cl.rutbusiness.app.ui.caja.asegurarPuestoAbierto
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.common.esMasQueCero
import cl.rutbusiness.app.ui.common.montoParaElServidor
import cl.rutbusiness.app.ui.common.soloPlata
import cl.rutbusiness.app.ui.offline.Lectura
import cl.rutbusiness.app.ui.offline.ServiciosOffline
import cl.rutbusiness.app.ui.offline.conCache
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.offline.ClaveDeCache
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.launch

/** Los momentos de cobrar el fiado. */
enum class PasoDeFiado {
    /** Quién me debe, ordenado por cuánto. */
    Lista,

    /** La cuenta corriente de un cliente. */
    Detalle,

    /** Anotando lo que está pagando ahora. */
    Abono,
}

/**
 * Quién le debe al negocio, y cobrarles.
 *
 * **Ningún saldo se calcula acá.** El total por cobrar, el saldo de cada cliente
 * y el saldo que queda después de un abono los manda el server; el teléfono los
 * escribe con la moneda del tenant y nada más. Lo único que se hace con un monto
 * es leerlo con [cl.rutbusiness.app.ui.common.montoParaElServidor] para validar
 * lo que la dueña escribió — validar no produce un número que se muestre.
 *
 * La búsqueda sí es local, y a propósito: los deudores de un almacén de barrio
 * son decenas, la lista ya está en memoria, y el cliente que llega a pagar está
 * parado en el mostrador esperando. Filtrar sin salir a la red es instantáneo y
 * funciona con la señal caída.
 */
class FiadoViewModel(
    private val sesion: SessionRepository,
    /** El caché y el estado de la conexión. `null` en las pruebas de pantalla. */
    private val offline: ServiciosOffline? = null,
) : ViewModel() {

    /** `true` mientras se esté llegando al sistema del negocio. */
    val hayConexion: Boolean get() = offline?.conexion?.hayConexion?.value ?: true

    /** Cuándo se trajo la lista que se está mostrando. `null` = es de ahora. */
    var guardadoEn by mutableStateOf<Long?>(null)
        private set

    /** El reloj de pared, para escribir "hace 20 minutos". */
    val relojDePared: Long get() = offline?.reloj?.invoke() ?: 0L

    /**
     * Por qué no se puede anotar un abono ahora, o `null` si sí se puede.
     *
     * Se muestra **antes** de que la dueña llene el formulario. Un abono es una
     * escritura sin clave de idempotencia (`POST /clientes/{id}/abonos` no la
     * acepta), así que no se puede encolar como la venta: un reintento
     * automático anotaría el pago dos veces y le perdonaría al cliente una
     * deuda que sigue viva. Sin server, no hay abono.
     */
    val motivoParaNoAbonar: String?
        get() = if (hayConexion) {
            null
        } else {
            "Sin conexión no se pueden anotar pagos: hay que dejarlos escritos en la cuenta " +
                "del cliente, y esa cuenta vive en el sistema del negocio."
        }

    var paso by mutableStateOf(PasoDeFiado.Lista)
        private set

    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    var cargando by mutableStateOf(true)
        private set

    var error by mutableStateOf<RbErrorCopy?>(null)
        private set

    var errorDeAccion by mutableStateOf<RbErrorCopy?>(null)
        private set

    var guardando by mutableStateOf(false)
        private set

    // --- lista ---------------------------------------------------------------

    var deudores by mutableStateOf<DeudoresDto?>(null)
        private set

    var consulta by mutableStateOf("")
        private set

    /**
     * Los deudores que hay que dibujar, en el orden que mandó el server.
     *
     * Se calcula al leerse y no se guarda en su propio estado: `deudores` y
     * `consulta` ya son estado de Compose, así que la lista se rearma sola en la
     * recomposición y no puede quedar desincronizada de lo tipeado.
     */
    val deudoresVisibles: List<DeudorDto>
        get() {
            val filas = deudores?.rows.orEmpty()
            val texto = normalizar(consulta)
            if (texto.isEmpty()) return filas
            return filas.filter { fila ->
                normalizar(fila.name).contains(texto) ||
                    normalizar(fila.phone.orEmpty()).contains(texto)
            }
        }

    // --- detalle -------------------------------------------------------------

    var elegido by mutableStateOf<DeudorDto?>(null)
        private set
    var cuenta by mutableStateOf<CuentaDto?>(null)
        private set
    var cargandoCuenta by mutableStateOf(false)
        private set

    /** Lo que se le dice a la dueña después de anotar un pago. */
    var avisoDeAbono by mutableStateOf<String?>(null)
        private set

    // --- abono ---------------------------------------------------------------

    var montoDelAbono by mutableStateOf("")
        private set
    var notaDelAbono by mutableStateOf("")
        private set

    /** La caja abierta, si hay. Sin ella el abono en efectivo no entra al arqueo. */
    var cajaAbierta by mutableStateOf<SesionDeCajaDto?>(null)
        private set

    /**
     * Si el billete entra a la caja / cuenta en el día (feria).
     *
     * Arranca en `true` porque el fiado del barrio se paga en efectivo y en el
     * mostrador. Apagarlo es para el que transfiere: esa plata no está en la
     * caja y meterla al arqueo haría faltar plata que nunca existió.
     */
    var entraALaCaja by mutableStateOf(true)
        private set

    /**
     * Pack feria: la Screen llama [modoFeria]; el ViewModel no lee
     * CompositionLocals. Con feria el abono en efectivo abre el puesto con $0
     * (o trata el 409 como abierto) sin ritual de cajón/computador.
     */
    var esFeria by mutableStateOf(false)
        private set

    init {
        cargar()
    }

    /**
     * Baja el flag de feria desde la Screen (`esFeria()` no se puede llamar acá).
     *
     * Si es feria y hay red, abre el puesto con $0 reusando [asegurarPuestoAbierto]
     * (mismo POST que Caja/Cobrar; 409 = ya abierto = éxito) y deja
     * [entraALaCaja] en true.
     */
    fun modoFeria(v: Boolean = true) {
        esFeria = v
        if (v) asegurarPuestoSiFalta()
    }

    /**
     * En feria deja el puesto listo: sesión abierta + efectivo cuenta en el día.
     * Idempotente; 409 / "ya tiene caja" = éxito vía [asegurarPuestoAbierto].
     */
    private fun asegurarPuestoSiFalta() {
        if (!esFeria || !hayConexion) return
        val api = sesion.apiActiva() ?: return
        viewModelScope.launch {
            when (val r = asegurarPuestoAbierto(api)) {
                is ResultadoPuesto.Abierto -> {
                    cajaAbierta = r.sesion
                    entraALaCaja = true
                }
                is ResultadoPuesto.Falla -> Unit
            }
        }
    }

    fun cargar() {
        val api = sesion.apiActiva()
        if (api == null) {
            cargando = false
            error = AppError.SesionExpirada().aCopy("el fiado")
            return
        }

        cargando = true
        error = null

        viewModelScope.launch {
            moneda = MonedaRepository(api).resolver()

            // Quién te debe se puede mirar sin señal: es lectura, y es de las
            // cosas que la dueña consulta parada en el mostrador cuando llega
            // alguien a pagar. Lo que no se va a poder es anotar el pago, y eso
            // lo dice `motivoParaNoAbonar` antes de que lo intente.
            when (
                val lectura = conCache(
                    offline = offline,
                    que = ClaveDeCache.Que.Deudores,
                    servidor = api.baseUrl,
                    serializador = DeudoresDto.serializer(),
                    traer = { FiadoApi(api).deudores() },
                )
            ) {
                is Lectura.DelServidor -> {
                    deudores = lectura.valor
                    guardadoEn = null
                }

                is Lectura.DelTelefono -> {
                    deudores = lectura.valor
                    guardadoEn = lectura.guardadoEn
                }

                is Lectura.Falla -> {
                    error = lectura.error.aCopy("quién te debe")
                }
            }

            if (esFeria) {
                // Feria: el puesto se abre solo ($0). 409 = ya abierto = OK.
                when (val r = asegurarPuestoAbierto(api)) {
                    is ResultadoPuesto.Abierto -> {
                        cajaAbierta = r.sesion
                        entraALaCaja = true
                    }
                    is ResultadoPuesto.Falla -> {
                        cajaAbierta = (CajaApi(api).sesionAbierta() as? Resultado.Ok)?.valor
                        if (cajaAbierta == null) entraALaCaja = false
                    }
                }
            } else {
                // Retail: sin caja abierta no es error; el abono en efectivo no
                // entra al arqueo hasta que abran el cajón.
                cajaAbierta = (CajaApi(api).sesionAbierta() as? Resultado.Ok)?.valor
                if (cajaAbierta == null) entraALaCaja = false
            }

            cargando = false
        }
    }

    fun cambiarConsulta(valor: String) {
        consulta = valor
    }

    // --- detalle -------------------------------------------------------------

    fun abrirCuenta(deudor: DeudorDto) {
        elegido = deudor
        cuenta = null
        avisoDeAbono = null
        errorDeAccion = null
        paso = PasoDeFiado.Detalle
        recargarCuenta()
    }

    private fun recargarCuenta() {
        val api = sesion.apiActiva() ?: return
        val clienteId = elegido?.customer ?: return

        cargandoCuenta = true
        viewModelScope.launch {
            when (val r = FiadoApi(api).cuenta(clienteId)) {
                is Resultado.Ok -> cuenta = r.valor
                is Resultado.Falla -> {
                    errorDeAccion = r.error.aCopy("la cuenta de ${elegido?.name.orEmpty()}")
                }
            }
            cargandoCuenta = false
        }
    }

    fun volverALaLista() {
        paso = PasoDeFiado.Lista
        elegido = null
        cuenta = null
        avisoDeAbono = null
        errorDeAccion = null
    }

    // --- abono ---------------------------------------------------------------

    fun irAlAbono() {
        montoDelAbono = ""
        notaDelAbono = ""
        avisoDeAbono = null
        errorDeAccion = null
        if (esFeria) {
            // Efectivo cuenta en el día; si el puesto aún no está, se abre.
            entraALaCaja = true
            asegurarPuestoSiFalta()
        } else {
            entraALaCaja = cajaAbierta != null
        }
        paso = PasoDeFiado.Abono
    }

    fun volverAlDetalle() {
        errorDeAccion = null
        paso = PasoDeFiado.Detalle
    }

    fun cambiarMontoDelAbono(valor: String) {
        montoDelAbono = soloPlata(valor)
        errorDeAccion = null
    }

    fun cambiarNotaDelAbono(valor: String) {
        notaDelAbono = valor
    }

    fun cambiarEntraALaCaja(entra: Boolean) {
        if (entra && esFeria) {
            entraALaCaja = true
            if (cajaAbierta == null) asegurarPuestoSiFalta()
            return
        }
        entraALaCaja = entra && cajaAbierta != null
    }

    fun impedimentoParaAbonar(): String? = when {
        montoDelAbono.isBlank() -> "Escribe cuánto te está pagando."
        montoParaElServidor(montoDelAbono) == null ->
            "Ese monto no se entiende. Escribe sólo números."

        !esMasQueCero(montoDelAbono) -> "El monto tiene que ser más que cero."
        else -> null
    }

    /**
     * Anota el pago.
     *
     * Sin reintento automático: `POST /customers/{id}/abono` no acepta clave de
     * idempotencia, y un abono anotado dos veces le baja la deuda a un cliente
     * que pagó una sola vez. Cuando falla se recarga la cuenta para que la dueña
     * vea si quedó o no antes de volver a tocarlo.
     */
    fun registrarAbono() {
        if (guardando || impedimentoParaAbonar() != null) return
        val api = sesion.apiActiva() ?: return
        val clienteId = elegido?.customer ?: return
        val monto = montoParaElServidor(montoDelAbono) ?: return

        guardando = true
        errorDeAccion = null

        viewModelScope.launch {
            // Feria + efectivo: el billete tiene que caer en el puesto. Si aún
            // no hay sesión, se abre con $0 (409 = ya abierta = éxito).
            if (esFeria && entraALaCaja && cajaAbierta == null) {
                when (val r = asegurarPuestoAbierto(api)) {
                    is ResultadoPuesto.Abierto -> {
                        cajaAbierta = r.sesion
                        entraALaCaja = true
                    }
                    is ResultadoPuesto.Falla -> Unit
                }
            }

            val fiado = FiadoApi(api)
            val abono = NuevoAbono(
                amount = monto,
                cajaAbierta = if (entraALaCaja) cajaAbierta?.id else null,
                note = notaDelAbono.trim().ifBlank { null },
            )

            when (val r = fiado.registrarAbono(clienteId, abono)) {
                is Resultado.Ok -> {
                    val abonado = moneda.formatear(r.valor.amount)
                    // El saldo que queda sale de volver a preguntar, no de
                    // restar: acá no se calcula plata.
                    val actualizada = (fiado.cuenta(clienteId) as? Resultado.Ok)?.valor
                    cuenta = actualizada ?: cuenta
                    avisoDeAbono = buildString {
                        append("Listo, quedó anotado que te pagó $abonado.")
                        actualizada?.let { append(" Ahora debe ${moneda.formatear(it.balance)}.") }
                        if (entraALaCaja && cajaAbierta != null) {
                            append(remateAbonoEfectivo(feria = esFeria))
                        }
                    }
                    montoDelAbono = ""
                    notaDelAbono = ""
                    paso = PasoDeFiado.Detalle
                    // La lista de deudores cambió: el total de arriba y el saldo
                    // de esta persona ya no son los que se trajeron al entrar.
                    refrescarDeudores(fiado)
                }

                is Resultado.Falla -> {
                    errorDeAccion = copyDeAbonoFallido(r.error)
                    recargarCuenta()
                }
            }

            guardando = false
        }
    }

    private suspend fun refrescarDeudores(fiado: FiadoApi) {
        (fiado.deudores() as? Resultado.Ok)?.let { deudores = it.valor }
    }

    // --- texto ---------------------------------------------------------------

    /**
     * Cómo se comparan dos textos al buscar.
     *
     * Sin mayúsculas y sin tildes: quien busca a "Pérez" escribiendo "perez" con
     * el cliente esperando no tiene por qué acordarse del acento, y el teclado
     * de un teléfono viejo tampoco se lo pone fácil.
     */
    private fun normalizar(texto: String): String = texto
        .trim()
        .lowercase()
        .map { caracter ->
            val indice = CON_TILDE.indexOf(caracter)
            if (indice >= 0) SIN_TILDE[indice] else caracter
        }
        .joinToString("")

    private fun copyDeAbonoFallido(error: AppError): RbErrorCopy = when (error) {
        is AppError.ServidorNoResponde -> RbErrorCopy(
            title = "No sabemos si quedó anotado",
            message = "Se cortó antes de que el sistema confirmara. Mira los movimientos de " +
                "abajo: si el pago ya está ahí, no lo anotes de nuevo.",
            retryLabel = null,
        )

        else -> error.aCopy("el pago")
    }

    private companion object {
        const val CON_TILDE = "áéíóúüñ"
        const val SIN_TILDE = "aeiouun"
    }
}
