package cl.rutbusiness.app.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import cl.rutbusiness.app.ui.assist.AssistRoute
import cl.rutbusiness.app.ui.common.PantallaCargando
import cl.rutbusiness.app.ui.login.LoginRoute
import cl.rutbusiness.app.ui.productos.ProductosRoute
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Raíz de la UI. Kotlin común: ni un solo `import android.*` de acá para abajo.
 *
 * La navegación la decide el estado de la sesión, no una pila de rutas: con dos
 * destinos, meter `navigation-compose` sería una dependencia más que cargar en
 * el arranque para no resolver ningún problema real.
 *
 * TODO(design-system): envolver en el tema real cuando aterrice `ui/theme/`.
 * Por ahora los defaults de Material 3: feos, pero honestos.
 */
@Composable
fun RutBusinessApp(sesion: SessionRepository) {
    val estado by sesion.estado.collectAsState()

    LaunchedEffect(Unit) { sesion.restaurar() }

    MaterialTheme {
        Surface(
            modifier = Modifier.fillMaxSize(),
            color = MaterialTheme.colorScheme.background,
        ) {
            when (val actual = estado) {
                EstadoSesion.Cargando -> PantallaCargando("Abriendo RutBusiness…")
                is EstadoSesion.SinSesion -> LoginRoute(sesion = sesion, estado = actual)
                is EstadoSesion.Activa -> ConSesion(sesion = sesion, estado = actual)
            }
        }
    }
}

/**
 * Qué ve la dueña una vez adentro.
 *
 * Arranca en el agente, no en el catálogo: según el founder ésa es LA interfaz
 * del producto — la dueña no navega menús, le pide las cosas. El catálogo
 * queda a un toque para cuando hablar no alcanza.
 *
 * El agente va envuelto en `RbTheme` y el resto sigue en `MaterialTheme`: el
 * design system todavía no se adoptó en login ni en productos, y cambiarles el
 * tema desde acá les movería los colores sin que nadie lo haya revisado.
 */
@Composable
private fun ConSesion(sesion: SessionRepository, estado: EstadoSesion.Activa) {
    var enCatalogo by rememberSaveable { mutableStateOf(false) }

    if (enCatalogo) {
        ProductosRoute(sesion = sesion, estado = estado)
    } else {
        RbTheme {
            AssistRoute(sesion = sesion, onVerCatalogo = { enCatalogo = true })
        }
    }
}
