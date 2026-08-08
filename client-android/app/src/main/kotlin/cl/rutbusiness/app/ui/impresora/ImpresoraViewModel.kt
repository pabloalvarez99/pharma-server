package cl.rutbusiness.app.ui.impresora

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.money.MonedaRepository
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.launch

/**
 * En qué anda la impresión.
 *
 * `Fallida` **no** es un estado terminal ni bloqueante: la venta ya está
 * cobrada y guardada, así que desde acá siempre se puede reintentar o seguir
 * sin boleta. Ver [ImpresoraViewModel].
 */
sealed interface EstadoDeImpresion {
    /** Todavía no se intentó nada, o ya se cerró el asunto. */
    data object Reposo : EstadoDeImpresion

    /** Falta decir cuál es la impresora. Primera y única vez. */
    data class Eligiendo(val disponibles: List<ImpresoraConocida>) : EstadoDeImpresion

    /** Elegida la impresora, falta el ancho del rollo. */
    data class EligiendoAncho(val impresora: ImpresoraConocida) : EstadoDeImpresion

    data object Imprimiendo : EstadoDeImpresion

    data object Impresa : EstadoDeImpresion

    data class Fallida(val falla: FallaDeImpresion) : EstadoDeImpresion

    /** La dueña decidió seguir sin papel. Se deja de molestar. */
    data object SinBoleta : EstadoDeImpresion
}

/**
 * Imprimir la boleta de una venta que **ya se cobró**.
 *
 * Ésta es la regla que manda sobre todas las demás de este archivo: la plata ya
 * entró cuando esta clase entra en acción. Nada de lo que pase acá puede
 * revertir, bloquear ni ensuciar la venta. Por eso:
 *
 * - no hay ningún camino que llame al server para modificar la orden;
 * - toda falla termina en [EstadoDeImpresion.Fallida], que ofrece reintentar o
 *   seguir sin boleta, y nunca en una excepción que suba a la pantalla;
 * - el botón de "Cobrar otra venta" de la pantalla de comprobante no depende de
 *   nada de acá, así que una impresora rota no deja a la cajera encerrada.
 *
 * Un punto de venta que pierde una venta por un problema de papel es peor que
 * un punto de venta que no imprime.
 */
class ImpresoraViewModel(
    private val boletas: FuenteDeBoletas,
    private val enlace: EnlaceDeImpresora,
    private val preferencias: PreferenciasDeImpresora,
    /** `null` en las pruebas: sin sesión se usa la moneda por defecto. */
    private val sesion: SessionRepository? = null,
) : ViewModel() {

    var estado by mutableStateOf<EstadoDeImpresion>(EstadoDeImpresion.Reposo)
        private set

    /** La impresora recordada, si ya se configuró alguna vez. */
    var impresora by mutableStateOf(preferencias.leer())
        private set

    /** La moneda con la que se escriben los montos del papel. Sale del server. */
    var moneda by mutableStateOf(Moneda.POR_DEFECTO)
        private set

    /** Los permisos a pedir en este Android. Vacío hasta Android 11. */
    val permisosQuePedir: List<String> get() = enlace.permisosQuePedir

    /** Hay algo que reimprimir: se imprimió (o se intentó) alguna boleta antes. */
    val hayUltimaBoleta: Boolean get() = preferencias.leerUltimaBoleta() != null

    /** La venta que se está imprimiendo ahora. */
    private var ordenEnCurso: String? = null

    init {
        viewModelScope.launch {
            sesion?.apiActiva()?.let { moneda = MonedaRepository(it).resolver() }
        }
    }

    /**
     * Imprime la boleta de [ordenId].
     *
     * Si todavía no hay impresora configurada, abre la lista en vez de fallar:
     * la primera impresión del negocio es también la configuración, y pedirle a
     * la dueña que vaya a buscar una pantalla de ajustes en ese momento es
     * pedirle que no imprima.
     */
    fun imprimir(ordenId: String) {
        ordenEnCurso = ordenId
        preferencias.guardarUltimaBoleta(ordenId)

        val elegida = impresora
        if (elegida == null) {
            elegir()
            return
        }
        lanzarImpresion(ordenId, elegida)
    }

    /** Reimprime la última. Se pide todo el tiempo: papel cortado, copia para el cliente. */
    fun reimprimir() {
        val ultima = preferencias.leerUltimaBoleta()
        if (ultima == null) {
            estado = EstadoDeImpresion.Reposo
            return
        }
        imprimir(ultima)
    }

    /** Abre la lista de impresoras emparejadas. */
    fun elegir() {
        estado = when (val r = enlace.emparejadas()) {
            is Intento.Ok -> EstadoDeImpresion.Eligiendo(r.valor)
            is Intento.Fallo -> EstadoDeImpresion.Fallida(r.falla)
        }
    }

    fun elegirImpresora(elegida: ImpresoraConocida) {
        estado = EstadoDeImpresion.EligiendoAncho(elegida)
    }

    /**
     * Guarda la impresora y sigue con lo que se estaba haciendo.
     *
     * Si había una boleta esperando, se imprime sola: la dueña tocó "Imprimir",
     * no "Configurar", y no tiene por qué volver a tocar el botón.
     */
    fun elegirAncho(elegida: ImpresoraConocida, ancho: AnchoDePapel) {
        val guardada = ImpresoraElegida(
            direccion = elegida.direccion,
            nombre = elegida.nombre,
            ancho = ancho,
        )
        preferencias.guardar(guardada)
        impresora = guardada

        val pendiente = ordenEnCurso ?: preferencias.leerUltimaBoleta()
        if (pendiente != null) {
            lanzarImpresion(pendiente, guardada)
        } else {
            estado = EstadoDeImpresion.Reposo
        }
    }

    /** Vuelve a intentar exactamente lo mismo. Nunca vuelve a cobrar. */
    fun reintentar() {
        val orden = ordenEnCurso ?: preferencias.leerUltimaBoleta()
        val elegida = impresora
        when {
            orden == null -> estado = EstadoDeImpresion.Reposo
            elegida == null -> elegir()
            else -> lanzarImpresion(orden, elegida)
        }
    }

    /** "Sigo sin boleta". La venta ya está: acá no se deshace nada. */
    fun seguirSinBoleta() {
        estado = EstadoDeImpresion.SinBoleta
    }

    /** Cambiar de impresora, desde el estado que sea. */
    fun cambiarImpresora() {
        elegir()
    }

    fun cerrar() {
        estado = EstadoDeImpresion.Reposo
    }

    /** Se limpia al empezar una venta nueva, para no ofrecer imprimir la anterior. */
    fun olvidarVentaEnCurso() {
        ordenEnCurso = null
        estado = EstadoDeImpresion.Reposo
    }

    private fun lanzarImpresion(ordenId: String, elegida: ImpresoraElegida) {
        estado = EstadoDeImpresion.Imprimiendo
        viewModelScope.launch {
            val boleta = when (val r = boletas.boleta(ordenId)) {
                is Intento.Ok -> r.valor
                is Intento.Fallo -> {
                    estado = EstadoDeImpresion.Fallida(r.falla)
                    return@launch
                }
            }

            val bytes = bytesDeBoleta(
                boleta = boleta,
                moneda = moneda,
                ancho = elegida.ancho,
                desfaseMinutos = enlace.desfaseHorarioMinutos(),
            )

            estado = when (val r = enlace.imprimir(elegida, bytes)) {
                is Intento.Ok -> EstadoDeImpresion.Impresa
                is Intento.Fallo -> EstadoDeImpresion.Fallida(r.falla)
            }
        }
    }
}
