package cl.rutbusiness.ui.components

/**
 * What the user is told when something fails.
 *
 * This is the port of `classifyFetchError` from
 * `client/src/views/stock-helpers.ts`, and the copy was rewritten rather than
 * translated. The original leaked the plumbing:
 *
 * > "No se pudo conectar a pharma-server. Verifica que esté corriendo e
 * > inténtalo de nuevo."
 *
 * `pharma-server` is the name of a git repository. The person holding the phone
 * has no idea what it is, cannot check whether it is "corriendo", and now feels
 * the failure is theirs. Same with "Ocurrió un error al cargar la información",
 * which says nothing actionable at all.
 *
 * **Tone = recado, not stacktrace.** Short title, plain Chilean Spanish, what
 * happened then what to do. No process names, no status codes, no "error", no
 * "servidor", no "sistema". If there is genuinely nothing the user can do, the
 * copy says who can.
 */
data class RbErrorCopy(
    val title: String,
    val message: String,
    /** Null when retrying cannot possibly help - a permissions problem. */
    val retryLabel: String?,
)

/** The kinds of failure the UI distinguishes. Anything else is [Unknown]. */
enum class RbErrorKind {
    /** The phone could not reach the business's system. */
    Offline,

    /** Reached it, but it took too long to answer. */
    Timeout,

    /** This account is not allowed to see this. */
    Forbidden,

    /** The session is no longer valid. */
    Unauthorized,

    /** The system answered with a failure. */
    ServerFault,

    /** Anything not recognised. */
    Unknown,
}

/**
 * Human copy for a failure kind.
 *
 * @param what the thing being loaded, in a form that completes "no pudimos
 *   cargar ___" - for example "tus ventas", "el inventario". Defaults to a
 *   phrasing that works when the caller has nothing specific to say.
 */
fun rbErrorCopy(kind: RbErrorKind, what: String = "esta parte"): RbErrorCopy = when (kind) {
    // Genérico (catálogo, Hoy, fallbacks): no nombra PC ni "negocio" — en feria
    // es un puesto y en nube no hay computador del local. Lo accionable es la
    // red del teléfono; pantallas con copy propio (entrada/caja) no pasan por acá.
    RbErrorKind.Offline -> RbErrorCopy(
        title = "No llegamos",
        message = "No pudimos traer $what. Revisá que el teléfono tenga wifi o datos " +
            "prendidos e intentá de nuevo.",
        retryLabel = "Reintentar",
    )

    RbErrorKind.Timeout -> RbErrorCopy(
        title = "Se demoró",
        message = "No alcanzamos a traer $what. Esperá un ratito e intentá de nuevo.",
        retryLabel = "Reintentar",
    )

    RbErrorKind.Forbidden -> RbErrorCopy(
        title = "Esto no está para tu cuenta",
        message = "No podés ver $what con esta cuenta. Si lo necesitás para trabajar, " +
            "pedile a quien te dio la cuenta que te lo habilite.",
        // Retrying the same request with the same account cannot succeed.
        retryLabel = null,
    )

    // "Tu RUT y tu clave" describía una pantalla de entrada que no existe: la
    // app pide el nombre corto del negocio, el correo y la clave. Un mensaje
    // que nombra un campo inexistente hace dudar de si la app es la correcta.
    //
    // Sin botón: los `ViewModel` cierran la sesión al ver este error, así que la
    // app se va sola a la pantalla de entrada. Un botón acá alcanzaría a
    // dibujarse un instante y llevaría a la misma pantalla que ya viene sola.
    //
    // CONDICIÓN QUE ESTE TEXTO EXIGE: la sesión se tiene que cerrar de verdad.
    // El redirect no lo hace el mensaje, lo hace `RutBusinessApp` observando
    // `sesion.estado`. Ya pasó una vez que faltara — en el cobro (2026-08-09):
    // el cajero quedaba mirando "te vamos a llevar a la pantalla de entrada"
    // para siempre, y sin botón, porque acá `retryLabel` es null.
    //
    // Eso ya no depende de que cada pantalla se acuerde. Lo cumple
    // `core.net.llamar()`, por donde pasan todas las llamadas al server: al ver
    // un 401 avisa a la sesión (`AvisoDeSesion`) y ésta se cierra sola. Si algún
    // día hay un caso donde no se puede cerrar la sesión, lo que se cambia es el
    // texto, no el silencio: prometerle un rescate a alguien que ya está
    // perdido y no cumplirlo es peor que no decirle nada.
    RbErrorKind.Unauthorized -> RbErrorCopy(
        title = "Hay que entrar de nuevo",
        message = "Por seguridad se cerró tu sesión. Te llevamos a la entrada para " +
            "que pongas tu correo y tu clave.",
        retryLabel = null,
    )

    RbErrorKind.ServerFault -> RbErrorCopy(
        title = "No se pudo ahora",
        message = "No es culpa tuya y no perdiste nada de lo que hiciste. " +
            "Intentá de nuevo en un ratito.",
        retryLabel = "Reintentar",
    )

    RbErrorKind.Unknown -> RbErrorCopy(
        title = "No salió",
        message = "No pudimos mostrar $what. Intentá de nuevo; si sigue igual, " +
            "cerrá la app y abrila otra vez.",
        retryLabel = "Reintentar",
    )
}
