package cl.rutbusiness.core.backup

/**
 * Clave del negocio para el respaldo cifrado (ADR-0022).
 *
 * **Diseño (stubs de cliente):**
 * 1. Al crear cuenta se genera una clave legible (palabras o bloques).
 * 2. La app **obliga** a mostrarla grande + "escribila en tu cuaderno".
 * 3. Opción: tarjeta de rescate (QR + palabras) para pegar en el cuaderno.
 * 4. Sin la clave, el ciphertext del bucket es basura - y se lo decimos day-1.
 * 5. RutBusiness **nunca** recupera la clave. Solo re-ingreso + Google.
 *
 * Este módulo **no** cifra ni sube nada todavía. Genera material de UI y
 * fija el contrato de palabras para que la pantalla de rescate y los tests
 * no inventen formatos distintos.
 *
 * La derivación real (Argon2id / HKDF → AES-GCM) y el upload al bucket viven
 * en un carril posterior; acá solo la **llave legible** y el copy.
 */

/**
 * Frase de recuperación en español legible.
 *
 * - [palabras]: 12 tokens cortos del vocabulario fijo (sin secretos del server).
 * - [bloques]: misma entropía en 8 bloques de 4 caracteres alfanuméricos
 *   mayúsculas (más fácil de copiar con lápiz en el sol).
 *
 * La app muestra **una** de las dos formas (preferencia de la dueña); ambas
 * codifican la misma semilla cuando el generador real exista. El stub actual
 * genera palabras y bloques independientes de demo - no usar en prod hasta
 * cablear crypto.
 */
data class ClaveDelNegocio(
    val palabras: List<String>,
    val bloques: List<String>,
) {
    init {
        require(palabras.size == 12) { "frase: 12 palabras, hay ${palabras.size}" }
        require(bloques.size == 8) { "bloques: 8 de 4 chars, hay ${bloques.size}" }
        require(bloques.all { it.length == 4 }) { "cada bloque son 4 caracteres" }
    }

    /** Una línea para el cuaderno: "gato mesa ...". */
    fun fraseCompleta(): String = palabras.joinToString(" ")

    /** Línea de bloques: "AB3K-9F2Q-...". */
    fun bloquesCompletos(): String = bloques.joinToString("-")
}

/**
 * Vocabulario corto es-CL (palabras comunes, sin homófonos peligrosos).
 * No es BIP-39: es legible en feria, no interoperable con wallets.
 */
internal val VOCABULARIO_RESCATE: List<String> = listOf(
    "mesa", "silla", "pan", "leche", "tomate", "cebolla", "papa", "zapallo",
    "limón", "naranja", "manzana", "plátano", "queso", "huevo", "arroz", "fideos",
    "aceite", "azúcar", "sal", "café", "té", "agua", "vaso", "plato",
    "cuchara", "tenedor", "olla", "sarten", "bolsa", "caja", "cuerda", "clavo",
    "martillo", "llave", "puerta", "ventana", "techo", "piso", "calle", "plaza",
    "feria", "puesto", "carreta", "balanza", "billete", "moneda", "cuenta", "deuda",
    "amigo", "vecina", "don", "doña", "mañana", "tarde", "noche", "sol",
    "lluvia", "viento", "cerro", "río", "mar", "playa", "campo", "huerta",
    "flor", "hoja", "tronco", "piedra", "arena", "barro", "fuego", "humo",
    "radio", "reloj", "lámpara", "vela", "jabón", "trapo", "escoba", "balde",
    "camión", "bici", "bus", "metro", "pasaje", "boleto", "cola", "fila",
    "rojo", "verde", "azul", "amarillo", "blanco", "negro", "gris", "café",
    "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho",
    "norte", "sur", "este", "oeste", "centro", "orilla", "esquina", "pasaje",
)

/**
 * Generador **determinista de UI** a partir de 16 bytes de semilla.
 *
 * En producción la semilla sale de CSPRNG del dispositivo y se deriva la
 * clave de cifrado aparte. Acá se indexa el vocabulario y se fabrican
 * bloques hex-like para que la tarjeta de rescate se pueda previsualizar.
 */
fun generarClaveDelNegocio(semilla: ByteArray): ClaveDelNegocio {
    require(semilla.size >= 16) { "semilla mínima 16 bytes" }
    val palabras = (0 until 12).map { i ->
        val idx = (semilla[i].toInt() and 0xFF) % VOCABULARIO_RESCATE.size
        VOCABULARIO_RESCATE[idx]
    }
    val alfabeto = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789" // sin 0/O/1/I
    val bloques = (0 until 8).map { b ->
        buildString(4) {
            for (k in 0 until 4) {
                val byte = semilla[(b * 2 + k / 2) % semilla.size].toInt() and 0xFF
                append(alfabeto[(byte + k * 7 + b) % alfabeto.length])
            }
        }
    }
    return ClaveDelNegocio(palabras = palabras, bloques = bloques)
}

/**
 * Clave de producción: 16 bytes del CSPRNG del aparato → frase + bloques.
 * No persiste la semilla (la dueña la tiene en el cuaderno).
 */
fun claveNuevaDelNegocio(): ClaveDelNegocio =
    generarClaveDelNegocio(CryptoPlataforma.randomBytes(16))

/** Semilla de demo fija - solo previews y tests. Nunca en alta real. */
fun claveDeDemostracion(): ClaveDelNegocio {
    val demo = byteArrayOf(
        11, 22, 33, 44, 55, 66, 77, 88,
        99, 10, 20, 30, 40, 50, 60, 70,
    )
    return generarClaveDelNegocio(demo)
}
