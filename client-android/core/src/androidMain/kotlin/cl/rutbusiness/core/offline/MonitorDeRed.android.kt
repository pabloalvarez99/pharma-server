package cl.rutbusiness.core.offline

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * El enlace según Android.
 *
 * Se usa `registerNetworkCallback` con un `NetworkRequest` y **no**
 * `registerDefaultNetworkCallback`, que es más corto pero existe desde API 24
 * y el piso de la app es 23. El teléfono viejo es justamente el que este
 * encargo tiene que servir, así que la rama cómoda no está disponible.
 *
 * El callback puede avisar por varias redes a la vez (wifi que se cae mientras
 * los datos móviles siguen). Por eso no se lleva un booleano sino el conjunto
 * de redes vivas: con `onLost` de una sola, decir "sin conexión" cuando la otra
 * sigue andando mandaría a la dueña a revisar una señal que sí tiene.
 */
actual class MonitorDeRed(context: Context) {

    private val manager =
        context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    private val vivas = mutableSetOf<Network>()

    private val _hayEnlace = MutableStateFlow(hayAlgunaRedAhora())
    actual val hayEnlace: StateFlow<Boolean> = _hayEnlace.asStateFlow()

    private val callback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            synchronized(vivas) {
                vivas.add(network)
                _hayEnlace.value = vivas.isNotEmpty()
            }
        }

        override fun onLost(network: Network) {
            synchronized(vivas) {
                vivas.remove(network)
                _hayEnlace.value = vivas.isNotEmpty()
            }
        }
    }

    init {
        val pedido = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()
        // Un teléfono sin permiso de red o un fabricante con el servicio
        // capado no puede tumbar el arranque de la app: si el registro falla,
        // se queda con lo que dijo la lectura inicial y las llamadas reales
        // siguen siendo la fuente de verdad.
        runCatching { manager.registerNetworkCallback(pedido, callback) }
    }

    actual fun cerrar() {
        runCatching { manager.unregisterNetworkCallback(callback) }
    }

    /**
     * Estado inicial, antes de que llegue el primer callback.
     *
     * `activeNetwork` + `getNetworkCapabilities` existen desde API 23, así que
     * no hace falta la rama con `activeNetworkInfo` deprecado.
     */
    private fun hayAlgunaRedAhora(): Boolean = runCatching {
        val red = manager.activeNetwork ?: return@runCatching false
        val capacidades = manager.getNetworkCapabilities(red) ?: return@runCatching false
        capacidades.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
    }.getOrDefault(false)
}
