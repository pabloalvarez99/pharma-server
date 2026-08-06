package cl.rutbusiness.app

import android.content.Context
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository

/**
 * El grafo de dependencias completo, a mano.
 *
 * Sin Hilt ni Koin: son tres objetos. Un framework de inyección acá solo suma
 * clases que cargar en el arranque, y el arranque en frío del teléfono lento es
 * justamente lo que estamos cuidando.
 */
class AppContainer(context: Context) {
    private val almacenamiento = AlmacenamientoPlataforma(context.applicationContext)
    val sesion = SessionRepository(almacenamiento)
}
