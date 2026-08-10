package cl.rutbusiness.core.rubro

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Carga y cachea el [RubroPack] del tenant.
 *
 * Mismo contrato que `client/src/vertical.ts` `loadRubroPack`:
 * - Preferir el server (`GET /api/v1/rubro-pack`).
 * - Si falla (LAN caída, server viejo), fallback local - **nunca tira**.
 * - Al cerrar sesión se limpia el cache.
 *
 * La UI lee [pack] y no vuelve a pegarle al server en cada recomposición.
 */
class RubroPackRepository {
    private val _pack = MutableStateFlow(PACK_OTRO)
    val pack: StateFlow<RubroPack> = _pack.asStateFlow()

    /** Valor actual sin suscribirse (ViewModels / helpers). */
    val actual: RubroPack get() = _pack.value

    /**
     * Trae el pack del server y lo deja en [pack].
     *
     * @param fallbackRubro si el GET falla, se usa el pack local de este
     *   vertical (si se conoce). Por defecto `otro`.
     */
    suspend fun cargar(api: ApiFactory, fallbackRubro: String? = null): RubroPack {
        val resultado = llamar(api) {
            api.http.get("${api.baseUrl}/api/v1/rubro-pack")
                .exigirExito(api.baseUrl)
                .body<RubroPack>()
        }
        val resuelto = when (resultado) {
            is Resultado.Ok -> resultado.valor
            is Resultado.Falla -> PacksLocales.de(fallbackRubro)
        }
        _pack.value = resuelto
        return resuelto
    }

    /** Fuerza un pack (tests, o vertical elegido en alta sin server). */
    fun usarLocal(rubro: String?) {
        _pack.value = PacksLocales.de(rubro)
    }

    fun limpiar() {
        _pack.value = PACK_OTRO
    }
}
