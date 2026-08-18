package cl.rutbusiness.app.ui.resumen

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.offline.ServiciosOffline
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.offline.ClaveDeCache
import cl.rutbusiness.core.reports.ReportsApi
import cl.rutbusiness.core.reports.TopProductoDto
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable

/**
 * Cómo le fue hoy comparado con ayer.
 *
 * Se responde con una palabra y no con un porcentaje. "+12,4% MoM" obliga a la
 * dueña a traducir antes de entender; "mejor que ayer" ya es la conclusión.
 *
 * [SinDatoDeAyer] existe porque ayer sin ventas y ayer sin datos no son lo
 * mismo: si el negocio abrió hace dos días, decir "mejor que ayer" sería una
 * comparación contra un cero que nadie vivió.
 */
enum class Comparacion { Mejor, Igual, Peor, SinDatoDeAyer }

/**
 * El resumen entero, tal como se dibujó, para guardarlo en el teléfono.
 *
 * Guarda **los mismos textos que mandó el server** y no cifras recompuestas.
 * Ésa es la única forma de garantizar que lo que se muestre mañana sin señal
 * sea el mismo número que la dueña ya vio hoy, y no uno que el teléfono volvió
 * a armar por su cuenta.
 */
@Serializable
data class ResumenGuardado(
    val ventasHoy: VentasDto?,
    val ventasDeAyer: String?,
    val comparacion: String,
    /**
     * Los 7 días de la semana, para que el gráfico siga ahí sin señal. Sin
     * este campo el bloque semanal desaparecería justo el día que más se
     * necesita mirar el teléfono guardado — ver [ResumenViewModel.guardar].
     */
    val ventasSemana: List<DiaDeLaSemana> = emptyList(),
    /**
     * Lo que más se vendió en el mes, para "Lo que más vendes". Falla sola:
     * si el reporte no cargó, la lista queda vacía y el bloque simplemente no
     * se dibuja — nunca tumba el resto del snapshot.
     */
    val masVendes: List<TopProductoDto> = emptyList(),
    val enCaja: String?,
    val nombreDeCaja: String?,
    val sinCajaAbierta: Boolean,
    val porCobrar: PorCobrarDto?,
    val stockBajo: List<ProductDto>,
    val cuantosConStockBajo: Int,
    val porVencer: List<LotePorVencerDto>,
    val cuantosPorVencer: Int,
)

/**
 * El resumen del día.
 *
 * **Ningún número de plata se calcula acá.** El server manda cada monto como
 * texto decimal y esta clase lo pasa entero a [Moneda.formatear]. Lo único que
 * se resuelve en el teléfono es un `mayor/igual/menor` entre dos totales que el
 * server ya sumó, y eso no produce un monto nuevo — ver
 * [cl.rutbusiness.core.money.Dinero.compareTo].
 *
 * Los bloques se cargan en paralelo y **caen por separado**. Que el negocio no
 * tenga caja abierta, o que el reporte de fiado falle, no puede borrar la cifra
 * de ventas del día: cada bloque sabe decir que no pudo, y el resto sigue en
 * pantalla. La única falla que se lleva la pantalla entera es la del agregado,
 * porque sin eso no queda resumen que mostrar.
 */
class ResumenViewModel(
    private val sesion: SessionRepository,
    /** Inyectable para que las pruebas fijen el día sin depender del reloj real. */
    private val ahora: () -> Long = { System.currentTimeMillis() },
    /** El caché y el estado de la conexión. `null` en las pruebas de pantalla. */
    private val offline: ServiciosOffline? = null,
) : ViewModel() {

    /**
     * Cuándo se trajo esto, si no es de ahora.
     *
     * `null` significa "recién traído del sistema". Cuando trae fecha, la
     * pantalla la muestra al lado de las cifras — un resumen del día sin fecha
     * es un número en el que la dueña va a confiar como si fuera de este
     * minuto, y puede ser de anoche.
     */
    var guardadoEn by mutableStateOf<Long?>(null)
        private set

    /** Nombre del puesto, si el último login lo trajo. */
    var nombreDelPuesto by mutableStateOf<String?>(null)
        private set

    /** El reloj de pared, para escribir "hace 20 minutos". */
    val relojDePared: Long get() = offline?.reloj?.invoke() ?: ahora()

    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    var cargando by mutableStateOf(true)
        private set

    /** Falla que deja la pantalla sin nada que mostrar. */
    var error by mutableStateOf<RbErrorCopy?>(null)
        private set

    // --- 1. cuánto se vendió hoy --------------------------------------------

    var ventasHoy by mutableStateOf<VentasDto?>(null)
        private set

    // --- 2. comparado con ayer ----------------------------------------------

    var comparacion by mutableStateOf(Comparacion.SinDatoDeAyer)
        private set

    /** Lo de ayer, tal como lo mandó el server. `null` si no se pudo traer. */
    var ventasDeAyer by mutableStateOf<String?>(null)
        private set

    /**
     * Los 7 días de la semana, del más viejo al de hoy. `null` mientras no se
     * pudo traer — distinto de una lista vacía, que nunca ocurre una vez que
     * carga bien: siempre son 7 días, completados en cero si no hubo ventas.
     */
    var ventasSemana by mutableStateOf<List<DiaDeLaSemana>?>(null)
        private set

    // --- 2b. lo que más vendes en el mes -------------------------------------

    /**
     * Los productos que más venden este mes, ya en el orden del server
     * (limitados a [LIMITE_MAS_VENDES]). Vacía si el reporte no cargó o si
     * todavía no hay ventas en el mes — en los dos casos el bloque no se
     * dibuja, nunca a medias.
     */
    var masVendes by mutableStateOf<List<TopProductoDto>>(emptyList())
        private set

    // --- 3. cuánta plata hay en caja ----------------------------------------

    /** Lo que el arqueo dice que debería haber en el cajón. */
    var enCaja by mutableStateOf<String?>(null)
        private set
    var nombreDeCaja by mutableStateOf<String?>(null)
        private set
    var sinCajaAbierta by mutableStateOf(false)
        private set

    // --- 4. cuánto le deben --------------------------------------------------

    var porCobrar by mutableStateOf<PorCobrarDto?>(null)
        private set

    // --- 5. qué se está por acabar y qué se vence ---------------------------

    var stockBajo by mutableStateOf<List<ProductDto>>(emptyList())
        private set
    var cuantosConStockBajo by mutableStateOf(0)
        private set

    var porVencer by mutableStateOf<List<LotePorVencerDto>>(emptyList())
        private set
    var cuantosPorVencer by mutableStateOf(0)
        private set

    init {
        cargar()
    }

    fun cargar() {
        val api = sesion.apiActiva()
        if (api == null) {
            cargando = false
            error = AppError.SesionExpirada().aCopy("tu resumen")
            return
        }

        cargando = true
        error = null

        viewModelScope.launch {
            nombreDelPuesto = sesion.nombreDelPuesto()
            // Sin señal ni se sale a la red: seis llamadas que van a morir por
            // timeout son treinta segundos de spinner para terminar mostrando
            // lo que ya está guardado en el teléfono.
            if (offline?.conexion?.hayConexion?.value == false) {
                if (mostrarGuardado(api.baseUrl)) {
                    cargando = false
                    return@launch
                }
            }

            moneda = MonedaRepository(api).resolver()

            val resumen = ResumenApi(api)
            val reportes = ReportsApi(api)
            val (desdeSemana, hastaSemana) = DiaUtc.rangoDeSemana(ahora())
            val (desdeMes, hastaMes) = DiaUtc.rangoDelMes(ahora())

            // Todo en paralelo. `async` sale a la red apenas se crea, así que
            // esperarlos en orden más abajo no los serializa: la pantalla tarda
            // lo que el más lento y no la suma de seis viajes por datos móviles.
            //
            // Una sola llamada a `sales-daily`, con el rango ampliado a los 6
            // días completos antes de hoy: la comparación con ayer y el
            // gráfico de la semana entera salen de la misma respuesta, sin
            // pedir la red dos veces por el mismo dato.
            val agregado = async { resumen.dashboard() }
            val semana = async { resumen.ventasDiarias(desdeSemana, hastaSemana) }
            val deuda = async { resumen.porCobrar() }
            val caja = async { cargarCaja(resumen) }
            val vencimientos = async { resumen.porVencer() }
            val faltantes = async { resumen.stockBajo() }
            val topProductos = async {
                reportes.topProductos(desde = desdeMes, hasta = hastaMes, limite = LIMITE_MAS_VENDES)
            }

            when (val r = agregado.await()) {
                is Resultado.Ok -> {
                    ventasHoy = r.valor.ventasHoy
                    cuantosConStockBajo = r.valor.inventario.stockBajo
                    cuantosPorVencer = r.valor.porVencer
                }

                is Resultado.Falla -> {
                    // El agregado es lo único sin lo cual no hay resumen. Si lo
                    // que falló fue la red y hay una copia guardada, se muestra
                    // ésa con su fecha: el encargo es que la dueña pueda mirar
                    // cuánto vendió aunque el sistema no conteste.
                    val salvado = r.error is AppError.ServidorNoResponde &&
                        mostrarGuardado(api.baseUrl)
                    if (!salvado) {
                        error = r.error.aCopy("tu resumen del día")
                    }
                    cargando = false
                    return@launch
                }
            }
            guardadoEn = null

            aplicarSemana(semana.await())
            porCobrar = (deuda.await() as? Resultado.Ok)?.valor
            stockBajo = (faltantes.await() as? Resultado.Ok)?.valor.orEmpty()
            porVencer = (vencimientos.await() as? Resultado.Ok)?.valor.orEmpty()
            masVendes = (topProductos.await() as? Resultado.Ok)?.valor.orEmpty()
            // La caja también se espera antes de apagar el spinner: si no, el
            // bloque alcanzaría a decir "no hay caja abierta" y se corregiría
            // solo un segundo después, que es la clase de parpadeo que hace
            // desconfiar del número de al lado.
            caja.await()

            guardar(api.baseUrl)
            cargando = false
        }
    }

    /**
     * Guarda lo que se está mostrando, para el día que no haya señal.
     *
     * Se guardan **los textos que mandó el server**, no cifras recompuestas: lo
     * que se escribe en disco es exactamente lo mismo que se dibujó, así que
     * volver a mostrarlo mañana no puede producir un número distinto del que la
     * dueña ya vio.
     */
    private suspend fun guardar(baseUrl: String) {
        val servicios = offline ?: return
        servicios.cache.guardar(
            ClaveDeCache.de(ClaveDeCache.Que.Resumen, baseUrl),
            ResumenGuardado.serializer(),
            ResumenGuardado(
                ventasHoy = ventasHoy,
                ventasDeAyer = ventasDeAyer,
                comparacion = comparacion.name,
                ventasSemana = ventasSemana.orEmpty(),
                masVendes = masVendes,
                enCaja = enCaja,
                nombreDeCaja = nombreDeCaja,
                sinCajaAbierta = sinCajaAbierta,
                porCobrar = porCobrar,
                stockBajo = stockBajo,
                cuantosConStockBajo = cuantosConStockBajo,
                porVencer = porVencer,
                cuantosPorVencer = cuantosPorVencer,
            ),
        )
    }

    /** Pinta la copia guardada. `false` si no había ninguna. */
    private suspend fun mostrarGuardado(baseUrl: String): Boolean {
        val servicios = offline ?: return false
        val guardado = servicios.cache.leer(
            ClaveDeCache.de(ClaveDeCache.Que.Resumen, baseUrl),
            ResumenGuardado.serializer(),
        ) ?: return false

        val datos = guardado.valor
        ventasHoy = datos.ventasHoy
        ventasDeAyer = datos.ventasDeAyer
        comparacion = Comparacion.entries.firstOrNull { it.name == datos.comparacion }
            ?: Comparacion.SinDatoDeAyer
        ventasSemana = datos.ventasSemana.ifEmpty { null }
        masVendes = datos.masVendes
        enCaja = datos.enCaja
        nombreDeCaja = datos.nombreDeCaja
        sinCajaAbierta = datos.sinCajaAbierta
        porCobrar = datos.porCobrar
        stockBajo = datos.stockBajo
        cuantosConStockBajo = datos.cuantosConStockBajo
        porVencer = datos.porVencer
        cuantosPorVencer = datos.cuantosPorVencer
        guardadoEn = guardado.guardadoEn
        error = null
        return true
    }

    /**
     * La plata que hay en el cajón ahora.
     *
     * Dos llamadas encadenadas porque el arqueo se pide por caja: primero cuál
     * está abierta, después cuánto tiene. Si no hay ninguna abierta no es un
     * error — es un negocio que todavía no abrió — y la pantalla lo dice así.
     */
    private suspend fun cargarCaja(api: ResumenApi) {
        val abiertas = (api.cajasAbiertas() as? Resultado.Ok)?.valor ?: return
        val caja = abiertas.firstOrNull()
        if (caja == null) {
            sinCajaAbierta = true
            return
        }
        nombreDeCaja = caja.nombre.ifBlank { null }
        val arqueo = (api.arqueo(caja.id) as? Resultado.Ok)?.valor ?: return
        enCaja = arqueo.session.esperado
        arqueo.session.nombre.takeIf { it.isNotBlank() }?.let { nombreDeCaja = it }
    }

    /**
     * Traduce el total de ayer en una palabra, y arma los 7 días del gráfico
     * semanal a partir de la misma respuesta.
     *
     * Ayer se busca **por fecha** en `filas`, nunca por posición: con el
     * rango ampliado a 7 días, `filas.firstOrNull()` pasaría a ser el día más
     * viejo de la serie, y la pantalla diría "mejor que ayer" comparando
     * contra hace una semana sin que ningún test se entere. `sales-daily` no
     * devuelve fila para un día sin ventas, así que no encontrar la fecha de
     * ayer significa "ayer no se vendió nada", que sí es comparable: si hoy
     * hay algo vendido, hoy fue mejor.
     */
    private fun aplicarSemana(resultado: Resultado<List<VentaDiariaDto>>) {
        val filas = (resultado as? Resultado.Ok)?.valor ?: return
        val ventasDeHoy = ventasHoy ?: return
        val hoy = Dinero.deTextoDeServidor(ventasDeHoy.revenue) ?: return

        val diasEsqueleto = DiaUtc.diasDeLaSemana(ahora())
        val fechaDeAyer = diasEsqueleto.lastOrNull { !it.esHoy }?.fecha
        val filaDeAyer = filas.firstOrNull { it.date == fechaDeAyer }
        val ayer = filaDeAyer?.revenue?.let { Dinero.deTextoDeServidor(it) } ?: Dinero.CERO
        ventasDeAyer = filaDeAyer?.revenue ?: "0"

        comparacion = when {
            hoy > ayer -> Comparacion.Mejor
            hoy < ayer -> Comparacion.Peor
            else -> Comparacion.Igual
        }

        ventasSemana = armarSemana(diasEsqueleto, filas, ventasDeHoy.revenue)
    }

    companion object {
        /** Cinco filas es lo que se lee de un vistazo — ver el encargo del bloque. */
        const val LIMITE_MAS_VENDES = 5
    }
}
