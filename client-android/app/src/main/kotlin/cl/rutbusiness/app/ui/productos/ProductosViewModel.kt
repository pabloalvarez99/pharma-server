package cl.rutbusiness.app.ui.productos

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.catalog.ProductRepository
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.launch

sealed interface EstadoProductos {
    data object Cargando : EstadoProductos
    data class Listo(val productos: List<ProductDto>) : EstadoProductos
    data class Error(val mensaje: String) : EstadoProductos
}

class ProductosViewModel(
    private val sesion: SessionRepository,
) : ViewModel() {

    var estado by mutableStateOf<EstadoProductos>(EstadoProductos.Cargando)
        private set

    init {
        cargar()
        // El token guardado se confirma acá, en paralelo: si venció, la sesión
        // se cierra sola y el usuario cae en el login sin haber tocado nada.
        viewModelScope.launch { sesion.confirmarSesion() }
    }

    fun cargar() {
        estado = EstadoProductos.Cargando
        viewModelScope.launch {
            val api = sesion.apiActiva()
            if (api == null) {
                estado = EstadoProductos.Error("Tu sesión venció. Vuelve a entrar para seguir.")
                return@launch
            }
            estado = when (val r = ProductRepository(api).listar()) {
                is Resultado.Ok -> EstadoProductos.Listo(r.valor)
                is Resultado.Falla -> EstadoProductos.Error(r.error.userMessage)
            }
        }
    }

    fun salir() {
        viewModelScope.launch { sesion.salir() }
    }
}
