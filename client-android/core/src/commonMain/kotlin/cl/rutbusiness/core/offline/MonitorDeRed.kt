package cl.rutbusiness.core.offline

import kotlinx.coroutines.flow.StateFlow

/**
 * Lo que el sistema operativo dice del enlace: ¿hay wifi o datos prendidos?
 *
 * Tercer y último punto `expect/actual` del proyecto, por lo mismo que los
 * otros dos: preguntar esto es API de plataforma. En Android es
 * `ConnectivityManager`; en iOS será `NWPathMonitor`. Ninguna pantalla se
 * entera.
 *
 * Ojo con lo que **no** promete: que el sistema diga "hay wifi" no significa
 * que el server del negocio conteste. El PC puede estar apagado, o el wifi
 * puede ser el del vecino sin salida. Por eso esto es una de las dos entradas
 * de [ConexionConElNegocio] y no la respuesta.
 */
expect class MonitorDeRed {
    /** `true` mientras el aparato tenga algún enlace de datos prendido. */
    val hayEnlace: StateFlow<Boolean>

    /** Suelta el callback del sistema. Se llama cuando muere el proceso. */
    fun cerrar()
}
