package cl.rutbusiness.app.ui.assist

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.compose.viewModel
import cl.rutbusiness.core.session.SessionRepository

/**
 * Engancha la pantalla del agente con la sesión.
 *
 * El `ViewModel` se pide con `viewModel()` para que la conversación sobreviva
 * lo que pase con la pantalla: girar el teléfono, que el sistema recomponga,
 * ir y volver. Perder lo conversado porque alguien rotó el aparato sería
 * exactamente lo que dice el encargo que no puede pasar.
 */
@Composable
fun AssistRoute(
    sesion: SessionRepository,
    onVerCatalogo: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val vm: AssistViewModel = viewModel(factory = AssistViewModelFactory(sesion))
    AssistScreen(vm = vm, onVerCatalogo = onVerCatalogo, modifier = modifier)
}

/**
 * Fábrica a mano, igual que el resto del grafo de dependencias de esta app: son
 * dos objetos y un framework de inyección solo sumaría clases que cargar en el
 * arranque, que es justo lo que se está cuidando en el teléfono lento.
 */
private class AssistViewModelFactory(
    private val sesion: SessionRepository,
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T =
        AssistViewModel(sesion) as T
}
