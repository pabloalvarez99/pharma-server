package cl.rutbusiness.core.offline

/**
 * Disco: bloques de texto con nombre, que sobreviven a que el proceso muera.
 *
 * Es a propósito la interfaz más chica que sirve. Lo que se guarda acá es JSON
 * —la cola de ventas y lo último que se leyó del server—, así que no hace falta
 * una base de datos: una base local traería un esquema que migrar, un hilo de
 * escritura propio y unos cientos de KB de librería en un aparato de 1 GB,
 * para guardar cinco archivos.
 *
 * **La única garantía que sí hace falta es la de [escribir]**: o queda el
 * contenido nuevo entero, o queda el viejo entero. Nunca la mitad. Con la cola
 * de ventas adentro, un archivo cortado a la mitad porque se apagó el teléfono
 * en medio de la escritura sería una venta perdida, y perder una venta es el
 * único bug que este módulo no tiene permitido tener.
 */
interface AlmacenDeBloques {

    /** El contenido, o `null` si nunca se escribió (o si quedó ilegible). */
    suspend fun leer(nombre: String): String?

    /**
     * Reemplaza el contenido de [nombre], entero o nada.
     *
     * La implementación de plataforma tiene que ser atómica frente a un corte
     * de luz: escribir a un temporal y recién después renombrar encima.
     */
    suspend fun escribir(nombre: String, contenido: String)

    suspend fun borrar(nombre: String)
}
