package cl.rutbusiness.app.ui.login

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.ServerUrl
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import kotlinx.coroutines.launch

class LoginViewModel(
    private val sesion: SessionRepository,
    inicial: EstadoSesion.SinSesion,
) : ViewModel() {

    var url by mutableStateOf(inicial.baseUrl ?: ServerUrl.EMULADOR)
        private set
    var sucursal by mutableStateOf(inicial.tenant.orEmpty())
        private set
    var email by mutableStateOf(inicial.email.orEmpty())
        private set
    var password by mutableStateOf("")
        private set
    var enviando by mutableStateOf(false)
        private set
    var error by mutableStateOf<String?>(null)
        private set

    /** Todo lleno = se puede intentar. La validación fina la hace el server. */
    val puedeEntrar: Boolean
        get() = !enviando && url.isNotBlank() && sucursal.isNotBlank() &&
            email.isNotBlank() && password.isNotBlank()

    fun cambiarUrl(valor: String) { url = valor; error = null }
    fun cambiarSucursal(valor: String) { sucursal = valor; error = null }
    fun cambiarEmail(valor: String) { email = valor; error = null }
    fun cambiarPassword(valor: String) { password = valor; error = null }

    fun entrar() {
        if (!puedeEntrar) return
        enviando = true
        error = null
        viewModelScope.launch {
            when (val r = sesion.entrar(url, sucursal, email, password)) {
                is Resultado.Ok -> {
                    // La contraseña no sobrevive al login: ni en memoria.
                    password = ""
                    enviando = false
                }
                is Resultado.Falla -> {
                    error = r.error.userMessage
                    enviando = false
                }
            }
        }
    }
}
