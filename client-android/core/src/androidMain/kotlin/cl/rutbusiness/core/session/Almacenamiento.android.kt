package cl.rutbusiness.core.session

import android.content.Context

actual class AlmacenamientoPlataforma(context: Context) {
    actual val tokens: TokenStore = TokenStoreAndroid(context)
    actual val preferencias: PreferenciasServidor = PreferenciasServidorAndroid(context)
}
