package cl.rutbusiness.core.session

import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.ServerUrl
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

sealed interface EstadoSesion {
    /** Todavía leyendo disco. Dura milisegundos; la UI muestra un spinner. */
    data object Cargando : EstadoSesion

    /** Hay que entrar. Trae lo último que se tipeó para no pedirlo de nuevo. */
    data class SinSesion(
        val baseUrl: String?,
        val tenant: String?,
        val email: String?,
    ) : EstadoSesion

    /**
     * Hay token guardado. [me] llega `null` mientras no se haya podido
     * confirmar contra el server -- la app igual se usa, no se bloquea por no
     * tener señal en ese segundo.
     */
    data class Activa(
        val baseUrl: String,
        val me: Me?,
    ) : EstadoSesion
}

/**
 * Dueña de la sesión: dónde está el server, si hay token y si sigue vivo.
 *
 * Decisión de UX: al abrir la app, **si hay token guardado se entra directo**,
 * sin esperar a que el server confirme. La confirmación va en segundo plano y
 * solo un 401 explícito bota la sesión. Hacerlo al revés significa que el
 * teléfono sin señal manda al usuario a tipear su contraseña de nuevo, que es
 * exactamente lo que no queremos con datos móviles intermitentes.
 */
class SessionRepository(
    private val almacenamiento: AlmacenamientoPlataforma,
) {
    private val _estado = MutableStateFlow<EstadoSesion>(EstadoSesion.Cargando)
    val estado: StateFlow<EstadoSesion> = _estado.asStateFlow()

    private var token: String? = null
    private var factory: ApiFactory? = null

    /** Cliente HTTP apuntando a [baseUrl], reusado mientras la URL no cambie. */
    fun apiPara(baseUrl: String): ApiFactory {
        val actual = factory
        if (actual != null && actual.baseUrl == baseUrl) return actual
        actual?.close()
        return ApiFactory(baseUrl) { token }.also { factory = it }
    }

    /** Cliente de la sesión activa, o `null` si todavía no hay sesión. */
    fun apiActiva(): ApiFactory? =
        (_estado.value as? EstadoSesion.Activa)?.let { apiPara(it.baseUrl) }

    /** Se llama una vez al arrancar la app. */
    suspend fun restaurar() {
        val baseUrl = almacenamiento.preferencias.leerBaseUrl()
        val guardado = almacenamiento.tokens.leer()
        token = guardado

        _estado.value = if (baseUrl != null && guardado != null) {
            EstadoSesion.Activa(baseUrl = baseUrl, me = null)
        } else {
            EstadoSesion.SinSesion(
                baseUrl = baseUrl,
                tenant = almacenamiento.preferencias.leerUltimoTenant(),
                email = almacenamiento.preferencias.leerUltimoEmail(),
            )
        }
    }

    /**
     * Confirma contra el server que el token guardado sigue sirviendo. Solo un
     * 401 cierra la sesión; que el server esté apagado no la toca.
     */
    suspend fun confirmarSesion() {
        val activa = _estado.value as? EstadoSesion.Activa ?: return
        when (val r = AuthApi(apiPara(activa.baseUrl)).me()) {
            is Resultado.Ok -> _estado.value = activa.copy(me = r.valor)
            is Resultado.Falla -> if (r.error is AppError.SesionExpirada) salir()
        }
    }

    /**
     * Entra al server. Devuelve [Resultado.Falla] con el mensaje ya listo para
     * mostrar; la pantalla de login no interpreta códigos HTTP.
     */
    suspend fun entrar(
        baseUrlCruda: String,
        tenant: String,
        email: String,
        password: String,
    ): Resultado<Unit> {
        val baseUrl = ServerUrl.normalizar(baseUrlCruda)
            ?: return Resultado.Falla(AppError.DireccionInvalida())

        // Sin token para el intento de login: es el endpoint que lo emite.
        token = null
        return when (val r = AuthApi(apiPara(baseUrl)).login(tenant, email, password)) {
            is Resultado.Falla -> r
            is Resultado.Ok -> {
                token = r.valor.token
                almacenamiento.tokens.guardar(r.valor.token)
                almacenamiento.preferencias.guardarBaseUrl(baseUrl)
                almacenamiento.preferencias.guardarUltimoAcceso(tenant.trim(), email.trim().lowercase())
                _estado.value = EstadoSesion.Activa(baseUrl = baseUrl, me = null)
                Resultado.Ok(Unit)
            }
        }
    }

    suspend fun salir() {
        token = null
        almacenamiento.tokens.borrar()
        _estado.value = EstadoSesion.SinSesion(
            baseUrl = almacenamiento.preferencias.leerBaseUrl(),
            tenant = almacenamiento.preferencias.leerUltimoTenant(),
            email = almacenamiento.preferencias.leerUltimoEmail(),
        )
    }
}
