package cl.rutbusiness.core.offline

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * [AlmacenDeBloques] sobre archivos sueltos en el directorio privado de la app.
 *
 * **Por qué archivos y no DataStore**, que ya está en el proyecto: DataStore
 * mantiene *todas* sus claves en memoria mientras la app viva. Le sirve a
 * `PreferenciasServidorAndroid`, que guarda tres strings cortos, y no le sirve
 * al caché: acá el bloque más grande es el catálogo, y la regla del aparato de
 * 1-2 GB es cachear lo que se usa, no tener el catálogo entero residente.
 * Un archivo se lee cuando la pantalla lo pide y el string se suelta al salir.
 *
 * **La escritura es atómica.** Se escribe a `<nombre>.tmp`, se fuerza a disco
 * (`fd.sync()`) y recién ahí se renombra encima del bueno. Un corte de luz deja
 * el archivo viejo intacto o el nuevo completo, nunca uno cortado por la mitad
 * — que con la cola de ventas adentro sería una venta perdida. `File.renameTo`
 * sobre el mismo sistema de archivos es la operación atómica que da Android;
 * `Files.move(ATOMIC_MOVE)` sería lo mismo pero es API 26 y el piso es 23.
 */
internal class AlmacenDeArchivos(private val carpeta: File) : AlmacenDeBloques {

    override suspend fun leer(nombre: String): String? = withContext(Dispatchers.IO) {
        val archivo = File(carpeta, nombre)
        if (!archivo.exists()) return@withContext null
        // Un archivo ilegible se trata como "no hay nada guardado": el caché es
        // una comodidad y la cola se re-arma vacía en el peor caso. Explotar
        // acá dejaría la app sin abrir, que es peor que perder el caché.
        runCatching { archivo.readText(Charsets.UTF_8) }.getOrNull()
    }

    override suspend fun escribir(nombre: String, contenido: String) = withContext(Dispatchers.IO) {
        carpeta.mkdirs()
        val temporal = File(carpeta, "$nombre.tmp")
        temporal.outputStream().use { salida ->
            salida.write(contenido.toByteArray(Charsets.UTF_8))
            salida.flush()
            // Sin este `sync` el rename puede llegar a disco antes que el
            // contenido y quedar un archivo nuevo vacío. Es justo el caso que
            // se está cubriendo: el teléfono que se apaga solo.
            salida.fd.sync()
        }
        val destino = File(carpeta, nombre)
        if (!temporal.renameTo(destino)) {
            // Algunos sistemas de archivos no renombran encima de un existente.
            destino.delete()
            temporal.renameTo(destino)
        }
        Unit
    }

    override suspend fun borrar(nombre: String) = withContext(Dispatchers.IO) {
        File(carpeta, nombre).delete()
        Unit
    }
}
