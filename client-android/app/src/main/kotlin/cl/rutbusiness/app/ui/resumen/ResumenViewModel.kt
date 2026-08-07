package cl.rutbusiness.app.ui.resumen

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.async
import kotlinx.coroutines.launch

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
) : ViewModel() {

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
            moneda = MonedaRepository(api).resolver()

            val resumen = ResumenApi(api)
            val (desdeAyer, hastaAyer) = DiaUtc.rangoDeAyer(ahora())

            // Todo en paralelo. `async` sale a la red apenas se crea, así que
            // esperarlos en orden más abajo no los serializa: la pantalla tarda
            // lo que el más lento y no la suma de seis viajes por datos móviles.
            val agregado = async { resumen.dashboard() }
            val ayer = async { resumen.ventasDiarias(desdeAyer, hastaAyer) }
            val deuda = async { resumen.porCobrar() }
            val caja = async { cargarCaja(resumen) }
            val vencimientos = async { resumen.porVencer() }
            val faltantes = async { resumen.stockBajo() }

            when (val r = agregado.await()) {
                is Resultado.Ok -> {
                    ventasHoy = r.valor.ventasHoy
                    cuantosConStockBajo = r.valor.inventario.stockBajo
                    cuantosPorVencer = r.valor.porVencer
                }

                is Resultado.Falla -> {
                    error = r.error.aCopy("tu resumen del día")
                    if (r.error is AppError.SesionExpirada) sesion.salir()
                    cargando = false
                    return@launch
                }
            }

            aplicarAyer(ayer.await())
            porCobrar = (deuda.await() as? Resultado.Ok)?.valor
            stockBajo = (faltantes.await() as? Resultado.Ok)?.valor.orEmpty()
            porVencer = (vencimientos.await() as? Resultado.Ok)?.valor.orEmpty()
            // La caja también se espera antes de apagar el spinner: si no, el
            // bloque alcanzaría a decir "no hay caja abierta" y se corregiría
            // solo un segundo después, que es la clase de parpadeo que hace
            // desconfiar del número de al lado.
            caja.await()

            cargando = false
        }
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
     * Traduce el total de ayer en una palabra.
     *
     * `sales-daily` no devuelve fila para un día sin ventas, así que una lista
     * vacía significa "ayer no se vendió nada", que sí es comparable: si hoy
     * hay algo vendido, hoy fue mejor.
     */
    private fun aplicarAyer(resultado: Resultado<List<VentaDiariaDto>>) {
        val filas = (resultado as? Resultado.Ok)?.valor ?: return
        val hoy = ventasHoy?.revenue?.let { Dinero.deTextoDeServidor(it) } ?: return

        val ayer = filas.firstOrNull()?.revenue?.let { Dinero.deTextoDeServidor(it) }
            ?: Dinero.CERO
        ventasDeAyer = filas.firstOrNull()?.revenue ?: "0"

        comparacion = when {
            hoy > ayer -> Comparacion.Mejor
            hoy < ayer -> Comparacion.Peor
            else -> Comparacion.Igual
        }
    }
}
