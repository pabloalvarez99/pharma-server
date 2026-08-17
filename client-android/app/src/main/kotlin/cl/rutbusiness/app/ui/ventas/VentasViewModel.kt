package cl.rutbusiness.app.ui.ventas

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.resumen.DiaUtc
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.launch

/**
 * Cuántas ventas se piden de una vez. Un puesto de feria no hace miles de
 * ventas en un día; con esto sobra para "hoy" sin paginar.
 */
private const val LIMITE_VENTAS_DEL_DIA = 200

/**
 * Los pasos de "Lo que vendiste": la lista del día, el detalle de una venta, y
 * deshacerla.
 */
enum class PasoDeVentas {
    /** La lista de ventas de hoy. */
    Lista,

    /** Qué llevaba una venta, cómo pagó, cuándo. */
    Detalle,

    /** El paso de confirmar antes de deshacer, adentro de la misma pantalla. */
    Confirmando,
}

/**
 * Ver las ventas de hoy y deshacer la que corresponda.
 *
 * **Ningún monto se calcula acá.** No se suma un total del día a partir de la
 * lista — si hace falta un total, es el que ya da el resumen. Para saber si
 * una venta está devuelta se lee [VentaDto.estaDevuelta] (que ya viene resuelto
 * por el server), nunca restando `refunded_total` de `total`.
 *
 * El rango del día se arma en UTC con el mismo criterio que usa el resumen —
 * ver [DiaUtc] — para que "hoy" acá sea el mismo "hoy" que ya usa el resto de
 * la app. [DiaUtc] no se toca: es de la ola 28 y su propio test lo fija.
 */
class VentasViewModel(
    private val sesion: SessionRepository,
    /** Inyectable para que las pruebas fijen el día sin depender del reloj real. */
    private val ahora: () -> Long = { System.currentTimeMillis() },
) : ViewModel() {

    var paso by mutableStateOf(PasoDeVentas.Lista)
        private set

    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    var cargando by mutableStateOf(true)
        private set

    /**
     * Un error de red no borra lo que ya se mostró: se guarda acá y la lista
     * que ya estaba en pantalla sigue abajo.
     */
    var error by mutableStateOf<RbErrorCopy?>(null)
        private set

    var ventas by mutableStateOf<List<VentaDto>>(emptyList())
        private set

    // --- detalle ---------------------------------------------------------------

    var ventaElegida by mutableStateOf<VentaDto?>(null)
        private set
    var detalle by mutableStateOf<DetalleDeVentaDto?>(null)
        private set
    var cargandoDetalle by mutableStateOf(false)
        private set
    var errorDeAccion by mutableStateOf<RbErrorCopy?>(null)
        private set

    // --- deshacer ---------------------------------------------------------------

    var deshaciendo by mutableStateOf(false)
        private set

    /** Lo que se le dice a la dueña después de deshacer con éxito. */
    var avisoDeDeshecho by mutableStateOf<String?>(null)
        private set

    init {
        cargar()
    }

    fun cargar() {
        val api = sesion.apiActiva()
        if (api == null) {
            cargando = false
            error = AppError.SesionExpirada().aCopy("lo que vendiste")
            return
        }

        cargando = true
        error = null

        viewModelScope.launch {
            moneda = MonedaRepository(api).resolver()

            val comienzoDeHoy = DiaUtc.comienzoDelDia(ahora())
            val desde = DiaUtc.rfc3339(comienzoDeHoy)
            val hasta = DiaUtc.rfc3339(ahora())

            when (val r = VentasApi(api).ventas(desde, hasta, LIMITE_VENTAS_DEL_DIA)) {
                is Resultado.Ok -> {
                    // Del más nuevo al más viejo, tal como se pide en el encargo.
                    // El server no promete un orden; se ordena por fecha acá sin
                    // tocar ningún monto.
                    ventas = r.valor.sortedByDescending { it.creadaEn }
                }

                is Resultado.Falla -> error = r.error.aCopy("lo que vendiste")
            }

            cargando = false
        }
    }

    // --- detalle -------------------------------------------------------------

    fun abrirVenta(venta: VentaDto) {
        ventaElegida = venta
        detalle = null
        errorDeAccion = null
        avisoDeDeshecho = null
        paso = PasoDeVentas.Detalle
        recargarDetalle()
    }

    private fun recargarDetalle() {
        val api = sesion.apiActiva() ?: return
        val id = ventaElegida?.id ?: return

        cargandoDetalle = true
        viewModelScope.launch {
            when (val r = VentasApi(api).detalle(id)) {
                is Resultado.Ok -> {
                    detalle = r.valor
                    ventaElegida = r.valor.order
                }

                is Resultado.Falla -> errorDeAccion = r.error.aCopy("esta venta")
            }
            cargandoDetalle = false
        }
    }

    fun volverALaLista() {
        paso = PasoDeVentas.Lista
        ventaElegida = null
        detalle = null
        errorDeAccion = null
    }

    // --- deshacer -------------------------------------------------------------

    fun pedirConfirmarDeshacer() {
        errorDeAccion = null
        paso = PasoDeVentas.Confirmando
    }

    fun cancelarDeshacer() {
        paso = PasoDeVentas.Detalle
    }

    /**
     * Deshace la venta abierta.
     *
     * Los items del cuerpo salen de [detalle] tal como los mandó el server —
     * mismo `unit_price` y `quantity` — nunca recompuestos desde el subtotal.
     * Después de un éxito se recarga la lista del server: la fila no se toca
     * en memoria, porque el server es el que sabe cómo quedó.
     */
    fun deshacer(feria: Boolean) {
        if (deshaciendo) return
        val api = sesion.apiActiva() ?: return
        val det = detalle ?: return
        val venta = ventaElegida ?: return
        // Una venta ya devuelta no ofrece el botón en la pantalla, pero se
        // repite el guardia acá: nada dispara un segundo POST sobre la misma
        // venta aunque algo más arriba se equivoque.
        if (venta.estaDevuelta) return
        if (det.items.isEmpty()) return

        deshaciendo = true
        errorDeAccion = null

        viewModelScope.launch {
            val devolucion = NuevaDevolucion(
                order = venta.id,
                motivo = motivoDeDevolucion(feria),
                items = det.items.map { item ->
                    ItemDeDevolucion(
                        product = item.product,
                        nombreDelProducto = item.nombreDelProducto,
                        quantity = item.quantity,
                        precioUnitario = item.precioUnitario,
                        restock = true,
                    )
                },
                metodoDeReembolso = venta.metodoDePago.takeIf { it.isNotBlank() },
            )

            when (val r = VentasApi(api).deshacer(devolucion)) {
                is Resultado.Ok -> {
                    avisoDeDeshecho = avisoDeshecha(feria)
                    paso = PasoDeVentas.Detalle
                    recargarDetalle()
                    recargarLista(api)
                }

                is Resultado.Falla -> {
                    errorDeAccion = copyDeDeshacerFallido(r.error, feria)
                    paso = PasoDeVentas.Detalle
                }
            }

            deshaciendo = false
        }
    }

    private suspend fun recargarLista(api: ApiFactory) {
        val comienzoDeHoy = DiaUtc.comienzoDelDia(ahora())
        val desde = DiaUtc.rfc3339(comienzoDeHoy)
        val hasta = DiaUtc.rfc3339(ahora())
        (VentasApi(api).ventas(desde, hasta, LIMITE_VENTAS_DEL_DIA) as? Resultado.Ok)?.let {
            ventas = it.valor.sortedByDescending { venta -> venta.creadaEn }
        }
    }

    private fun copyDeDeshacerFallido(error: AppError, feria: Boolean): RbErrorCopy = when (error) {
        is AppError.SinPermiso -> RbErrorCopy(
            title = tituloErrorDeshacer(feria),
            message = mensajeSinPermisoDeshacer(feria),
            retryLabel = null,
        )

        else -> error.aCopy(if (feria) "deshacer la venta" else "devolver la venta")
    }
}
