package cl.rutbusiness.core.offline

import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.pos.PosRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * El que saca las ventas de la cola y las manda, solo, cuando se puede.
 *
 * Vive lo que vive el proceso. No es un servicio de fondo ni un `WorkManager`:
 * el encargo es que la venta salga cuando vuelva la señal, y mientras la app
 * esté abierta esto lo hace sin traer una dependencia más. Si la app se cierra
 * con la cola llena, la cola queda en disco y sale al abrirla de nuevo — que es
 * lo que la dueña hace apenas nota que no hay sistema.
 *
 * **No reintenta en loop.** Duerme hasta que pase alguna de tres cosas: vuelve
 * el enlace, entra una venta nueva, o vence la espera del backoff. Un loop
 * apretado contra un teléfono sin señal quema batería y megas y no llega antes.
 */
class DespachadorDeVentas(
    private val cola: ColaDeVentas,
    private val conexion: ConexionConElNegocio,
    private val hayEnlace: StateFlow<Boolean>,
    private val haySesion: StateFlow<Boolean>,
    private val apiActiva: () -> ApiFactory?,
    private val reloj: () -> Long,
    /** Inyectable para que las pruebas no esperen de verdad. */
    private val dormir: suspend (Long) -> Unit = { delay(it) },
) {

    fun arrancar(scope: CoroutineScope) {
        scope.launch {
            cola.cargar()
            while (true) {
                esperarTurno()
                val venta = cola.proxima() ?: continue
                val api = apiActiva() ?: continue
                intentar(venta, api)
            }
        }
    }

    /**
     * Un solo intento, para el botón "Intentar ahora".
     *
     * Existe porque el reintento automático puede estar durmiendo cinco minutos
     * justo cuando la dueña ve el wifi volver, y hacerla esperar mirando el
     * cartel es exactamente la desconfianza que la cola tiene que evitar.
     */
    suspend fun intentarAhora() {
        cola.apurar()
        val venta = cola.proxima() ?: return
        val api = apiActiva() ?: return
        intentar(venta, api)
    }

    /**
     * Duerme hasta que valga la pena intentar.
     *
     * Cada espera se corta sola cuando cambia la condición que la motivaba: sin
     * enlace espera el enlace, sin ventas espera una venta, y con el backoff
     * corriendo espera el reloj **o** que entre una venta nueva — porque una
     * venta recién cobrada no puede quedarse cinco minutos esperando el turno
     * de otra que ya falló.
     */
    private suspend fun esperarTurno() {
        while (true) {
            if (!haySesion.value) {
                haySesion.first { it }
                continue
            }
            if (!hayEnlace.value) {
                hayEnlace.first { it }
                continue
            }
            val esperando = cola.ventas.value.filter { it.esperando }
            if (esperando.isEmpty()) {
                cola.ventas.first { lista -> lista.any { it.esperando } }
                continue
            }
            val falta = esperando.minOf { it.proximoIntentoEn } - reloj()
            if (falta <= 0L) return
            withTimeoutOrNull(falta) { cola.ventas.drop(1).first() }
        }
    }

    /**
     * Manda una venta.
     *
     * Sale con la **misma** clave de idempotencia con la que se encoló. Ése es
     * el candado contra el cobro doble: si el intento anterior alcanzó a llegar
     * al server y lo que se perdió fue la respuesta, este POST contesta 200 con
     * la orden que ya existe, no crea otra. Por eso un reintento acá es seguro
     * y no hay que preguntarle nada a la dueña.
     *
     * Qué se hace con cada falla:
     *
     *  - **nadie contestó** → se posterga y se reintenta. No pasó nada todavía,
     *    o pasó y no nos enteramos; las dos se arreglan reintentando.
     *  - **el server contestó un error suyo (5xx, 408, 429)** → se posterga: es
     *    un problema que se pasa solo.
     *  - **el server dijo que no (4xx)** → se marca rechazada y **no se borra**.
     *    Stock que no alcanzó, cliente que ya no está: son cosas que la dueña
     *    tiene que ver. Una venta que se borra sola es plata que nadie supo que
     *    se perdió.
     *  - **cualquier otra cosa** → se posterga. Ante una falla que no sabemos
     *    leer, la decisión segura con plata es reintentar, no descartar.
     */
    private suspend fun intentar(venta: VentaEnCola, api: ApiFactory) {
        when (val r = PosRepository(api).vender(venta.solicitud, venta.clave)) {
            is Resultado.Ok -> {
                conexion.anotarRespuesta()
                cola.quitar(venta.clave)
            }

            is Resultado.Falla -> when (val error = r.error) {
                is AppError.ServidorNoResponde -> {
                    conexion.anotarSilencio()
                    cola.postergar(venta.clave)
                }

                is AppError.ErrorDelServidor -> {
                    conexion.anotarRespuesta()
                    if (seArreglaSolo(error.status)) {
                        cola.postergar(venta.clave)
                    } else {
                        cola.rechazar(venta.clave, error.userMessage)
                    }
                }

                is AppError.SinPermiso -> {
                    conexion.anotarRespuesta()
                    cola.rechazar(venta.clave, error.userMessage)
                }

                else -> {
                    conexion.anotarRespuesta()
                    cola.postergar(venta.clave)
                }
            }
        }
    }

    private fun seArreglaSolo(status: Int): Boolean =
        status >= 500 || status == 408 || status == 425 || status == 429
}
