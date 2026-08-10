package cl.rutbusiness.core.net

/**
 * A quién se le avisa que el server dijo 401.
 *
 * Existe por la misma razón que [ReporteDeRed], y con la misma forma: la capa de
 * red reporta un hecho ("el token no sirve más") y quien escucha decide qué
 * hacer con él. Así [ApiFactory] no necesita conocer a la sesión ni al revés.
 *
 * **Por qué no alcanzaba con acordarse.** Cerrar la sesión al 401 estaba escrito
 * a mano en cada rama de error de cada ViewModel: catorce lugares en seis
 * pantallas. El texto que ve el usuario ("Tu sesión expiró, vuelve a entrar")
 * *exige* que quien lo muestre además llame a `salir()`, porque si no la app
 * manda a entrar de nuevo con el token viejo todavía guardado y el siguiente
 * intento vuelve a dar 401. Eso ya faltó una vez. Un comentario avisando no es
 * un mecanismo; esto sí: se avisa desde [llamar], que es el único embudo por el
 * que pasan todas las llamadas al server.
 */
interface AvisoDeSesion {

    /** El server contestó 401: el token guardado ya no sirve. */
    suspend fun vencio()

    companion object {
        /** El que no escucha. Es el default: los tests no tienen sesión. */
        val Nulo: AvisoDeSesion = object : AvisoDeSesion {
            override suspend fun vencio() = Unit
        }
    }
}
