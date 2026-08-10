package cl.rutbusiness.core.session

import android.content.Context
import cl.rutbusiness.core.offline.AlmacenDeArchivos
import cl.rutbusiness.core.offline.AlmacenDeBloques
import java.io.File

actual class AlmacenamientoPlataforma private constructor(
    context: Context,
    tokenStore: TokenStore?,
) {
    constructor(context: Context) : this(context, null)

    private val app = context.applicationContext

    actual val tokens: TokenStore = tokenStore ?: TokenStoreAndroid(app)
    actual val preferencias: PreferenciasServidor = PreferenciasServidorAndroid(app)

    /**
     * `filesDir` y no `cacheDir`: la cola de ventas vive acá, y `cacheDir` es
     * lo primero que Android borra cuando el teléfono se queda sin espacio.
     * Perder una venta cobrada porque el sistema hizo limpieza sería el peor
     * bug posible de este módulo.
     */
    actual val bloques: AlmacenDeBloques = AlmacenDeArchivos(File(app.filesDir, "rb-offline"))

    companion object {
        /**
         * Igual que el de producción, pero con otro guardián del token.
         *
         * Existe por una razón concreta: [TokenStoreAndroid] cifra contra
         * `AndroidKeyStore`, que **no existe** fuera de un aparato. Robolectric
         * tira `NoSuchAlgorithmException` al primer `guardar()`, así que sin
         * esta costura no se puede montar una sesión activa en la JVM — y por
         * lo tanto no se puede probar en la JVM **ningún** `ViewModel` que
         * hable con el server. Eso es lo que dejó pasar que la pantalla de
         * cobro no cerrara la sesión ante un 401 (2026-08-09).
         *
         * No relaja nada en producción: el constructor público sigue siendo el
         * de siempre y sigue cifrando. Sólo una prueba puede pasar otra cosa.
         */
        fun conTokensDePrueba(context: Context, tokens: TokenStore): AlmacenamientoPlataforma =
            AlmacenamientoPlataforma(context, tokens)
    }
}
