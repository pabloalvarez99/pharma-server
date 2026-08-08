package cl.rutbusiness.core.offline

import cl.rutbusiness.core.net.ReporteDeRed
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Si el teléfono está llegando o no al sistema del negocio.
 *
 * **No es lo mismo que "hay wifi".** El server vive en el PC del local, así que
 * hay dos formas distintas de quedarse sin sistema y desde la pantalla se ven
 * igual: el teléfono se quedó sin señal, o el PC se apagó. Una sola de las dos
 * la sabe el sistema operativo. La otra sólo se sabe cuando una llamada real
 * falla.
 *
 * Entonces el estado sale de las dos entradas:
 *
 *  - [MonitorDeRed] dice que no hay enlace → **sin conexión**, y es seguro:
 *    no hay a dónde mandar nada.
 *  - Hay enlace, pero la última llamada murió sin respuesta → **sin conexión**
 *    igual. Mentirle a la dueña con "andando" mientras nada llega sería peor
 *    que no mostrar nada.
 *  - Hay enlace y algo contestó → **en línea**.
 *
 * El reintento de la cola es lo que sondea: cada intento que sale bien vuelve
 * a poner el cartel en verde sin que nadie tenga que tocar "reintentar".
 *
 * Un error del server (stock insuficiente, rol que no alcanza) **no** es estar
 * sin conexión: el sistema contestó. Sólo [anotarSilencio] cambia el estado.
 */
class ConexionConElNegocio(
    /**
     * Lo que dice el sistema operativo del enlace. Se recibe como flujo y no
     * como [MonitorDeRed] a propósito: así esta clase -que es donde vive la
     * regla- se puede probar sin levantar Android.
     */
    private val hayEnlace: StateFlow<Boolean>,
) : ReporteDeRed {

    private val _hayConexion = MutableStateFlow(hayEnlace.value)

    /** `false` cuando no se está llegando al sistema del negocio. */
    val hayConexion: StateFlow<Boolean> = _hayConexion.asStateFlow()

    /** El sistema contestó algo, aunque haya sido un error suyo. */
    override fun anotarRespuesta() {
        if (hayEnlace.value) _hayConexion.value = true
    }

    /** La llamada se fue y no volvió: timeout, DNS, conexión rechazada. */
    override fun anotarSilencio() {
        _hayConexion.value = false
    }

    /**
     * Empieza a escuchar al sistema operativo.
     *
     * Perder el enlace se refleja al toque, porque es certeza: sin enlace no
     * hay a dónde mandar nada.
     *
     * **Recuperarlo no alcanza.** Que el wifi vuelva a asociarse no dice nada
     * sobre si el PC del negocio está prendido, y pintar el cartel de verde ahí
     * lo haría parpadear cada vez que el teléfono salta de una antena a otra.
     * Entonces se pregunta: [sonda] es una llamada chica y de verdad al server
     * (`GET /health/ready`, sin token y sin body). Una por evento de red, no un
     * sondeo periódico — los datos móviles se pagan por mega.
     */
    fun escuchar(scope: CoroutineScope, sonda: suspend () -> Boolean) {
        scope.launch {
            var anterior = hayEnlace.value
            hayEnlace.collect { enlace ->
                val volvio = enlace && !anterior
                anterior = enlace
                when {
                    !enlace -> _hayConexion.value = false
                    volvio -> if (sonda()) anotarRespuesta() else anotarSilencio()
                }
            }
        }
    }
}
