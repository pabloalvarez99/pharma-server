package cl.rutbusiness.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import cl.rutbusiness.app.ui.cobrar.CobrarRoute
import cl.rutbusiness.app.ui.login.LoginRoute
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Raíz de la UI. Kotlin común: ni un solo `import android.*` de acá para abajo.
 *
 * [RbTheme] envuelve todo una sola vez. Debajo nadie lee `MaterialTheme`: los
 * tokens salen del design system, que es el que carga las garantías de
 * contraste y de tamaño táctil.
 *
 * La navegación la decide el estado de la sesión, no una pila de rutas: con dos
 * destinos, meter `navigation-compose` sería una dependencia más que cargar en
 * el arranque para no resolver ningún problema real.
 */
@Composable
fun RutBusinessApp(sesion: SessionRepository) {
    val estado by sesion.estado.collectAsState()

    RbTheme {
        when (val actual = estado) {
            EstadoSesion.Cargando -> RbLoadingState(label = "Abriendo RutBusiness...")
            is EstadoSesion.SinSesion -> LoginRoute(sesion = sesion, estado = actual)
            is EstadoSesion.Activa -> CobrarRoute(sesion = sesion, estado = actual)
        }
    }
}
