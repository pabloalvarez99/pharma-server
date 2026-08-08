package cl.rutbusiness.core.net

/**
 * La dirección del server la escribe una persona a mano en un teléfono. Va a
 * llegar con espacios, sin esquema, con barra al final, o en mayúsculas.
 * Normalizar acá es más barato que pedirle que la escriba de nuevo.
 */
object ServerUrl {

    /** Sugerencias que la pantalla de login muestra como atajo. */
    const val EMULADOR = "http://10.0.2.2:8080"

    /**
     * Deja la URL en forma canónica (`http://host:puerto`, sin barra final) o
     * devuelve `null` si no hay forma de entenderla.
     *
     * - `192.168.1.10:8080`  -> `http://192.168.1.10:8080`
     * - `  MiPc:8080/  `     -> `http://mipc:8080`
     * - `https://erp.cl`     -> `https://erp.cl`
     */
    fun normalizar(entrada: String): String? {
        val limpia = entrada.trim().trimEnd('/')
        if (limpia.isEmpty()) return null

        val conEsquema = when {
            limpia.startsWith("http://", ignoreCase = true) -> "http://" + limpia.substring(7)
            limpia.startsWith("https://", ignoreCase = true) -> "https://" + limpia.substring(8)
            // Sin esquema asumimos http: el caso normal es un PC de la LAN.
            else -> "http://$limpia"
        }

        val sinEsquema = conEsquema.substringAfter("://")
        if (sinEsquema.isEmpty()) return null
        // Un host no puede traer espacios ni ruta; si trae ruta la cortamos,
        // porque los endpoints los pone el cliente generado.
        val hostYPuerto = sinEsquema.substringBefore('/')
        if (hostYPuerto.isEmpty() || hostYPuerto.any { it.isWhitespace() }) return null

        val puerto = hostYPuerto.substringAfter(':', "")
        if (puerto.isNotEmpty() && puerto.toIntOrNull()?.let { it in 1..65535 } != true) return null

        val esquema = conEsquema.substringBefore("://").lowercase()
        return "$esquema://${hostYPuerto.lowercase()}"
    }
}
