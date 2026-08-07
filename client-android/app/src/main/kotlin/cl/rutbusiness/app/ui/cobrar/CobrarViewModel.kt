package cl.rutbusiness.app.ui.cobrar

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.catalog.ProductRepository
import cl.rutbusiness.core.customers.ClienteDto
import cl.rutbusiness.core.customers.CustomerRepository
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.pos.Carrito
import cl.rutbusiness.core.pos.ComprobanteDto
import cl.rutbusiness.core.pos.MedioDePago
import cl.rutbusiness.core.pos.PosRepository
import cl.rutbusiness.core.pos.SolicitudDeVenta
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Los tres momentos de una venta. */
enum class PasoDeCobro { Buscar, Pago, Comprobante }

class CobrarViewModel(
    private val sesion: SessionRepository,
) : ViewModel() {

    var paso by mutableStateOf(PasoDeCobro.Buscar)
        private set

    // --- moneda -------------------------------------------------------------

    /**
     * Con qué moneda se escribe la plata. Sale del server; hasta que conteste,
     * el default. Nunca hay un "CLP" clavado en una pantalla.
     */
    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    // --- búsqueda -----------------------------------------------------------

    var consulta by mutableStateOf("")
        private set
    var buscando by mutableStateOf(false)
        private set
    var resultados by mutableStateOf<List<ProductDto>>(emptyList())
        private set
    var errorBusqueda by mutableStateOf<RbErrorCopy?>(null)
        private set
    var buscoAlgunaVez by mutableStateOf(false)
        private set

    private var trabajoDeBusqueda: Job? = null

    // --- carrito ------------------------------------------------------------

    var carrito by mutableStateOf(Carrito())
        private set

    // --- pago ---------------------------------------------------------------

    var medio by mutableStateOf(MedioDePago.Efectivo)
        private set
    var montoEntregado by mutableStateOf("")
        private set
    var cliente by mutableStateOf<ClienteDto?>(null)
        private set
    var clientes by mutableStateOf<List<ClienteDto>>(emptyList())
        private set
    var errorPago by mutableStateOf<RbErrorCopy?>(null)
        private set
    var cobrando by mutableStateOf(false)
        private set

    // --- comprobante --------------------------------------------------------

    var comprobante by mutableStateOf<ComprobanteDto?>(null)
        private set
    var puntosGanados by mutableStateOf(0L)
        private set

    /**
     * La clave de idempotencia de **este** intento de cobro.
     *
     * Se guarda entre reintentos: si el POST falló por red y el usuario toca
     * "Reintentar", sale la misma clave y el server -que quizá sí alcanzó a
     * grabar la venta- contesta 200 con la orden que ya existe en vez de cobrar
     * de nuevo.
     *
     * Se anula en cuanto cambia **cualquier** cosa del pedido: otro producto,
     * otra cantidad, otro medio de pago, otro monto, otro cliente. Reusar la
     * clave con un pedido distinto haría que el server devolviera la venta
     * vieja y el cajero cobrara lo que no era.
     */
    private var claveDeCobro: String? = null

    init {
        viewModelScope.launch {
            sesion.apiActiva()?.let { moneda = MonedaRepository(it).resolver() }
        }
        buscar()
    }

    // --- búsqueda -----------------------------------------------------------

    fun cambiarConsulta(valor: String) {
        consulta = valor
        errorBusqueda = null
        // Debounce: la cajera tipea rápido y cada tecla no puede ser un viaje
        // por datos móviles. 250 ms es corto para que se sienta vivo y largo
        // para no disparar cinco búsquedas por palabra.
        trabajoDeBusqueda?.cancel()
        trabajoDeBusqueda = viewModelScope.launch {
            delay(250)
            buscar()
        }
    }

    /** Enter en el campo: busca ya, sin esperar el debounce. */
    fun buscarAhora() {
        trabajoDeBusqueda?.cancel()
        buscar()
    }

    private fun buscar() {
        val api = sesion.apiActiva() ?: return
        val texto = consulta.trim()
        buscando = true
        errorBusqueda = null
        trabajoDeBusqueda = viewModelScope.launch {
            // Un código de barras se resuelve, no se busca: si el texto parece
            // uno (8 dígitos o más), primero se prueba el lookup directo. Es lo
            // que va a hacer el escáner cuando entre, y hoy funciona igual
            // tipeándolo.
            if (pareceCodigoDeBarras(texto)) {
                when (val r = ProductRepository(api).porCodigoDeBarras(texto)) {
                    is Resultado.Ok -> {
                        resultados = listOf(r.valor)
                        buscando = false
                        buscoAlgunaVez = true
                        return@launch
                    }
                    // 404 = ese código no existe. Se cae a la búsqueda por
                    // texto en vez de cortar: puede ser un SKU interno.
                    is Resultado.Falla -> Unit
                }
            }

            when (val r = ProductRepository(api).buscar(texto)) {
                is Resultado.Ok -> resultados = r.valor
                is Resultado.Falla -> {
                    errorBusqueda = r.error.aCopy("tus productos")
                    if (r.error is AppError.SesionExpirada) sesion.salir()
                }
            }
            buscando = false
            buscoAlgunaVez = true
        }
    }

    // --- carrito ------------------------------------------------------------

    /**
     * Agrega un producto. Síncrono a propósito: no hay red en el camino, así
     * que el carrito cambia en el mismo frame del toque.
     */
    fun agregar(producto: ProductDto) {
        carrito = carrito.agregar(producto)
        invalidarClave()
    }

    fun cambiarCantidad(productoId: String, cantidad: Int) {
        carrito = carrito.cambiarCantidad(productoId, cantidad)
        invalidarClave()
        if (carrito.vacio) paso = PasoDeCobro.Buscar
    }

    fun quitar(productoId: String) = cambiarCantidad(productoId, 0)

    fun cantidadEnCarrito(productoId: String): Int =
        carrito.items.firstOrNull { it.productoId == productoId }?.cantidad ?: 0

    // --- pago ---------------------------------------------------------------

    fun irAPagar() {
        if (carrito.vacio) return
        errorPago = null
        paso = PasoDeCobro.Pago
        cargarClientes()
    }

    fun volverABuscar() {
        paso = PasoDeCobro.Buscar
    }

    fun cambiarMedio(nuevo: MedioDePago) {
        medio = nuevo
        errorPago = null
        if (!nuevo.exigeCliente) cliente = null
        if (!nuevo.pideMontoEntregado) montoEntregado = ""
        invalidarClave()
    }

    fun cambiarMontoEntregado(valor: String) {
        // Sólo dígitos y un separador: el teclado numérico de Android igual
        // deja meter símbolos y el server rechazaría el decimal.
        montoEntregado = valor.filter { it.isDigit() || it == '.' || it == ',' }
        errorPago = null
        invalidarClave()
    }

    fun elegirCliente(nuevo: ClienteDto?) {
        cliente = nuevo
        errorPago = null
        invalidarClave()
    }

    private fun cargarClientes() {
        if (clientes.isNotEmpty()) return
        val api = sesion.apiActiva() ?: return
        viewModelScope.launch {
            when (val r = CustomerRepository(api).listar()) {
                is Resultado.Ok -> clientes = r.valor
                // Que no se puedan cargar los clientes no rompe la pantalla:
                // efectivo y transferencia siguen andando. El fiado avisa solo
                // cuando el usuario lo elige y no hay a quién fiarle.
                is Resultado.Falla -> Unit
            }
        }
    }

    /** `null` = se puede cobrar. Si no, el motivo, ya escrito para mostrar. */
    fun impedimentoParaCobrar(): String? = when {
        carrito.vacio -> "Agrega al menos un producto."
        medio.exigeCliente && cliente == null ->
            "El fiado queda en la cuenta de alguien: elige el cliente."
        medio.pideMontoEntregado && montoEntregado.isBlank() ->
            "Escribe con cuánto te paga para calcular el vuelto."
        else -> null
    }

    fun cobrar() {
        if (cobrando || impedimentoParaCobrar() != null) return
        val api = sesion.apiActiva() ?: return

        val clave = claveDeCobro ?: PosRepository.nuevaClave().also { claveDeCobro = it }
        cobrando = true
        errorPago = null

        viewModelScope.launch {
            val repo = PosRepository(api)
            val solicitud = SolicitudDeVenta(
                items = carrito.aLineasDeVenta(),
                paymentMethod = medio.codigo,
                cashAmount = montoEntregado.replace(',', '.').takeIf { it.isNotBlank() },
                customer = cliente?.id,
            )

            when (val venta = repo.vender(solicitud, clave)) {
                is Resultado.Falla -> {
                    errorPago = copyDeCobroFallido(venta.error)
                    cobrando = false
                }

                is Resultado.Ok -> {
                    puntosGanados = venta.valor.loyaltyPointsAwarded
                    // El comprobante trae el vuelto calculado por el server.
                    // Si no llega, igual se muestra la venta con lo que sí
                    // sabemos: la plata ya se cobró, no se puede dejar al
                    // cajero mirando un error.
                    comprobante = when (val c = repo.comprobante(venta.valor.order.id)) {
                        is Resultado.Ok -> c.valor
                        is Resultado.Falla -> null
                    }
                    totalCobrado = venta.valor.order.total
                    paso = PasoDeCobro.Comprobante
                    cobrando = false
                }
            }
        }
    }

    /** El total que cobró el server. Es el número que manda, no el del carrito. */
    var totalCobrado by mutableStateOf<String?>(null)
        private set

    /** Cierra el comprobante y deja todo listo para la venta siguiente. */
    fun nuevaVenta() {
        carrito = Carrito()
        comprobante = null
        totalCobrado = null
        puntosGanados = 0
        montoEntregado = ""
        cliente = null
        medio = MedioDePago.Efectivo
        errorPago = null
        claveDeCobro = null
        consulta = ""
        paso = PasoDeCobro.Buscar
        buscar()
    }

    fun salir() {
        viewModelScope.launch { sesion.salir() }
    }

    private fun invalidarClave() {
        claveDeCobro = null
    }

    /**
     * El texto de un cobro que no salió.
     *
     * No reusa la copy genérica de "no pudimos cargar X": cuando lo que falla es
     * un cobro, la cajera tiene al cliente esperando con la plata en la mano y
     * la pregunta que se hace es una sola, **"¿y si toco de nuevo, le cobro dos
     * veces?"**. La respuesta va en el mensaje, porque es verdad: la clave de
     * idempotencia hace que reintentar sea seguro.
     *
     * Cuando el server sí contestó (stock insuficiente, rol que no alcanza), su
     * mensaje gana: sabe cosas que el teléfono no.
     */
    private fun copyDeCobroFallido(error: AppError): RbErrorCopy = when (error) {
        is AppError.ServidorNoResponde -> RbErrorCopy(
            title = "No llegó la venta",
            message = "No alcanzamos a mandarle la venta al sistema del negocio. Revisa la señal " +
                "y toca Reintentar: aunque la mandes de nuevo, no se cobra dos veces.",
            retryLabel = "Reintentar",
        )

        else -> error.aCopy("la venta")
    }

    private fun pareceCodigoDeBarras(texto: String): Boolean =
        texto.length >= 8 && texto.all { it.isDigit() }
}
