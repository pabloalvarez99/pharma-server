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
 * Every message here follows one shape: **what happened, then what to do.** No
 * process names, no status codes, no "error". If there is genuinely nothing the
 * user can do, the copy says who can.
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
    // Nombra las dos causas, en orden de probabilidad, porque desde el teléfono
    // se ven idénticas. La versión anterior culpaba sólo al teléfono ("no está
    // llegando a internet") y mandaba a revisar la señal a quien tenía el
    // computador del negocio apagado: media hora perdida en el lugar equivocado.
    RbErrorKind.Offline -> RbErrorCopy(
        title = "No llegamos a tu negocio",
        message = "No pudimos traer $what. Revisa que el teléfono tenga wifi o datos " +
            "prendidos, y que el computador del negocio esté encendido.",
        retryLabel = "Reintentar",
    )

    RbErrorKind.Timeout -> RbErrorCopy(
        title = "Se demoró demasiado",
        message = "La conexión está muy lenta y no alcanzamos a traer $what. " +
            "Espera unos segundos y vuelve a intentar.",
        retryLabel = "Reintentar",
    )

    RbErrorKind.Forbidden -> RbErrorCopy(
        title = "No tienes acceso a esto",
        message = "Tu cuenta no puede ver $what. Si lo necesitas para trabajar, " +
            "pídele al dueño del negocio que te lo habilite.",
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
    // CONDICIÓN QUE ESTE TEXTO EXIGE, y que este módulo no puede garantizar
    // solo: quien muestre esta copy TIENE que llamar a `sesion.salir()`. El
    // redirect no lo hace el mensaje, lo hace `RutBusinessApp` observando
    // `sesion.estado`. Ya pasó una vez que faltara — en el cobro (2026-08-09):
    // el cajero quedaba mirando "te vamos a llevar a la pantalla de entrada"
    // para siempre, y sin botón, porque acá `retryLabel` es null. Si algún día
    // hay un caso donde no se puede cerrar la sesión, lo que se cambia es el
    // texto, no el silencio: prometerle un rescate a alguien que ya está
    // perdido y no cumplirlo es peor que no decirle nada.
    RbErrorKind.Unauthorized -> RbErrorCopy(
        title = "Tienes que entrar de nuevo",
        message = "Por seguridad se cerró tu sesión. Te vamos a llevar a la pantalla " +
            "de entrada para que ingreses con tu correo y tu clave.",
        retryLabel = null,
    )

    RbErrorKind.ServerFault -> RbErrorCopy(
        title = "El sistema no respondió bien",
        message = "No es culpa tuya y no perdiste nada de lo que hiciste. " +
            "Vuelve a intentar en un momento.",
        retryLabel = "Reintentar",
    )

    RbErrorKind.Unknown -> RbErrorCopy(
        title = "No pudimos mostrar esto",
        message = "Algo falló al cargar $what. Vuelve a intentar; si sigue " +
            "igual, cierra la aplicación y ábrela de nuevo.",
        retryLabel = "Reintentar",
    )
}
