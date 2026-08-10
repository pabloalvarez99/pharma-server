package cl.rutbusiness.core.offline

/**
 * Disco de mentira, en memoria.
 *
 * [reinicios] cuenta cuántas veces se "apagó el teléfono": crear una vista
 * nueva sobre el mismo mapa es exactamente lo que pasa cuando el proceso muere
 * y la app vuelve a leer el archivo.
 */
class AlmacenDeMentira(
    private val archivos: MutableMap<String, String> = mutableMapOf(),
) : AlmacenDeBloques {

    var escrituras = 0
        private set

    override suspend fun leer(nombre: String): String? = archivos[nombre]

    override suspend fun escribir(nombre: String, contenido: String) {
        escrituras++
        archivos[nombre] = contenido
    }

    override suspend fun borrar(nombre: String) {
        archivos.remove(nombre)
    }

    /** El mismo disco, visto por un proceso nuevo. */
    fun trasReiniciarLaApp(): AlmacenDeMentira = AlmacenDeMentira(archivos)
}
