package cl.rutbusiness.core.session

/**
 * Identidad Google (ADR-0022) - contrato de cliente **sin secretos**.
 *
 * Producto: en feria el feriante entra con su cuenta de Google (sin PC). El
 * core offline sigue sin red una vez autenticado (ADR-0005).
 *
 * **Qué no va acá:**
 * - Client ID / OAuth client secret (viven en `local.properties` gitignored
 *   o en la consola de Google Cloud; se leen solo en el build real).
 * - Verificación de `id_token` (eso es del server con las claves públicas
 *   de Google).
 * - Logging del token (nunca).
 *
 * El stub [IdentidadGoogleNoCableada] deja la UI y los tests vivos sin
 * pretender OAuth. Cuando haya client id de consola, un impl Android
 * (Credential Manager / Play Services) reemplaza el stub en el grafo.
 */
sealed interface ResultadoGoogle {
    /**
     * El usuario eligió una cuenta. [idToken] es opaco y de un solo uso
     * hacia el server - **no** se persiste ni se loguea.
     */
    data class Listo(
        val email: String?,
        val displayName: String?,
        /** JWT opaco de Google; solo viaja al endpoint de exchange. */
        val idToken: String,
    ) : ResultadoGoogle

    /** Cerró el picker o tocó atrás. */
    data object Cancelado : ResultadoGoogle

    /**
     * Build sin client id, sin Play Services, o feature flag off.
     * [razonInterna] es para logs de diagnóstico; la UI usa [mensajeUsuario].
     */
    data class NoDisponible(
        val mensajeUsuario: String,
        val razonInterna: String,
    ) : ResultadoGoogle

    /** Fallo de red / Play Services / token vacío. */
    data class Error(val mensajeUsuario: String) : ResultadoGoogle
}

/**
 * Capacidad de pedir una cuenta Google en este aparato.
 */
interface IdentidadGoogle {
    /**
     * `true` solo si el build trae client id y el runtime puede abrir el
     * picker. El stub siempre devuelve `false`.
     */
    fun disponible(): Boolean

    /**
     * Copy de promoción en la pantalla de entrada.
     *
     * [rubroEsFeria] cambia el tono: feria habla de cuaderno y teléfono;
     * formal habla de acceso sin teclear clave cada vez.
     */
    fun copyPromocion(rubroEsFeria: Boolean): String

    /**
     * Etiqueta del botón primario de Google.
     * Estable para tests de UI (Robolectric / escala).
     */
    fun etiquetaBoton(): String = "Continuar con Google"

    /**
     * Lanza el flujo de cuenta. El stub **no** toca red ni UI system.
     */
    suspend fun pedirCuenta(): ResultadoGoogle
}

/**
 * Stub estable: Google no está cableado en este build.
 *
 * Usar en tests, en emuladores sin client id, y en main hasta que el carril
 * de consola Google + secrets (Regla 3) esté listo. No inventa tokens.
 */
object IdentidadGoogleNoCableada : IdentidadGoogle {

    override fun disponible(): Boolean = false

    override fun copyPromocion(rubroEsFeria: Boolean): String =
        if (rubroEsFeria) {
            "Más adelante vas a poder entrar con tu cuenta de Google, sin " +
                "acordarte de otra clave. Hoy usa el correo y la clave del " +
                "negocio. Si se te rompe el teléfono, con Google y la llave " +
                "del cuaderno recuperas ventas y deudas."
        } else {
            "Entrar con Google llega pronto. Por ahora usa el correo y la " +
                "clave que te dieron al crear el negocio."
        }

    override suspend fun pedirCuenta(): ResultadoGoogle =
        ResultadoGoogle.NoDisponible(
            mensajeUsuario = "Entrar con Google todavía no está listo en " +
                "este teléfono. Usa el correo y la clave de abajo.",
            razonInterna = "google_sign_in_not_wired",
        )
}

/**
 * Shape de wire hacia `POST /api/v1/auth/google` ([AuthApi.loginConGoogle]).
 *
 * El server hoy responde 501 (stub). El cliente manda solo el `id_token` de
 * Google + tenant opcional. **Nunca** manda client secret.
 */
data class GoogleTokenExchangeRequest(
    val idToken: String,
    val tenant: String? = null,
)
