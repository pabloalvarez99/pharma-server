package cl.rutbusiness.core.session

import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.AvisoDeSesion
import cl.rutbusiness.core.net.ReporteDeRed
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.ServerUrl
import cl.rutbusiness.core.offline.CacheDeLecturas
import cl.rutbusiness.core.offline.ClaveDeCache
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
    /**
     * Quién se entera de si el server contesta. Se le pasa a cada [ApiFactory]
     * que se arme acá, que son todas las de la app: así el cartel de "sin
     * conexión" lo prende cualquier llamada, no sólo las de una pantalla.
     */
    private val red: ReporteDeRed = ReporteDeRed.Nulo,
) {
    private val _estado = MutableStateFlow<EstadoSesion>(EstadoSesion.Cargando)
    val estado: StateFlow<EstadoSesion> = _estado.asStateFlow()

    private var token: String? = null
    private var factory: ApiFactory? = null
    private var restaurada = false

    /**
     * Cliente HTTP apuntando a [baseUrl], reusado mientras la URL no cambie.
     *
     * Acá se cablea el 401: **toda** llamada que salga de esta sesión cierra la
     * sesión sola si el server dice que el token no sirve. Es el único lugar
     * donde hay que acordarse, y por eso el único donde se puede olvidar.
     */
    fun apiPara(baseUrl: String): ApiFactory {
        val actual = factory
        if (actual != null && actual.baseUrl == baseUrl) return actual
        actual?.close()
        return ApiFactory(baseUrl, red, sesionVencida = alVencer) { token }
            .also { factory = it }
    }

    /**
     * Sólo se cierra lo que estaba abierto. Un 401 del propio login no es una
     * sesión que venció -es una contraseña que no era-, y ahí `salir()` sería
     * borrar el token de nadie y pisar el estado de la pantalla de entrada.
     */
    private val alVencer = object : AvisoDeSesion {
        override suspend fun vencio() {
            if (_estado.value is EstadoSesion.Activa) salir()
        }
    }

    /**
     * Disco de la plataforma, para el caché de lecturas y la cola de ventas.
     *
     * Lo expone la sesión porque es la que ya tiene el almacenamiento y la que
     * sabe contra qué server estamos: las dos cosas que hacen falta para que el
     * caché de un negocio no se le muestre a otro.
     */
    val disco: cl.rutbusiness.core.offline.AlmacenDeBloques get() = almacenamiento.bloques

    /** Cliente de la sesión activa, o `null` si todavía no hay sesión. */
    fun apiActiva(): ApiFactory? =
        (_estado.value as? EstadoSesion.Activa)?.let { apiPara(it.baseUrl) }

    /**
     * Slug del negocio del último login (para tarjeta de rescate / QR).
     * No es secreto: es el mismo nombre corto que tipeó en el login.
     */
    suspend fun ultimoTenant(): String? =
        almacenamiento.preferencias.leerUltimoTenant()

    /**
     * Se llama una vez al arrancar la app, y **antes** del primer frame: quien
     * la dispara es `Application.onCreate`, no la UI.
     *
     * Por qué importa el orden: la lectura de disco tarda ~40 ms en el aparato
     * lento. Colgada de un `LaunchedEffect` corría *después* de que Compose ya
     * hubiera compuesto y dibujado el estado [EstadoSesion.Cargando], o sea un
     * frame entero de spinner que se tiraba a la basura 50 ms más tarde.
     * Disparada desde el arranque del proceso, esa lectura se solapa con la
     * inicialización de Compose y en el aparato lento el spinner ya no alcanza
     * a aparecer: el primer frame es directamente el login o el catálogo.
     *
     * Idempotente: si algo la llama dos veces, la segunda no hace daño, pero
     * tampoco vuelve a leer disco de gusto.
     */
    suspend fun restaurar() {
        if (restaurada) return
        restaurada = true

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
     *
     * El 401 no se maneja acá: esta llamada sale por [apiPara], igual que todas
     * las demás, y ahí ya está cableado. Repetirlo en este `when` sería la
     * misma línea escrita dos veces, que es como empezó el problema.
     */
    suspend fun confirmarSesion() {
        val activa = _estado.value as? EstadoSesion.Activa ?: return
        when (val r = AuthApi(apiPara(activa.baseUrl)).me()) {
            is Resultado.Ok -> _estado.value = activa.copy(me = r.valor)
            is Resultado.Falla -> Unit
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

    /**
     * Entra con `id_token` de Google (ADR-0022).
     *
     * Misma activación de sesión que [entrar]; el server debe verificar JWKS.
     * Hoy suele fallar con 501 hasta que ops cablee OAuth. **No** loguear
     * [idToken].
     */
    suspend fun entrarConGoogle(
        baseUrlCruda: String,
        idToken: String,
        tenant: String? = null,
        emailHint: String? = null,
    ): Resultado<Unit> {
        val baseUrl = ServerUrl.normalizar(baseUrlCruda)
            ?: return Resultado.Falla(AppError.DireccionInvalida())
        if (idToken.isBlank()) {
            return Resultado.Falla(AppError.Inesperado("google id_token vacío"))
        }
        token = null
        return when (
            val r = AuthApi(apiPara(baseUrl)).loginConGoogle(
                idToken = idToken,
                tenant = tenant,
            )
        ) {
            is Resultado.Falla -> r
            is Resultado.Ok -> {
                token = r.valor.token
                almacenamiento.tokens.guardar(r.valor.token)
                almacenamiento.preferencias.guardarBaseUrl(baseUrl)
                val t = tenant?.trim().orEmpty()
                val e = emailHint?.trim()?.lowercase().orEmpty()
                if (t.isNotEmpty() || e.isNotEmpty()) {
                    almacenamiento.preferencias.guardarUltimoAcceso(t, e)
                }
                _estado.value = EstadoSesion.Activa(baseUrl = baseUrl, me = null)
                Resultado.Ok(Unit)
            }
        }
    }

    /**
     * Cierra la sesión.
     *
     * Se lleva el caché de lecturas: son los productos, los deudores y las
     * ventas de un negocio, y quien entre después puede ser otra persona. La
     * **cola de ventas no se toca** — esas ventas ya se cobraron y salen apenas
     * alguien vuelva a entrar. Borrarlas acá sería perder plata cobrada porque
     * alguien tocó "Salir".
     */
    suspend fun salir() {
        (_estado.value as? EstadoSesion.Activa)?.baseUrl?.let { baseUrl ->
            CacheDeLecturas(almacenamiento.bloques) { 0L }
                .olvidar(ClaveDeCache.todas(baseUrl))
        }
        token = null
        almacenamiento.tokens.borrar()
        _estado.value = EstadoSesion.SinSesion(
            baseUrl = almacenamiento.preferencias.leerBaseUrl(),
            tenant = almacenamiento.preferencias.leerUltimoTenant(),
            email = almacenamiento.preferencias.leerUltimoEmail(),
        )
    }
}
