package cl.rutbusiness.app.ui.impresora

import android.content.Context

/**
 * Dónde queda anotada la impresora del negocio, en este teléfono.
 *
 * `SharedPreferences` y no DataStore: son cuatro cadenas que hay que poder leer
 * **sin suspender**, porque la pantalla de cobro pregunta "¿hay impresora
 * configurada?" mientras compone el primer frame. DataStore obligaría a que esa
 * respuesta llegue un frame después, y el botón de imprimir aparecería
 * parpadeando justo cuando la cajera va a tocarlo. Además evita sumar una
 * dependencia al arranque en frío, que es lo que este proyecto cuida.
 *
 * Acá **no** hay ningún secreto: una MAC de impresora y un ancho de papel. El
 * token sigue cifrado en su propio store.
 */
class PreferenciasDeImpresoraAndroid(context: Context) : PreferenciasDeImpresora {

    private val prefs = context.applicationContext
        .getSharedPreferences("rutbusiness_impresora", Context.MODE_PRIVATE)

    override fun leer(): ImpresoraElegida? {
        val direccion = prefs.getString(DIRECCION, null)?.takeIf { it.isNotBlank() } ?: return null
        val nombre = prefs.getString(NOMBRE, null)?.takeIf { it.isNotBlank() } ?: direccion
        // Un ancho guardado que ya no exista (renombrado, versión vieja) cae en
        // 58 mm, que es el rollo del negocio chico y el que más se vende.
        val ancho = AnchoDePapel.entries
            .firstOrNull { it.name == prefs.getString(ANCHO, null) }
            ?: AnchoDePapel.Mm58
        return ImpresoraElegida(direccion = direccion, nombre = nombre, ancho = ancho)
    }

    override fun guardar(impresora: ImpresoraElegida) {
        prefs.edit()
            .putString(DIRECCION, impresora.direccion)
            .putString(NOMBRE, impresora.nombre)
            .putString(ANCHO, impresora.ancho.name)
            .apply()
    }

    override fun olvidar() {
        prefs.edit().remove(DIRECCION).remove(NOMBRE).remove(ANCHO).apply()
    }

    /**
     * La última venta que se mandó a imprimir.
     *
     * Se guarda el id de la orden y no la boleta armada: reimprimir vuelve a
     * pedirle el comprobante al server, así que el papel dice exactamente lo
     * mismo que la venta guardada aunque la app se haya cerrado en el medio.
     */
    override fun leerUltimaBoleta(): String? =
        prefs.getString(ULTIMA_BOLETA, null)?.takeIf { it.isNotBlank() }

    override fun guardarUltimaBoleta(ordenId: String) {
        prefs.edit().putString(ULTIMA_BOLETA, ordenId).apply()
    }

    private companion object {
        const val DIRECCION = "direccion"
        const val NOMBRE = "nombre"
        const val ANCHO = "ancho"
        const val ULTIMA_BOLETA = "ultima_boleta"
    }
}
