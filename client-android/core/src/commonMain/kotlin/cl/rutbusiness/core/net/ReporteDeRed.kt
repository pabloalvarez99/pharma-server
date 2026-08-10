package cl.rutbusiness.core.net

/**
 * A quién se le avisa cómo le fue a cada llamada.
 *
 * Existe para que [ApiFactory] no tenga que conocer al monitor de conexión ni
 * al revés: la capa de red reporta hechos ("contestó", "no contestó") y quien
 * escucha decide qué cartel prender.
 *
 * Se avisa desde [llamar], que es el **único** embudo por el que pasan todas
 * las llamadas al server. Ponerlo en cada repositorio dejaría agujeros, y un
 * agujero acá es un cartel de "sin conexión" que no se prende.
 */
interface ReporteDeRed {

    /** Hubo respuesta HTTP, aunque haya sido un error del server. */
    fun anotarRespuesta()

    /** La llamada se fue y no volvió: timeout, DNS, conexión rechazada. */
    fun anotarSilencio()

    companion object {
        /** El que no escucha. Es el default: los tests no necesitan carteles. */
        val Nulo: ReporteDeRed = object : ReporteDeRed {
            override fun anotarRespuesta() = Unit
            override fun anotarSilencio() = Unit
        }
    }
}
