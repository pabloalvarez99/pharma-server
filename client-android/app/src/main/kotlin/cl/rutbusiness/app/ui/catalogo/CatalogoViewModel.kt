package cl.rutbusiness.app.ui.catalogo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.common.esMasQueCero
import cl.rutbusiness.app.ui.common.montoParaElServidor
import cl.rutbusiness.app.ui.common.soloPlata
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.catalog.ProductRepository
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/** Los dos momentos de esta pantalla: mirar la lista, o cargar una cosa. */
enum class PasoDelCatalogo { Lista, Formulario }

/**
 * "Lo que vendo": la lista de lo que el negocio vende y el alta a mano.
 *
 * Es el camino que faltaba. Hasta ahora un negocio recién creado sólo podía
 * cargar su primer producto escaneando un código, y el pack `feria` apaga el
 * escáner a propósito -en un puesto no hay EAN pegado en el tomate-, así que el
 * único camino estaba cerrado justo para el usuario objetivo.
 *
 * Tres campos en el camino feliz y ninguno más: nombre, precio, y cómo se vende.
 * Todo lo demás que el server acepta -categoría, costo, código, lote- se puede
 * cargar después desde otra parte; pedirlo acá convertiría "cargá lo que vendés"
 * en un formulario de sistema.
 */
class CatalogoViewModel(
    private val sesion: SessionRepository,
    /** Clave del atributo de unidad que declara el pack (`attrs.unidad`). */
    private val claveDeUnidad: String = CLAVE_UNIDAD,
) : ViewModel() {

    var paso by mutableStateOf(PasoDelCatalogo.Lista)
        private set

    /** Con qué moneda se escribe la plata. Sale del server, como en Cobrar. */
    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    // --- lista ---------------------------------------------------------------

    var consulta by mutableStateOf("")
        private set
    var cosas by mutableStateOf<List<ProductDto>>(emptyList())
        private set
    var cargando by mutableStateOf(true)
        private set
    var errorDeLista by mutableStateOf<RbErrorCopy?>(null)
        private set

    private var trabajoDeBusqueda: Job? = null

    // --- formulario ----------------------------------------------------------

    /** `null` = alta. Con valor = se está editando esa cosa. */
    var editando by mutableStateOf<ProductDto?>(null)
        private set
    var nombre by mutableStateOf("")
        private set
    var precio by mutableStateOf("")
        private set
    var unidad by mutableStateOf("")
        private set
    var guardando by mutableStateOf(false)
        private set

    /** Qué campo está mal, ya escrito para mostrar debajo del campo. */
    var errorDeNombre by mutableStateOf<String?>(null)
        private set
    var errorDePrecio by mutableStateOf<String?>(null)
        private set

    /** Una falla del server al guardar (no de validación). */
    var errorAlGuardar by mutableStateOf<RbErrorCopy?>(null)
        private set

    /** Lo último que se guardó bien, para confirmarlo arriba de la lista. */
    var recienGuardado by mutableStateOf<String?>(null)
        private set

    init {
        viewModelScope.launch {
            sesion.apiActiva()?.let { moneda = MonedaRepository(it).resolver() }
        }
        cargar()
    }

    // --- lista ---------------------------------------------------------------

    fun cargar() {
        val api = sesion.apiActiva() ?: run {
            cargando = false
            return
        }
        cargando = true
        errorDeLista = null
        trabajoDeBusqueda?.cancel()
        trabajoDeBusqueda = viewModelScope.launch {
            when (val r = ProductRepository(api).buscar(consulta.trim())) {
                is Resultado.Ok -> {
                    // Sin el centinela de los montos sueltos: la dueña no lo
                    // cargó, no lo puede explicar, y editarlo rompería el cobro
                    // rápido. Ver [VentaSuelta].
                    cosas = sinVentaSuelta(r.valor)
                    errorDeLista = null
                }

                is Resultado.Falla -> {
                    errorDeLista = r.error.aCopy("lo que vendes")
                    if (r.error is AppError.SesionExpirada) sesion.salir()
                }
            }
            cargando = false
        }
    }

    fun cambiarConsulta(valor: String) {
        consulta = valor
        // Mismo rebote que Cobrar: cada tecla no puede ser un viaje por datos
        // móviles caros.
        trabajoDeBusqueda?.cancel()
        trabajoDeBusqueda = viewModelScope.launch {
            delay(250)
            cargar()
        }
    }

    // --- formulario ----------------------------------------------------------

    /** Abre el formulario vacío. */
    fun nuevaCosa() {
        editando = null
        nombre = ""
        precio = ""
        unidad = ""
        limpiarErrores()
        recienGuardado = null
        paso = PasoDelCatalogo.Formulario
    }

    /** Abre el formulario cargado con lo que ya tiene esa cosa. */
    fun editar(cosa: ProductDto) {
        editando = cosa
        nombre = cosa.name
        precio = cosa.price
        unidad = unidadDe(cosa)
        limpiarErrores()
        recienGuardado = null
        paso = PasoDelCatalogo.Formulario
    }

    fun volverALaLista() {
        paso = PasoDelCatalogo.Lista
        limpiarErrores()
    }

    fun cambiarNombre(valor: String) {
        nombre = valor
        errorDeNombre = null
        errorAlGuardar = null
    }

    fun cambiarPrecio(valor: String) {
        // El teclado numérico de Android igual deja meter símbolos según el
        // fabricante, y un `$` llega al server como un decimal inválido.
        precio = soloPlata(valor)
        errorDePrecio = null
        errorAlGuardar = null
    }

    fun cambiarUnidad(valor: String) {
        unidad = valor
        errorAlGuardar = null
    }

    /** Toca un chip: si ya estaba elegido, lo suelta. */
    fun elegirUnidad(sugerida: String) {
        unidad = if (unidad.equals(sugerida, ignoreCase = true)) "" else sugerida
        errorAlGuardar = null
    }

    /**
     * Guarda: crea o edita según [editando].
     *
     * El precio viaja **dígito por dígito** como lo escribió la dueña. No se
     * redondea ni se reescala acá: la moneda del tenant puede tener 0 o 2
     * decimales y esta pantalla no tiene por qué saber cuál es.
     */
    fun guardar() {
        if (guardando) return
        val limpioNombre = nombre.trim()
        val limpioPrecio = precio.trim()

        errorDeNombre = if (limpioNombre.isEmpty()) {
            "Ponle un nombre: es lo que vas a buscar cuando cobres."
        } else {
            null
        }
        errorDePrecio = when {
            limpioPrecio.isEmpty() -> "Falta el precio."
            montoParaElServidor(limpioPrecio) == null -> "Ese precio no se entiende. Escribe sólo números."
            !esMasQueCero(limpioPrecio) -> "El precio tiene que ser mayor que cero."
            else -> null
        }
        if (errorDeNombre != null || errorDePrecio != null) return

        val paraElServidor = montoParaElServidor(limpioPrecio) ?: return
        val api = sesion.apiActiva() ?: return
        val cosa = editando
        val unidadEscrita = unidad.trim()

        guardando = true
        errorAlGuardar = null

        viewModelScope.launch {
            val r = if (cosa == null) {
                crearProducto(
                    api = api,
                    nombre = limpioNombre,
                    precio = paraElServidor,
                    // Nace con una unidad, igual que en el escáner: con cero el
                    // server rechaza la venta por stock insuficiente y cargar la
                    // cosa no habría servido de nada.
                    stock = 1,
                    unidad = unidadEscrita,
                    claveDeUnidad = claveDeUnidad,
                )
            } else {
                editarProducto(
                    api = api,
                    productoId = cosa.id,
                    nombre = limpioNombre,
                    precio = paraElServidor,
                    unidad = unidadEscrita,
                    previos = atributosDe(cosa),
                    claveDeUnidad = claveDeUnidad,
                )
            }

            when (r) {
                is Resultado.Ok -> {
                    recienGuardado = r.valor.name
                    guardando = false
                    paso = PasoDelCatalogo.Lista
                    // La lista se recarga contra el server y no se parchea en
                    // memoria: el server pone el slug, normaliza el precio y
                    // puede haber otra caja cargando al mismo tiempo.
                    cargar()
                }

                is Resultado.Falla -> {
                    errorAlGuardar = r.error.aCopy("lo que estabas cargando")
                    guardando = false
                    if (r.error is AppError.SesionExpirada) sesion.salir()
                }
            }
        }
    }

    fun olvidarConfirmacion() {
        recienGuardado = null
    }

    /**
     * Cómo se vende esa cosa, leído de `attrs`. Vacío si no lo tiene.
     *
     * Todo el camino es a prueba de basura: `attrs` es JSON libre del server, y
     * ni `.jsonPrimitive` -que **tira** si el valor es un objeto- ni
     * `JsonNull.content` -que devuelve el texto `"null"` y lo dibujaría adentro
     * del chip- se pueden usar a secas sobre algo que no controlamos.
     */
    fun unidadDe(cosa: ProductDto): String {
        val valor = atributosDe(cosa)?.get(claveDeUnidad) as? JsonPrimitive ?: return ""
        if (valor is JsonNull) return ""
        return valor.content
    }

    private fun limpiarErrores() {
        errorDeNombre = null
        errorDePrecio = null
        errorAlGuardar = null
    }

    private fun atributosDe(cosa: ProductDto): JsonObject? = cosa.attrs as? JsonObject
}
