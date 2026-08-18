package cl.rutbusiness.app.ui.cobrar

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.diag.Latencia
// El alta de productos ya no vive detrás del escáner: es del catálogo, y el
// escáner es uno de sus dos usuarios (`ui/catalogo/ProductosApi.kt`).
import cl.rutbusiness.app.ui.caja.AccionErrorRed
import cl.rutbusiness.app.ui.caja.ResultadoPuesto
import cl.rutbusiness.app.ui.caja.asegurarPuestoAbierto
import cl.rutbusiness.app.ui.caja.copyErrorRed
import cl.rutbusiness.app.ui.catalogo.asegurarVentaSuelta
import cl.rutbusiness.app.ui.catalogo.crearProducto
import cl.rutbusiness.app.ui.catalogo.pegarCodigo
import cl.rutbusiness.app.ui.catalogo.sinVentaSuelta
import cl.rutbusiness.app.ui.catalogo.ventaSueltaEn
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.common.esMasQueCero
import cl.rutbusiness.app.ui.common.montoParaElServidor
import cl.rutbusiness.app.ui.common.soloPlata
import cl.rutbusiness.app.ui.offline.ServiciosOffline
import cl.rutbusiness.app.ui.offline.soloCache
import cl.rutbusiness.app.ui.scanner.AntiRebote
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.catalog.ProductRepository
import cl.rutbusiness.core.customers.ClienteDto
import cl.rutbusiness.core.customers.CustomerRepository
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.offline.ClaveDeCache
import cl.rutbusiness.core.offline.ColaDeVentas
import cl.rutbusiness.core.offline.VentaEnCola
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
import kotlinx.serialization.builtins.ListSerializer

/**
 * Los momentos de una venta.
 *
 * [MontoSuelto] es un desvío de [Buscar] y no un paso más de la secuencia: se
 * entra a escribir un número y se sale con la línea puesta. Está en el mismo
 * `enum` porque es una pantalla completa —el teclado numérico al 200% no deja
 * lugar para nada más— y no un panel encima de la búsqueda.
 */
enum class PasoDeCobro { Buscar, MontoSuelto, Pago, Comprobante }

class CobrarViewModel(
    private val sesion: SessionRepository,
    /**
     * La cola, el caché y el estado de la conexión. `null` en las pruebas de
     * pantalla que no montan el grafo entero: sin esto la pantalla se comporta
     * como antes, asumiendo que hay red.
     */
    private val offline: ServiciosOffline? = null,
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

    // --- monto suelto --------------------------------------------------------

    /** Lo que se está escribiendo en el cobro rápido. */
    var montoSuelto by mutableStateOf("")
        private set

    /** `true` mientras se busca o se crea el producto centinela. */
    var preparandoMontoSuelto by mutableStateOf(false)
        private set

    /** Qué está mal con lo escrito, o qué falló al armar la línea. */
    var errorMontoSuelto by mutableStateOf<String?>(null)
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

    /**
     * Hay una lista de clientes viajando. La pantalla lo necesita para no
     * decirle "elige el cliente" a alguien que todavía no tiene entre quiénes
     * elegir: eso se lee como "no tengo clientes", no como "esperá".
     */
    var cargandoClientes by mutableStateOf(false)
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

    // --- sin red -------------------------------------------------------------

    /** `true` mientras se esté llegando al sistema del negocio. */
    val hayConexion: Boolean get() = offline?.conexion?.hayConexion?.value ?: true

    /**
     * La venta que quedó en la cola en vez de irse al server.
     *
     * Cuando esto no es `null`, el paso de comprobante muestra "guardada" en
     * lugar de un vuelto: no hay vuelto que mostrar porque no hay total, y no
     * hay total porque el total lo pone el server cuando la reciba.
     */
    var ventaEncolada by mutableStateOf<VentaEnCola?>(null)
        private set

    /**
     * Cuándo se trajo **lo que está en pantalla**.
     *
     * No es "cuándo se guardó la copia": es la hora de la lista que la cajera
     * está mirando ahora. La diferencia importa cuando la señal se corta con la
     * pantalla abierta — ahí lo que se ve se trajo hace un minuto aunque la
     * copia de disco sea de hace cinco horas, y poner la fecha vieja sería
     * mentir al revés.
     */
    var catalogoGuardadoEn by mutableStateOf<Long?>(null)
        private set

    /** El catálogo que se pudo guardar, para buscar adentro sin red. */
    private var catalogoLocal: List<ProductDto> = emptyList()

    /**
     * Pack feria: la Screen llama [modoFeria]; al entrar a Cobrar se abre el
     * puesto con $0 (o se trata el 409 como abierto) sin ritual de cajón.
     */
    private var esFeria = false

    /** Una sola corrida de [asegurarPuestoAbierto] por vida del VM. */
    private var puestoFeriaAsegurado = false

    init {
        viewModelScope.launch {
            sesion.apiActiva()?.let { moneda = MonedaRepository(it).resolver() }
        }
        // El orden importa y costó una pantalla vacía en el emulador: primero se
        // levanta de disco lo que ya se tenía guardado, **después** se busca.
        // Al revés, la primera búsqueda sin señal filtraba un catálogo local
        // todavía vacío y la cajera veía "Sin productos todavía" teniendo los
        // productos ahí mismo, en el teléfono. Leer el archivo son unos
        // milisegundos; la red, treinta segundos de timeout.
        viewModelScope.launch {
            levantarCatalogoDeDisco()
            buscar()
            refrescarCatalogoGuardado()
            asegurarCentinelaDeMontoSuelto()
        }
        // Los clientes del fiado se piden ACÁ, con el catálogo, y no al entrar
        // al paso de pago. En la feria el fiado no es un extra: es cómo
        // funciona el barrio. Pedirlos recién al tocar "Pagar" hacía que la
        // lista estuviera viajando justo cuando la cajera elige "Fiado", con el
        // vecino esperando — y con un catálogo grande el server tarda segundos.
        // La ventana buena es el rato en que se arma el carrito: no cuesta
        // nada y es la diferencia entre elegir a Juan y mirar una lista vacía.
        cargarClientes()
    }

    /**
     * Baja el flag de feria desde la Screen (`esFeria()` no se puede llamar acá).
     *
     * Si es feria y hay red, abre el puesto con $0 reusando [asegurarPuestoAbierto]
     * (mismo POST que Caja; 409 = ya abierto = éxito).
     */
    fun modoFeria(v: Boolean = true) {
        esFeria = v
        if (v) asegurarPuestoSiFeria()
    }

    private fun asegurarPuestoSiFeria() {
        if (!esFeria || puestoFeriaAsegurado || !hayConexion) return
        val api = sesion.apiActiva() ?: return
        puestoFeriaAsegurado = true
        viewModelScope.launch {
            when (asegurarPuestoAbierto(api)) {
                is ResultadoPuesto.Abierto -> Unit
                is ResultadoPuesto.Falla -> {
                    // Se deja reintentar si la red volvió o el server contestó mal
                    // una vez: no es un error de cobro, es preparación del día.
                    puestoFeriaAsegurado = false
                }
            }
        }
    }

    /**
     * Deja en disco los productos con los que se va a poder vender sin señal.
     *
     * Se refresca **cada seis horas como mucho**, no en cada entrada a la
     * pantalla. Traer el catálogo es la descarga más grande que hace la app, y
     * en un plan de datos chileno repetirla cada vez que la cajera vuelve a
     * Cobrar es plata de la dueña. Seis horas cubre el cambio de precios de un
     * día normal.
     *
     * Se guardan [LIMITE_CACHE] productos y no el catálogo entero: con 1-2 GB
     * de RAM la regla es cachear lo que se usa. Son unas decenas de KB en disco
     * y sólo están en memoria mientras esta pantalla vive.
     */
    private suspend fun levantarCatalogoDeDisco() {
        val servicios = offline ?: return
        val api = sesion.apiActiva() ?: return
        val guardado = soloCache(
            servicios,
            ClaveDeCache.Que.Catalogo,
            api.baseUrl,
            ListSerializer(ProductDto.serializer()),
        ) ?: return
        catalogoLocal = guardado.valor
        catalogoGuardadoEn = guardado.guardadoEn
    }

    /** Baja una copia nueva, si la que hay ya está vieja y hay con qué. */
    private suspend fun refrescarCatalogoGuardado() {
        val servicios = offline ?: return
        val api = sesion.apiActiva() ?: return
        if (!hayConexion) return
        val guardadoEn = catalogoGuardadoEn
        val fresco = guardadoEn != null &&
            servicios.reloj() - guardadoEn < VIGENCIA_DEL_CATALOGO_MS
        if (fresco) return

        when (val r = ProductRepository(api).buscar("", limite = LIMITE_CACHE)) {
            is Resultado.Ok -> {
                catalogoLocal = r.valor
                catalogoGuardadoEn = servicios.reloj()
                guardarCatalogoEnDisco()
            }

            is Resultado.Falla -> Unit
        }
    }

    /** Baja [catalogoLocal] a disco, tal como está. */
    private suspend fun guardarCatalogoEnDisco() {
        val servicios = offline ?: return
        val api = sesion.apiActiva() ?: return
        servicios.cache.guardar(
            ClaveDeCache.de(ClaveDeCache.Que.Catalogo, api.baseUrl),
            ListSerializer(ProductDto.serializer()),
            catalogoLocal,
        )
    }

    /**
     * Crea el centinela de los montos sueltos, si el negocio todavía no lo tiene.
     *
     * **Por qué acá y no cuando hace falta.** Cobrar un monto suelto sin señal
     * funciona: `asegurarVentaSuelta` mira primero el catálogo del teléfono. El
     * agujero es la primera vez, porque crearlo sí pide red — y la primera vez
     * cae justo en la feria, que es donde no hay. Desde que el centinela dejó de
     * necesitar reposición, crearlo pasó a ser algo que ocurre **una sola vez en
     * la vida del negocio**, así que se puede adelantar a un momento con señal.
     * Éste lo es: la pantalla ya está abierta, ya hubo red para el catálogo, y la
     * cajera no está esperando a que salga nada.
     *
     * Va **fuera** del corte de seis horas de [refrescarCatalogoGuardado] a
     * propósito: ahí se decide si vale la pena volver a bajar el catálogo entero,
     * y esto es otra cosa. Si el catálogo estuviera fresco pero el centinela no
     * existiera, quedaría sin crear justo en el caso que importa. Cuesta una
     * búsqueda por nombre y sólo mientras falte: apenas existe, la primera línea
     * corta sin salir a la red.
     *
     * Si falla, se calla: es una preparación, no una acción que alguien pidió.
     * El cobro sigue teniendo su propio camino y su propio mensaje.
     */
    private suspend fun asegurarCentinelaDeMontoSuelto() {
        if (ventaSueltaEn(catalogoLocal) != null) return
        if (!hayConexion) return
        val api = sesion.apiActiva() ?: return

        val r = asegurarVentaSuelta(api, catalogoLocal)
        if (r is Resultado.Ok && ventaSueltaEn(catalogoLocal) == null) {
            catalogoLocal = catalogoLocal + r.valor
            // A disco, no sólo a memoria: si esto se quedara en RAM, cerrar la
            // app antes de la primera venta suelta reabriría el agujero.
            guardarCatalogoEnDisco()
        }
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

        // Sin señal no se sale a la red: serían treinta segundos de espera para
        // terminar mostrando lo mismo que ya está en el teléfono. Se filtra el
        // catálogo guardado, que es instantáneo, y la pantalla dice de cuándo es.
        if (!hayConexion) {
            resultados = filtrarLocal(texto)
            buscando = false
            buscoAlgunaVez = true
            errorBusqueda = null
            return
        }

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
                is Resultado.Ok -> {
                    // Sin el centinela de los montos sueltos: es plomería, y una
                    // fila a $0 en medio de la búsqueda se agrega sin querer.
                    resultados = sinVentaSuelta(r.valor)
                    // La lista que se ve es de ahora. Se anota igual, porque si
                    // la señal se corta en el próximo minuto ésta es la hora
                    // que hay que mostrar.
                    catalogoGuardadoEn = offline?.reloj?.invoke()
                }

                is Resultado.Falla -> {
                    // Si lo que falló fue la red y hay catálogo guardado, se
                    // muestra ése en vez de una pantalla de error: el encargo
                    // es que la dueña pueda seguir mirando lo que ya cargó.
                    val local = filtrarLocal(texto)
                    if (r.error is AppError.ServidorNoResponde && local.isNotEmpty()) {
                        resultados = local
                        errorBusqueda = null
                    } else {
                        errorBusqueda = r.error.aCopy("tus productos")
                    }
                }
            }
            buscando = false
            buscoAlgunaVez = true
        }
    }

    /**
     * Busca adentro del catálogo guardado.
     *
     * Por nombre y por código de barras, sin acentos ni mayúsculas: la cajera
     * escribe "azucar" y el producto se llama "Azúcar". Es lo mismo que hace el
     * server con `search`, resuelto en el teléfono para que funcione sin red.
     */
    private fun filtrarLocal(texto: String): List<ProductDto> {
        val buscado = sinAcentos(texto)
        // El centinela vive en `catalogoLocal` para poder cobrar montos sueltos
        // sin señal, pero nunca se muestra como resultado.
        if (buscado.isEmpty()) return sinVentaSuelta(catalogoLocal)
        return sinVentaSuelta(catalogoLocal).filter { producto ->
            sinAcentos(producto.name).contains(buscado) ||
                producto.barcode?.contains(buscado) == true ||
                producto.slug.contains(buscado)
        }
    }

    private fun sinAcentos(texto: String): String = texto.trim().lowercase()
        .replace('á', 'a').replace('é', 'e').replace('í', 'i')
        .replace('ó', 'o').replace('ú', 'u').replace('ü', 'u')

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

    // --- monto suelto --------------------------------------------------------

    /**
     * Abre el cobro rápido, con el monto que ya hubiera cargado.
     *
     * Que arranque con lo que ya está puesto es la mitad de la función: la otra
     * forma de entrar acá es "me equivoqué, eran $2.500".
     */
    fun abrirMontoSuelto() {
        montoSuelto = montoSueltoEnElCarrito() ?: ""
        errorMontoSuelto = null
        paso = PasoDeCobro.MontoSuelto
    }

    fun cambiarMontoSuelto(valor: String) {
        // Mismo filtro que el resto de la plata: el teclado numérico de algunos
        // fabricantes deja meter símbolos, y un "$" llega al server como un
        // decimal inválido.
        montoSuelto = soloPlata(valor)
        errorMontoSuelto = null
    }

    fun cancelarMontoSuelto() {
        errorMontoSuelto = null
        paso = PasoDeCobro.Buscar
    }

    /**
     * Deja el monto escrito como una línea del carrito y va a cobrar.
     *
     * Va derecho al paso de pago porque en el puesto ese monto **es** la venta:
     * volver a la búsqueda sería devolver a la dueña a una lista que no pensaba
     * usar. Si además está vendiendo otras cosas, el carrito quedó igual y atrás
     * la trae de vuelta.
     */
    fun confirmarMontoSuelto() {
        if (preparandoMontoSuelto) return
        val escrito = montoSuelto.trim()
        val paraElServidor = montoParaElServidor(escrito)

        errorMontoSuelto = when {
            escrito.isEmpty() -> "Escribe cuánto vas a cobrar."
            paraElServidor == null -> "Ese monto no se entiende. Escribe sólo números."
            !esMasQueCero(escrito) -> "El monto tiene que ser mayor que cero."
            else -> null
        }
        if (errorMontoSuelto != null || paraElServidor == null) return

        val api = sesion.apiActiva() ?: return

        // Sin señal y sin haberlo usado nunca no hay a qué producto colgar la
        // línea, y el server no acepta líneas sin producto. Se dice así, con lo
        // que se puede hacer al respecto, en vez de un "no pudimos conectar" que
        // mandaría a revisar la señal cuando el problema es otro.
        val enElTelefono = ventaSueltaEn(catalogoLocal)
        if (enElTelefono == null && !hayConexion) {
            errorMontoSuelto = "La primera vez que cobres un monto suelto necesitas señal. " +
                "Después te funciona aunque no haya."
            return
        }

        preparandoMontoSuelto = true
        viewModelScope.launch {
            when (val r = asegurarVentaSuelta(api, catalogoLocal)) {
                is Resultado.Ok -> {
                    carrito = carrito.conMontoSuelto(r.valor, paraElServidor)
                    invalidarClave()
                    // Queda en el catálogo del teléfono para que la próxima -y la
                    // primera sin señal- no tenga que salir a la red. En disco,
                    // no sólo en memoria: cerrar la app no puede desandar esto.
                    if (ventaSueltaEn(catalogoLocal) == null) {
                        catalogoLocal = catalogoLocal + r.valor
                        guardarCatalogoEnDisco()
                    }
                    montoSuelto = ""
                    preparandoMontoSuelto = false
                    paso = PasoDeCobro.Pago
                }

                is Resultado.Falla -> {
                    errorMontoSuelto = r.error.userMessage
                    preparandoMontoSuelto = false
                }
            }
        }
    }

    /** El monto suelto que ya está en el carrito, si hay. */
    fun montoSueltoEnElCarrito(): String? {
        val centinela = ventaSueltaEn(catalogoLocal) ?: return null
        return carrito.items.firstOrNull { it.productoId == centinela.id }?.precioUnitario
    }

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

        // Sin señal el código se resuelve contra el catálogo guardado. No todos
        // los productos traen el código de barras adentro de la lista que manda
        // el server, así que esto puede no encontrarlo aunque el producto
        // exista — y en ese caso se dice, no se ofrece crearlo: crear un
        // producto necesita el server, y sin server sería un botón que falla.
        if (!hayConexion) {
            val local = catalogoLocal.firstOrNull { it.barcode == limpio }
            if (local != null) {
                agregar(local)
                codigoSinProducto = null
                errorDeEscaneo = null
                ultimaLectura = LecturaOk(
                    nombre = local.name,
                    enCarrito = cantidadEnCarrito(local.id),
                    orden = ++ordenDeLectura,
                )
            } else {
                ultimaLectura = null
                codigoSinProducto = null
                errorDeEscaneo = RbErrorCopy(
                    title = "No lo encontramos sin conexión",
                    message = "El código $limpio no está en los productos que alcanzamos a " +
                        "guardar. Búscalo por nombre, o espera a que vuelva la señal.",
                    retryLabel = null,
                )
            }
            return
        }

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
            // La sesión la cierra `llamar()` por su cuenta (`AvisoDeSesion`);
            // lo que falta acá es apagar la cámara, porque la app se está yendo
            // a la pantalla de entrada y nadie más lo haría.
            error is AppError.SesionExpirada -> escaneando = false

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

    /**
     * Trae a quién se le puede fiar. Se dispara al abrir la pantalla (ver
     * `init`) y otra vez al entrar al pago, que es el reintento: si la primera
     * falló por señal, `clientes` sigue vacía y esta vuelve a salir.
     *
     * El guard mira **las dos** cosas — que no haya lista y que no haya una
     * pedida en vuelo —. Con sólo `clientes.isNotEmpty()`, ir y volver entre
     * buscar y pagar disparaba una llamada a `/customers` por cada viaje,
     * encima de la que ya venía en camino.
     */
    private fun cargarClientes() {
        if (clientes.isNotEmpty() || cargandoClientes) return
        val api = sesion.apiActiva() ?: return
        cargandoClientes = true
        viewModelScope.launch {
            when (val r = CustomerRepository(api).listar()) {
                is Resultado.Ok -> clientes = r.valor
                // Que no se puedan cargar los clientes no rompe la pantalla:
                // efectivo y transferencia siguen andando. El fiado avisa solo
                // cuando el usuario lo elige y no hay a quién fiarle.
                is Resultado.Falla -> Unit
            }
            cargandoClientes = false
        }
    }

    /**
     * Qué medios de pago se pueden usar ahora mismo.
     *
     * Sin señal queda **sólo efectivo**, y eso se ve en la pantalla antes de
     * que la dueña toque nada. Los otros dos no es que fallen: es que no se
     * pueden hacer bien.
     *
     *  - **Fiado**: la cuenta corriente del cliente vive en el server. Anotar
     *    una deuda contra un saldo que no podemos leer es exactamente cómo se
     *    fía dos veces lo mismo.
     *  - **Transferencia**: la dueña la da por buena cuando ve la plata en su
     *    cuenta, y eso lo mira en otra app que tampoco tiene señal. Encolar una
     *    venta pagada con una transferencia que nadie confirmó sería registrar
     *    un cobro que puede no existir.
     */
    fun motivoParaNoUsar(opcion: MedioDePago): String? = when {
        hayConexion -> null
        opcion == MedioDePago.Efectivo -> null
        opcion == MedioDePago.Fiado -> copyMotivoNoUsarFiado(esFeria)
        else -> copyMotivoNoUsarTransferencia(esFeria)
    }

    /** `null` = se puede cobrar. Si no, el motivo, ya escrito para mostrar. */
    fun impedimentoParaCobrar(): String? = when {
        carrito.vacio -> "Agrega al menos un producto."
        motivoParaNoUsar(medio) != null -> motivoParaNoUsar(medio)
        // "Elige el cliente" delante de una lista vacía se lee como "no tienes
        // clientes", y manda a la cajera a crear uno que ya existe. Mientras la
        // lista viene, se dice que viene.
        medio.exigeCliente && cliente == null && cargandoClientes ->
            "Estamos trayendo tus clientes, un segundo."
        medio.exigeCliente && cliente == null ->
            "El fiado queda en la cuenta de alguien: elige el cliente."
        // Sin señal no se pide el monto entregado: el vuelto lo calcula el
        // server y sin server no hay vuelto que mostrar. Pedir el número
        // prometería una cuenta que no vamos a poder hacer.
        hayConexion && medio.pideMontoEntregado && montoEntregado.isBlank() ->
            "Escribe con cuánto te paga para calcular el vuelto."
        colaLlena -> "Hay demasiadas ventas esperando enviarse. Revisa la conexión antes de seguir."
        else -> null
    }

    /** La cola no da para una más. Se avisa antes de cobrar, no después. */
    private val colaLlena: Boolean
        get() = offline != null && offline.cola.cuantasEsperan >= ColaDeVentas.MAXIMO

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

            // Sin señal ni se intenta: se guarda y se avisa. Salir a la red
            // para esperar treinta segundos de timeout delante de un cliente
            // que quiere irse es tiempo regalado, y el resultado es el mismo.
            if (!hayConexion) {
                encolar(clave, solicitud)
                return@launch
            }

            when (val venta = repo.vender(solicitud, clave)) {
                is Resultado.Falla -> {
                    // La red se cortó justo al confirmar: el caso que este
                    // encargo existe para arreglar. La venta **no** se pierde
                    // ni queda esperando que alguien toque "Reintentar" — entra
                    // a la cola con la misma clave de idempotencia y sale sola.
                    // Si el server llegó a grabarla antes de que se cortara, el
                    // reintento contesta 200 con esa misma orden y no se cobra
                    // dos veces.
                    if (venta.error is AppError.ServidorNoResponde && sePuedeEncolar()) {
                        encolar(clave, solicitud)
                    } else {
                        errorPago = copyDeCobroFallido(venta.error)
                        cobrando = false
                        // El 401 no se maneja acá. El copy de sesión expirada
                        // promete llevar a la pantalla de entrada, y quien lo
                        // cumple es `sesion.salir()` — pero eso ya pasa solo:
                        // lo dispara `llamar()` para cualquier llamada de esta
                        // sesión (`AvisoDeSesion`). Acá la venta no se cobró, el
                        // server contestó 401, así que no hay plata en juego:
                        // sólo el carrito, que se pierde con la sesión.
                    }
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

    /**
     * Sólo el efectivo se encola.
     *
     * Es lo que se puede cobrar sin preguntarle nada a nadie: el cliente entregó
     * billetes y ya está. La transferencia y el fiado necesitan una
     * confirmación que sin server no existe, así que si una de ésas falla por
     * red se muestra el error de siempre con su "Reintentar" -que reusa la
     * misma clave y por lo tanto tampoco cobra dos veces-.
     */
    private fun sePuedeEncolar(): Boolean =
        offline != null && medio == MedioDePago.Efectivo && !colaLlena

    private suspend fun encolar(clave: String, solicitud: SolicitudDeVenta) {
        val servicios = offline
        if (servicios == null) {
            errorPago = copyDeCobroFallido(AppError.ServidorNoResponde(""))
            cobrando = false
            return
        }

        val venta = VentaEnCola(
            clave = clave,
            solicitud = solicitud,
            cobradaEn = servicios.reloj(),
            lineas = carrito.items.size,
        )

        if (!servicios.cola.encolar(venta)) {
            errorPago = RbErrorCopy(
                title = "No podemos guardar más ventas",
                message = "Ya hay ${ColaDeVentas.MAXIMO} ventas esperando enviarse. Revisa la " +
                    "conexión con el sistema del negocio antes de seguir cobrando.",
                retryLabel = null,
            )
            cobrando = false
            return
        }

        ventaEncolada = venta
        comprobante = null
        totalCobrado = null
        puntosGanados = 0
        paso = PasoDeCobro.Comprobante
        cobrando = false
    }

    /** El total que cobró el server. Es el número que manda, no el del carrito. */
    var totalCobrado by mutableStateOf<String?>(null)
        private set

    /** Cierra el comprobante y deja todo listo para la venta siguiente. */
    fun nuevaVenta() {
        carrito = Carrito()
        comprobante = null
        ventaEncolada = null
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
            message = copyErrorRed(feria = esFeria, accion = AccionErrorRed.Cobro),
            retryLabel = "Reintentar",
        )

        else -> error.aCopy("la venta")
    }

    private fun pareceCodigoDeBarras(texto: String): Boolean =
        texto.length >= 8 && texto.all { it.isDigit() }

    private companion object {
        /**
         * Cuántos productos se guardan para vender sin señal.
         *
         * 200 cubre el mostrador de un almacén de barrio -lo que se vende todos
         * los días- sin ser "el catálogo entero". En disco son unas decenas de
         * KB y en memoria sólo mientras esta pantalla vive, que es lo que un
         * aparato de 1-2 GB puede pagar.
         */
        const val LIMITE_CACHE = 200

        /**
         * Cada cuánto se vuelve a bajar esa copia.
         *
         * Seis horas. Traerla es la descarga más grande que hace la app, y
         * repetirla cada vez que la cajera vuelve a Cobrar son megas de un plan
         * de datos que paga la dueña. En seis horas entran los cambios de
         * precio de un día normal.
         */
        const val VIGENCIA_DEL_CATALOGO_MS = 6L * 60L * 60L * 1000L
    }
}
