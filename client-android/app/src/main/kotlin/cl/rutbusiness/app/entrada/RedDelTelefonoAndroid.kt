package cl.rutbusiness.app.entrada

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import cl.rutbusiness.app.ui.entrada.RedDelTelefono

/**
 * Le pregunta a Android si hay red, sin salir a la calle a comprobarlo.
 *
 * **Se mira el transporte y no [NetworkCapabilities.NET_CAPABILITY_VALIDATED].**
 * "Validado" quiere decir que Android logró alcanzar internet por esa red, y en
 * este producto eso sería la pregunta equivocada: el caso normal es un wifi de
 * local con el computador del negocio adentro y sin salida a internet — o con
 * salida, pero caída ese rato. Ese wifi no está "validado" y sin embargo es
 * exactamente la red por la que la app tiene que hablar. Preguntar por validado
 * mandaría a la dueña a reclamarle a su proveedor de internet mientras su
 * sistema anda perfecto a tres metros.
 *
 * Lo que sí se descarta es el teléfono sin nada prendido, que es el caso que
 * vale la pena atajar antes de esperar diez segundos a un timeout.
 *
 * Nunca lanza: cualquier problema leyendo el estado de la red se responde con
 * `true`, o sea "seguí adelante e intentá". Un chequeo previo que se equivoca
 * hacia "no hay red" bloquearía a alguien que sí podía entrar, y eso es peor
 * que hacerle esperar el timeout.
 */
class RedDelTelefonoAndroid(context: Context) : RedDelTelefono {

    private val app = context.applicationContext

    override fun hayRed(): Boolean = runCatching {
        val manager = app.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
            ?: return@runCatching true

        val activa = manager.activeNetwork ?: return@runCatching false
        val capacidades = manager.getNetworkCapabilities(activa) ?: return@runCatching false

        TRANSPORTES.any { capacidades.hasTransport(it) }
    }.getOrDefault(true)

    private companion object {
        /**
         * Por dónde puede estar conectado el teléfono.
         *
         * Ethernet está en la lista porque el aparato objetivo puede ser una
         * tablet de mostrador con adaptador de red, y en ese caso no hay ni
         * wifi ni datos y la red igual existe.
         */
        val TRANSPORTES = intArrayOf(
            NetworkCapabilities.TRANSPORT_WIFI,
            NetworkCapabilities.TRANSPORT_CELLULAR,
            NetworkCapabilities.TRANSPORT_ETHERNET,
            NetworkCapabilities.TRANSPORT_VPN,
        )
    }
}
