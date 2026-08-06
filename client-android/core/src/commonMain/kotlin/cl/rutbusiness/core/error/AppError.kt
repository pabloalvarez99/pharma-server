package cl.rutbusiness.core.error

/**
 * Todo lo que puede salir mal entre el teléfono y el server, en categorías que
 * el usuario entiende.
 *
 * La regla: cada caso trae un mensaje que dice **qué hacer**, no qué falló por
 * dentro. "No pudimos conectar" es inútil solo; "revisa que el PC del negocio
 * esté prendido" es accionable. Los detalles técnicos van en [technical], que
 * nunca se muestra salvo que el operador abra el detalle.
 */
sealed class AppError(
    val userMessage: String,
    val technical: String? = null,
) {
    /**
     * No se pudo hablar con el server: sin red en el teléfono, PC apagado,
     * dirección equivocada o el server caído.
     *
     * A propósito **no** se separa "sin internet" de "server apagado": desde el
     * teléfono las dos se ven igual, y prometer precisión que no tenemos manda
     * al usuario a revisar lo que no era. Un mensaje que cubre los dos casos y
     * los nombra en orden de probabilidad sirve más.
     */
    class ServidorNoResponde(val baseUrl: String, technical: String? = null) : AppError(
        userMessage = "No pudimos conectar con $baseUrl. Revisa que tu teléfono tenga internet, que el PC del negocio esté prendido y que la dirección esté bien escrita.",
        technical = technical,
    )

    /** La dirección que escribió el operador no es una URL usable. */
    class DireccionInvalida(technical: String? = null) : AppError(
        userMessage = "Esa dirección no se entiende. Debe verse como http://192.168.1.10:8080",
        technical = technical,
    )

    /** Usuario, contraseña o sucursal equivocados. */
    class CredencialesInvalidas(technical: String? = null) : AppError(
        userMessage = "Correo, contraseña o sucursal incorrectos. Revísalos y vuelve a intentar.",
        technical = technical,
    )

    /** El JWT venció o fue revocado: hay que volver a entrar. */
    class SesionExpirada(technical: String? = null) : AppError(
        userMessage = "Tu sesión venció. Vuelve a entrar para seguir.",
        technical = technical,
    )

    /** El usuario entró bien pero no tiene permiso para esta acción. */
    class SinPermiso(technical: String? = null) : AppError(
        userMessage = "Tu usuario no tiene permiso para esto. Pídeselo a quien administra el negocio.",
        technical = technical,
    )

    /** El server contestó un error propio (4xx/5xx con envelope). */
    class ErrorDelServidor(
        val status: Int,
        val code: String?,
        serverMessage: String?,
        technical: String? = null,
    ) : AppError(
        userMessage = serverMessage?.takeIf { it.isNotBlank() }
            ?: "El servidor tuvo un problema (error $status). Intenta de nuevo en un momento.",
        technical = technical,
    )

    /** Cualquier otra cosa. Nunca se traga en silencio. */
    class Inesperado(technical: String? = null) : AppError(
        userMessage = "Pasó algo que no esperábamos. Intenta de nuevo; si sigue igual, avísale a soporte.",
        technical = technical,
    )
}
