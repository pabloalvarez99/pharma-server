package cl.rutbusiness.app.ui.cobrar

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.diag.Latencia
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.scanner.AntiRebote
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

    // --- escáner ------------------------------------------------------------

    /**
     * Lo último que entró por la cámara.
     *
     * [orden] existe para que dos lecturas seguidas del **mismo** producto sean
     * objetos distintos: la pantalla dispara la vibración cuando este valor
     * cambia, y sin el contador pasar dos veces el mismo tarro no vibraría la
     * segunda vez -justo cuando la cajera más necesita saber que entró-.
     */
    data class LecturaOk(val nombre: String, val enCarrito: Int, val orden: Int)

    var escaneando by mutableStateOf(false)
        private set
    var ultimaLectura by mutableStateOf<LecturaOk?>(null)
        private set

    /** Un código que se leyó bien pero no está en el catálogo. */
    var codigoSinProducto by mutableStateOf<String?>(null)
        private set
    var errorDeEscaneo by mutableStateOf<RbErrorCopy?>(null)
        private set

    private var ordenDeLectura = 0
    private val antiRebote = AntiRebote()

    fun abrirEscaner() {
        antiRebote.olvidar()
        ultimaLectura = null
        codigoSinProducto = null
        errorDeEscaneo = null
        creandoProducto = false
        escaneando = true
    }

    fun cerrarEscaner() {
        escaneando = false
        creandoProducto = false
    }

    /**
     * Salida sin cámara.
     *
     * Si venimos de un código que no estaba en el catálogo, queda escrito en el
     * campo de búsqueda: casi siempre lo que falla es un dígito, y reescribir
     * trece a mano cuando doce estaban bien es trabajo regalado.
     */
    fun escribirAMano() {
        val codigo = codigoSinProducto
        escaneando = false
        creandoProducto = false
        if (codigo != null) {
            consulta = codigo
            codigoSinProducto = null
            buscarAhora()
        }
    }

    /**
     * Un código que vio la cámara.
     *
     * @param ahoraMs reloj monótono; [System.nanoTime] y no la hora de pared
     *   porque un ajuste de hora en medio de una venta no puede duplicar una
     *   línea del carrito.
     */
    fun escanear(codigo: String, ahoraMs: Long = System.nanoTime() / 1_000_000L) {
        val limpio = codigo.trim()
        // Con el formulario de producto nuevo abierto la cámara ya no está
        // encendida; esto sólo cubre un frame que venía en vuelo.
        if (limpio.isEmpty() || creandoProducto) return
        if (!antiRebote.aceptar(limpio, ahoraMs)) return

        // Del código decodificado al frame que ya muestra el producto adentro.
        // Es lo único de este camino que depende de nosotros: enfocar y decodificar
        // los pone la cámara del aparato. Sale por `adb logcat -s RbLatencia`.
        Latencia.marcar()
        val api = sesion.apiActiva() ?: return
        viewModelScope.launch {
            when (val r = ProductRepository(api).porCodigoDeBarras(limpio)) {
                is Resultado.Ok -> {
                    agregar(r.valor)
                    codigoSinProducto = null
                    errorDeEscaneo = null
                    ultimaLectura = LecturaOk(
                        nombre = r.valor.name,
                        enCarrito = cantidadEnCarrito(r.valor.id),
                        orden = ++ordenDeLectura,
                    )
                }

                is Resultado.Falla -> noResolvio(limpio, r.error)
            }
        }
    }

    private suspend fun noResolvio(codigo: String, error: AppError) {
        ultimaLectura = null
        when {
            error is AppError.SesionExpirada -> {
                escaneando = false
                sesion.salir()
            }

            // 404 no es una falla del escáner: leyó perfecto, el catálogo no lo
            // tiene. Es el único caso donde ofrecemos crear el producto.
            error is AppError.ErrorDelServidor && error.status == 404 -> {
                errorDeEscaneo = null
                codigoSinProducto = codigo
                nombreNuevo = ""
                precioNuevo = ""
            }

            else -> {
                codigoSinProducto = null
                errorDeEscaneo = error.aCopy("ese producto")
            }
        }
    }

    // --- producto nuevo desde el escáner -------------------------------------

    var creandoProducto by mutableStateOf(false)
        private set
    var nombreNuevo by mutableStateOf("")
        private set
    var precioNuevo by mutableStateOf("")
        private set
    var guardandoProducto by mutableStateOf(false)
        private set
    var errorAlCrear by mutableStateOf<String?>(null)
        private set

    fun crearProductoDelCodigo() {
        if (codigoSinProducto == null) return
        errorAlCrear = null
        creandoProducto = true
    }

    fun cancelarCreacion() {
        creandoProducto = false
        errorAlCrear = null
    }

    fun cambiarNombreNuevo(valor: String) {
        nombreNuevo = valor
        errorAlCrear = null
    }

    fun cambiarPrecioNuevo(valor: String) {
        precioNuevo = valor.filter { it.isDigit() }
        errorAlCrear = null
    }

    /**
     * Crea el producto del código que no estaba, y lo deja en el carrito.
     *
     * Son **dos** llamadas porque el server las pide así: `POST /products` no
     * acepta `barcode` (`NewProduct` no lo tiene), y el EAN se asigna con un
     * `PATCH` aparte. Si el PATCH falla, el producto igual quedó creado y se
     * cobra lo mismo; lo único que se pierde es que el próximo escaneo lo
     * encuentre, y eso se dice en pantalla en vez de esconderlo.
     *
     * Nace con **una** unidad de stock a propósito: es la que la cajera tiene en
     * la mano. Con cero, el server rechaza la venta por stock insuficiente y
     * crear el producto no habría servido de nada.
     */
    fun guardarProductoNuevo() {
        if (guardandoProducto) return
        val codigo = codigoSinProducto ?: return
        val nombre = nombreNuevo.trim()
        val precio = precioNuevo.trim()
        errorAlCrear = when {
            nombre.isEmpty() -> "Ponle un nombre: es lo que va a salir en la boleta."
            precio.isEmpty() -> "Falta el precio."
            else -> null
        }
        if (errorAlCrear != null) return

        val api = sesion.apiActiva() ?: return
        guardandoProducto = true

        viewModelScope.launch {
            when (val creado = crearProducto(api, nombre = nombre, precio = precio)) {
                is Resultado.Falla -> {
                    errorAlCrear = creado.error.userMessage
                    guardandoProducto = false
                }

                is Resultado.Ok -> {
                    val conCodigo = pegarCodigo(api, creado.valor.id, codigo)
                    val producto = (conCodigo as? Resultado.Ok)?.valor ?: creado.valor

                    // El producto que se acaba de crear sigue delante del lente.
                    // Se anota como visto para que la cámara no lo cuente otra
                    // vez apenas vuelva el visor: ya está adentro.
                    antiRebote.anotar(codigo, System.nanoTime() / 1_000_000L)
                    agregar(producto)
                    guardandoProducto = false
                    creandoProducto = false
                    codigoSinProducto = null
                    errorAlCrear = null
                    errorDeEscaneo = if (conCodigo is Resultado.Falla) {
                        RbErrorCopy(
                            title = "Quedó sin código",
                            message = "Creamos «$nombre» y ya está en el carrito, pero no " +
                                "pudimos dejarle pegado el código $codigo. La próxima vez vas a " +
                                "tener que buscarlo por nombre.",
                            retryLabel = null,
                        )
                    } else {
                        null
                    }
                    ultimaLectura = LecturaOk(
                        nombre = producto.name,
                        enCarrito = cantidadEnCarrito(producto.id),
                        orden = ++ordenDeLectura,
                    )
                }
            }
        }
    }

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
        // El escáner también arranca de cero: el cartel verde de la venta
        // anterior arriba de la venta nueva diría que ese producto está en este
        // carrito, y no lo está.
        escaneando = false
        creandoProducto = false
        ultimaLectura = null
        codigoSinProducto = null
        errorDeEscaneo = null
        antiRebote.olvidar()
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
            message = "No alcanzamos a mandarle la venta al computador del negocio. Revisa que " +
                "el teléfono tenga señal y toca «Reintentar»: aunque la mandes de nuevo, no se " +
                "cobra dos veces.",
            retryLabel = "Reintentar",
        )

        else -> error.aCopy("la venta")
    }

    private fun pareceCodigoDeBarras(texto: String): Boolean =
        texto.length >= 8 && texto.all { it.isDigit() }
}
