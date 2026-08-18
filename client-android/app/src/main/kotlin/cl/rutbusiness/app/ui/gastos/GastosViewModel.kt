package cl.rutbusiness.app.ui.gastos

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.app.ui.common.aCopy
import cl.rutbusiness.app.ui.resumen.DiaUtc
import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbErrorCopy
import kotlinx.coroutines.launch

/** Cuántos gastos se piden de una vez. Sobra para un mes de puesto sin paginar. */
private const val LIMITE_GASTOS = 200

/** El rango que se está mirando: hoy, o este mes. */
enum class RangoDeGastos { Hoy, EsteMes }

/**
 * Ver lo que salió del puesto — hoy o este mes — y anotar un gasto nuevo.
 *
 * **Ningún monto se suma acá.** El endpoint no trae un total del período; si
 * hace falta uno se le pide al agente, nunca se arma sumando filas en el
 * teléfono — un total así discreparía del resto del sistema el día que se
 * pagine. El rango se arma en UTC con el mismo criterio que usa el resumen —
 * ver [DiaUtc] — y **siempre sobre `incurred_at`**, nunca `created_at`: un
 * gasto de ayer anotado hoy tiene que caer en el día en que se gastó, no en el
 * día en que se escribió.
 */
class GastosViewModel(
    private val sesion: SessionRepository,
    /** Inyectable para que las pruebas fijen el día sin depender del reloj real. */
    private val ahora: () -> Long = { System.currentTimeMillis() },
) : ViewModel() {

    var rango by mutableStateOf(RangoDeGastos.Hoy)
        private set

    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    var cargando by mutableStateOf(true)
        private set

    /** Un error de red no borra lo que ya se mostró: se guarda acá y la lista sigue abajo. */
    var error by mutableStateOf<RbErrorCopy?>(null)
        private set

    var gastos by mutableStateOf<List<GastoDto>>(emptyList())
        private set

    // --- anotar ---------------------------------------------------------------

    var anotando by mutableStateOf(false)
        private set

    var montoNuevo by mutableStateOf("")
        private set
    var categoriaNueva by mutableStateOf("")
        private set
    var descripcionNueva by mutableStateOf("")
        private set
    var metodoDePagoNuevo by mutableStateOf("cash")
        private set

    var guardando by mutableStateOf(false)
        private set
    var errorDeGuardado by mutableStateOf<RbErrorCopy?>(null)
        private set
    var avisoGuardado by mutableStateOf<String?>(null)
        private set

    init {
        cargar()
    }

    fun cambiarRango(nuevo: RangoDeGastos) {
        if (rango == nuevo) return
        rango = nuevo
        cargar()
    }

    fun cargar() {
        val api = sesion.apiActiva()
        if (api == null) {
            cargando = false
            error = AppError.SesionExpirada().aCopy("lo que gastaste")
            return
        }

        cargando = true
        error = null

        viewModelScope.launch {
            moneda = MonedaRepository(api).resolver()
            recargar(api)
            cargando = false
        }
    }

    private suspend fun recargar(api: ApiFactory) {
        val (desde, hasta) = rangoActual()
        when (val r = GastosApi(api).gastos(desde, hasta, LIMITE_GASTOS)) {
            is Resultado.Ok -> {
                error = null
                gastos = r.valor.sortedByDescending { it.incurredAt }
            }

            is Resultado.Falla -> error = r.error.aCopy("lo que gastaste")
        }
    }

    private fun rangoActual(): Pair<String, String> = when (rango) {
        RangoDeGastos.Hoy -> {
            val comienzoDeHoy = DiaUtc.comienzoDelDia(ahora())
            DiaUtc.rfc3339(comienzoDeHoy) to DiaUtc.rfc3339(ahora())
        }

        RangoDeGastos.EsteMes -> {
            val comienzoDelMes = comienzoDelMesUtc(ahora())
            DiaUtc.rfc3339(comienzoDelMes) to DiaUtc.rfc3339(ahora())
        }
    }

    /**
     * Medianoche UTC del día 1 del mes en que cae [milisDeEpoca].
     *
     * [DiaUtc] no tiene esta función — es de la ola 28 y no se toca (ver
     * encargo) — así que se escribe acá, con el mismo algoritmo de Hinnant que
     * usa `DiaUtc.fechaCivil` (privado, no se puede llamar desde afuera) pero
     * en la dirección inversa: de `(año, mes, día)` a días de época.
     */
    private fun comienzoDelMesUtc(milisDeEpoca: Long): Long {
        val fecha = DiaUtc.rfc3339(milisDeEpoca)
        val anio = fecha.substring(0, 4).toLong()
        val mes = fecha.substring(5, 7).toLong()
        return diasDesdeCivil(anio, mes, 1L) * DiaUtc.MILIS_POR_DIA
    }

    /** `days_from_civil` de Howard Hinnant — el inverso del algoritmo que usa [DiaUtc]. */
    private fun diasDesdeCivil(anioOriginal: Long, mes: Long, dia: Long): Long {
        val anio = if (mes <= 2L) anioOriginal - 1L else anioOriginal
        val era = (if (anio >= 0L) anio else anio - 399L) / 400L
        val anioDeLaEra = anio - era * 400L
        val diaDelAnio = (153L * (if (mes > 2L) mes - 3L else mes + 9L) + 2L) / 5L + dia - 1L
        val diaDeLaEra = anioDeLaEra * 365L + anioDeLaEra / 4L - anioDeLaEra / 100L + diaDelAnio
        return era * 146_097L + diaDeLaEra - 719_468L
    }

    // --- anotar -----------------------------------------------------------------

    fun abrirFormulario() {
        montoNuevo = ""
        categoriaNueva = ""
        descripcionNueva = ""
        metodoDePagoNuevo = "cash"
        errorDeGuardado = null
        avisoGuardado = null
    }

    fun cambiarMontoNuevo(valor: String) {
        montoNuevo = valor
    }

    fun cambiarCategoriaNueva(valor: String) {
        categoriaNueva = valor
    }

    fun cambiarDescripcionNueva(valor: String) {
        descripcionNueva = valor
    }

    fun cambiarMetodoDePagoNuevo(valor: String) {
        metodoDePagoNuevo = valor
    }

    /**
     * Lo mismo que valida el server: descripción no vacía, categoría no
     * vacía, monto mayor que cero. Nada más — no se inventa una validación
     * de estructura que el server no tiene.
     */
    fun impedimentoParaAnotar(): String? {
        if (categoriaNueva.isBlank()) return "Escribe en qué gastaste"
        if (descripcionNueva.isBlank()) return "Escribe una descripción"
        val monto = Dinero.deTextoDeServidor(montoNuevo.replace(',', '.'))
        if (monto == null || monto <= Dinero.CERO) return "El monto tiene que ser mayor que cero"
        return null
    }

    fun anotar(feria: Boolean) {
        if (guardando) return
        val api = sesion.apiActiva() ?: return
        if (impedimentoParaAnotar() != null) return

        guardando = true
        errorDeGuardado = null

        viewModelScope.launch {
            val nuevo = NuevoGasto(
                category = categoriaNueva.trim(),
                description = descripcionNueva.trim(),
                amount = montoNuevo.trim().replace(',', '.'),
                paymentMethod = metodoDePagoNuevo,
            )

            when (val r = GastosApi(api).anotar(nuevo)) {
                is Resultado.Ok -> {
                    avisoGuardado = CopyGastos.avisoGuardado(feria)
                    recargar(api)
                }

                is Resultado.Falla -> errorDeGuardado = copyDeGuardadoFallido(r.error, feria)
            }

            guardando = false
        }
    }

    private fun copyDeGuardadoFallido(error: AppError, feria: Boolean): RbErrorCopy = when (error) {
        is AppError.SinPermiso -> RbErrorCopy(
            title = CopyGastos.tituloErrorAnotar(feria),
            message = CopyGastos.mensajeSinPermiso(feria),
            retryLabel = null,
        )

        else -> error.aCopy("anotar este gasto")
    }
}
