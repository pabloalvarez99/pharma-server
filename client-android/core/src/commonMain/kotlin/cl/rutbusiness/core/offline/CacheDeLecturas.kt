package cl.rutbusiness.core.offline

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Lo último que contestó el server, guardado para cuando no conteste.
 *
 * Sólo **lecturas**. Una escritura que no salió no se cachea: se encola
 * ([ColaDeVentas]) o se le dice a la dueña que no se pudo. Guardar una
 * escritura acá haría creer que pasó algo que no pasó.
 *
 * Cada bloque es un archivo aparte y se lee cuando la pantalla lo pide. Eso es
 * deliberado con 1-2 GB de RAM: el catálogo no queda residente, entra cuando se
 * abre Cobrar y se suelta al salir. Un caché de todo, siempre en memoria, es
 * exactamente lo que no se puede pagar acá.
 *
 * El nombre del archivo lleva adentro contra qué server se guardó. Un teléfono
 * que se apunta a otro negocio no puede ver el catálogo del anterior, y esa
 * separación tiene que estar en el nombre y no en un `if`, porque un `if` que
 * falta se ve igual que un caché vacío.
 */
class CacheDeLecturas(
    private val almacen: AlmacenDeBloques,
    private val reloj: () -> Long,
) {

    /** Guarda [valor] bajo [clave], sellado con la hora de ahora. */
    suspend fun <T> guardar(clave: ClaveDeCache, serializador: KSerializer<T>, valor: T) {
        val sobre = Sobre(guardadoEn = reloj(), datos = valor)
        val texto = runCatching {
            JSON.encodeToString(Sobre.serializer(serializador), sobre)
        }.getOrNull() ?: return
        almacen.escribir(clave.archivo, texto)
    }

    /**
     * Lo guardado, o `null` si no hay nada usable.
     *
     * Un JSON viejo que ya no calza con el modelo (el server cambió, la app se
     * actualizó) vuelve `null` en vez de explotar: el caché es una comodidad y
     * su peor caso permitido es no estar.
     */
    suspend fun <T> leer(clave: ClaveDeCache, serializador: KSerializer<T>): Fechado<T>? {
        val texto = almacen.leer(clave.archivo) ?: return null
        val sobre = runCatching {
            JSON.decodeFromString(Sobre.serializer(serializador), texto)
        }.getOrNull() ?: return null
        return Fechado(valor = sobre.datos, guardadoEn = sobre.guardadoEn)
    }

    /** Borra todo lo cacheado de un server. Se llama al cerrar sesión. */
    suspend fun olvidar(claves: List<ClaveDeCache>) {
        claves.forEach { almacen.borrar(it.archivo) }
    }

    @Serializable
    private data class Sobre<T>(val guardadoEn: Long, val datos: T)

    private companion object {
        /**
         * `ignoreUnknownKeys` por lo mismo que en `ApiFactory`: acá se guardan
         * los mismos DTO que manda el server, y un caché escrito por la versión
         * anterior de la app no puede tumbar la nueva.
         */
        val JSON = Json {
            ignoreUnknownKeys = true
            explicitNulls = false
        }
    }
}

/**
 * Qué se cachea, y de qué server.
 *
 * [servidor] entra en el nombre del archivo reducido a algo que un sistema de
 * archivos acepte. No es criptografía y no pretende serlo: es separar dos
 * negocios distintos en el mismo teléfono.
 */
class ClaveDeCache private constructor(val archivo: String) {

    companion object {
        fun de(que: Que, servidor: String): ClaveDeCache =
            ClaveDeCache("${que.prefijo}-${apodo(servidor)}.json")

        /** Todo lo que se cachea, en un solo lugar para poder borrarlo junto. */
        fun todas(servidor: String): List<ClaveDeCache> = Que.entries.map { de(it, servidor) }

        /**
         * Un nombre corto y estable para una URL.
         *
         * Suma de 32 bits sobre los caracteres, en hexa. Dos servers distintos
         * podrían chocar en teoría; en la práctica un teléfono ve uno o dos, y
         * la consecuencia de un choque sería ver el catálogo viejo hasta la
         * primera carga con red. No amerita traer una dependencia de hashing.
         */
        private fun apodo(servidor: String): String {
            var acumulado = 0
            for (c in servidor.trim().trimEnd('/').lowercase()) {
                acumulado = acumulado * 31 + c.code
            }
            return acumulado.toUInt().toString(16)
        }
    }

    enum class Que(val prefijo: String) {
        Catalogo("catalogo"),
        Clientes("clientes"),
        Deudores("deudores"),
        Resumen("resumen"),
    }
}
