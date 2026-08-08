package cl.rutbusiness.core.session

import android.content.Context
import cl.rutbusiness.core.offline.AlmacenDeArchivos
import cl.rutbusiness.core.offline.AlmacenDeBloques
import java.io.File

actual class AlmacenamientoPlataforma(context: Context) {
    private val app = context.applicationContext

    actual val tokens: TokenStore = TokenStoreAndroid(app)
    actual val preferencias: PreferenciasServidor = PreferenciasServidorAndroid(app)

    /**
     * `filesDir` y no `cacheDir`: la cola de ventas vive acá, y `cacheDir` es
     * lo primero que Android borra cuando el teléfono se queda sin espacio.
     * Perder una venta cobrada porque el sistema hizo limpieza sería el peor
     * bug posible de este módulo.
     */
    actual val bloques: AlmacenDeBloques = AlmacenDeArchivos(File(app.filesDir, "rb-offline"))
}
