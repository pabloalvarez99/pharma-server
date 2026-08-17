package cl.rutbusiness.core.backup

/**
 * Clave del negocio para el respaldo cifrado (ADR-0022).
 *
 * **Diseño:**
 * 1. Al crear cuenta se genera una clave legible (palabras y bloques).
 * 2. La app **obliga** a mostrarla grande + "escribila en tu cuaderno".
 * 3. La tarjeta de rescate lleva las dos formas (QR con bloques) para pegar
 *    en el cuaderno.
 * 4. Sin la clave, el ciphertext del bucket es basura - y se lo decimos day-1.
 * 5. RutBusiness **nunca** recupera la clave. Solo re-ingreso + Google.
 *
 * ## Las dos formas son la MISMA semilla
 *
 * Ésta es la invariante que sostiene todo el respaldo, y por eso está escrita
 * acá arriba: la frase de 12 palabras y los 5 bloques **codifican los mismos
 * 84 bits**, de ida y de vuelta. Cualquiera de las dos recupera la semilla
 * canónica, y es esa semilla —no el texto que se ve en pantalla— la que
 * alimenta el KDF.
 *
 * Antes no era así: cada forma se derivaba por su cuenta desde la semilla y el
 * KDF corría sobre el *string* mostrado. Escribir las palabras y después
 * escanear el QR (que lleva bloques) daba dos llaves distintas, o sea un
 * respaldo que no abre. Para un feriante que tiene el negocio entero ahí, eso
 * es perderlo todo. La forma de que no vuelva a pasar es que exista una sola
 * semilla canónica y que el KDF nunca vea texto de presentación.
 *
 * ## Por qué 84 bits y no 128
 *
 * El techo lo pone lo que una persona puede escribir a mano en un cuaderno, al
 * sol, sin equivocarse: 12 palabras de un vocabulario de 128 son 7 bits cada
 * una. Con PBKDF2 a 210.000 iteraciones, romperlo por fuerza bruta cuesta del
 * orden de 2^101 operaciones de hash. No es el eslabón débil, y estirar la
 * frase a 18 palabras para llegar a 128 bits sí haría que la dueña la copie
 * mal. Se prefiere una clave que se escribe bien a una que se escribe peor.
 */

/** Bits que aporta cada palabra. El vocabulario tiene 2^7 = 128 entradas. */
private const val BITS_POR_PALABRA = 7

/** Palabras de la frase. */
const val PALABRAS_FRASE: Int = 12

/** Bloques de la tarjeta. */
const val BLOQUES_TARJETA: Int = 5

/** Caracteres por bloque. */
const val LARGO_BLOQUE: Int = 4

/** Bits de la semilla canónica: 12 palabras x 7 bits. */
const val BITS_SEMILLA: Int = PALABRAS_FRASE * BITS_POR_PALABRA

/** Bits de checksum que viajan en los bloques (detectan un bloque mal copiado). */
private const val BITS_CHECKSUM = 16

/** Bytes de la semilla canónica (84 bits caben en 11 bytes; sobran 4 al final). */
const val BYTES_SEMILLA: Int = (BITS_SEMILLA + 7) / 8

/**
 * Alfabeto de los bloques: 32 símbolos, 5 bits cada uno.
 *
 * Sin `0`/`O` ni `1`/`I`: escritos a mano y leídos al sol se confunden, y una
 * letra mal leída acá es un respaldo que no abre.
 */
private const val ALFABETO_BLOQUES = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"

/**
 * Frase de recuperación en español legible.
 *
 * - [palabras]: 12 tokens del vocabulario fijo.
 * - [bloques]: **los mismos** 84 bits en 5 bloques de 4 caracteres, más 16
 *   bits de checksum. Más fácil de copiar con lápiz, y es lo que va en el QR.
 *
 * Las dos formas son intercambiables: se puede cifrar con una y restaurar con
 * la otra.
 */
data class ClaveDelNegocio(
    val palabras: List<String>,
    val bloques: List<String>,
) {
    init {
        require(palabras.size == PALABRAS_FRASE) {
            "frase: $PALABRAS_FRASE palabras, hay ${palabras.size}"
        }
        require(bloques.size == BLOQUES_TARJETA) {
            "bloques: $BLOQUES_TARJETA de $LARGO_BLOQUE chars, hay ${bloques.size}"
        }
        require(bloques.all { it.length == LARGO_BLOQUE }) {
            "cada bloque son $LARGO_BLOQUE caracteres"
        }
    }

    /** Una línea para el cuaderno: "mesa silla pan …". */
    fun fraseCompleta(): String = palabras.joinToString(" ")

    /** Línea de bloques: "AB3K-9F2Q-…". */
    fun bloquesCompletos(): String = bloques.joinToString("-")

    /** Los 84 bits que las dos formas codifican. */
    fun semilla(): ByteArray = semillaDesdeFrase(palabras).getOrThrow()
}

/**
 * Vocabulario es-CL de **exactamente 128 palabras distintas**.
 *
 * Dos propiedades que no son decorativas:
 *
 * - **128 = 2^7**, así cada palabra son 7 bits exactos. Con 113 palabras el
 *   generador anterior hacía `byte % 113`, que además de perder bits mete
 *   sesgo de módulo: 30 de las 113 salían un 50% más seguido que el resto.
 * - **Sin repetidas.** El vocabulario anterior tenía `café` y `pasaje` dos
 *   veces, así que una palabra podía venir de dos índices distintos y
 *   decodificarla era adivinar. Con eso, la frase escrita en el cuaderno no
 *   siempre reconstruía la semilla.
 *
 * Registro: cosas de feria y de casa. Nada de homófonos entre sí.
 * No es BIP-39: es legible en feria, no interoperable con wallets.
 */
internal val VOCABULARIO_RESCATE: List<String> = listOf(
    "mesa", "silla", "pan", "leche", "tomate", "cebolla", "papa", "zapallo",
    "limon", "naranja", "manzana", "platano", "queso", "huevo", "arroz", "fideos",
    "aceite", "azucar", "sal", "cafe", "te", "agua", "vaso", "plato",
    "cuchara", "tenedor", "olla", "sarten", "bolsa", "caja", "cuerda", "clavo",
    "martillo", "llave", "puerta", "ventana", "techo", "piso", "calle", "plaza",
    "feria", "puesto", "carreta", "balanza", "billete", "moneda", "cuenta", "deuda",
    "amigo", "vecina", "don", "dona", "manana", "tarde", "noche", "sol",
    "lluvia", "viento", "cerro", "rio", "mar", "playa", "campo", "huerta",
    "flor", "hoja", "tronco", "piedra", "arena", "barro", "fuego", "humo",
    "radio", "reloj", "lampara", "vela", "jabon", "trapo", "escoba", "balde",
    "camion", "bici", "bus", "metro", "pasaje", "boleto", "cola", "fila",
    "rojo", "verde", "azul", "amarillo", "blanco", "negro", "gris", "morado",
    "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho",
    "norte", "sur", "este", "oeste", "centro", "orilla", "esquina", "vereda",
    "pera", "uva", "sandia", "melon", "palta", "choclo", "poroto", "lechuga",
    "zanahoria", "ajo", "perejil", "durazno", "frutilla", "kilo", "gramo", "canasto",
)

/**
 * Palabra normalizada → índice.
 *
 * La dueña escribe sin tildes y a veces en mayúsculas; el vocabulario ya está
 * guardado sin tildes por la misma razón (una tilde de menos en el cuaderno no
 * puede costar el respaldo).
 */
internal val INDICE_VOCABULARIO: Map<String, Int> =
    VOCABULARIO_RESCATE.withIndex().associate { (i, p) -> p to i }.also {
        // Invariante del formato. Si esto se rompe, el respaldo deja de ser
        // reconstruible: se sube la versión del sobre, no se parchea acá.
        // Se valida al cargar la clase y no sólo en un test, porque el costo de
        // enterarse tarde lo paga la dueña con su respaldo.
        check(VOCABULARIO_RESCATE.size == 1 shl BITS_POR_PALABRA) {
            "el vocabulario son ${1 shl BITS_POR_PALABRA} palabras, hay ${VOCABULARIO_RESCATE.size}"
        }
        check(it.size == VOCABULARIO_RESCATE.size) { "el vocabulario tiene palabras repetidas" }
    }

/**
 * Quita tildes y pasa a minúscula, para que "Limón" y "limon" sean la misma.
 *
 * A mano y sin depender de `java.text.Normalizer`: esto es `commonMain` y
 * tiene que compilar también para iOS.
 */
internal fun normalizarPalabra(p: String): String {
    val sb = StringBuilder(p.length)
    for (c in p.trim().lowercase()) {
        sb.append(
            when (c) {
                'á' -> 'a'
                'é' -> 'e'
                'í' -> 'i'
                'ó' -> 'o'
                'ú', 'ü' -> 'u'
                'ñ' -> 'n'
                else -> c
            },
        )
    }
    return sb.toString()
}

// ── empaquetado de bits ──────────────────────────────────────────────────────

/** Escribe [n] bits (los menos significativos de [valor]) al final de [bits]. */
private fun escribirBits(bits: MutableList<Boolean>, valor: Int, n: Int) {
    for (i in n - 1 downTo 0) bits.add((valor shr i) and 1 == 1)
}

/** Lee [n] bits desde [desde] como entero. */
private fun leerBits(bits: List<Boolean>, desde: Int, n: Int): Int {
    var v = 0
    for (i in 0 until n) v = (v shl 1) or if (bits[desde + i]) 1 else 0
    return v
}

private fun bitsABytes(bits: List<Boolean>): ByteArray {
    val out = ByteArray((bits.size + 7) / 8)
    bits.forEachIndexed { i, b -> if (b) out[i / 8] = (out[i / 8].toInt() or (1 shl (7 - i % 8))).toByte() }
    return out
}

private fun bytesABits(bytes: ByteArray, cuantos: Int): List<Boolean> =
    (0 until cuantos).map { i -> (bytes[i / 8].toInt() shr (7 - i % 8)) and 1 == 1 }

// ── frase ↔ semilla ──────────────────────────────────────────────────────────

/** Índices de las 12 palabras → semilla canónica de [BYTES_SEMILLA] bytes. */
private fun indicesASemilla(indices: List<Int>): ByteArray {
    val bits = mutableListOf<Boolean>()
    indices.forEach { escribirBits(bits, it, BITS_POR_PALABRA) }
    return bitsABytes(bits)
}

private fun semillaAIndices(semilla: ByteArray): List<Int> {
    val bits = bytesABits(semilla, BITS_SEMILLA)
    return (0 until PALABRAS_FRASE).map { leerBits(bits, it * BITS_POR_PALABRA, BITS_POR_PALABRA) }
}

/**
 * 12 palabras → semilla canónica.
 *
 * Tolera tildes ausentes y mayúsculas. Falla con la palabra exacta que no
 * está en el vocabulario, para que la app pueda decir "revisá la palabra 7"
 * en vez de "clave inválida".
 */
fun semillaDesdeFrase(palabras: List<String>): Result<ByteArray> {
    if (palabras.size != PALABRAS_FRASE) {
        return Result.failure(IllegalArgumentException("Van $PALABRAS_FRASE palabras, contaste ${palabras.size}."))
    }
    val indices = ArrayList<Int>(PALABRAS_FRASE)
    palabras.forEachIndexed { i, p ->
        val idx = INDICE_VOCABULARIO[normalizarPalabra(p)]
            ?: return Result.failure(
                IllegalArgumentException("La palabra ${i + 1} (\"${p.trim()}\") no es de la tarjeta."),
            )
        indices.add(idx)
    }
    return Result.success(indicesASemilla(indices))
}

/** Semilla canónica → las 12 palabras. */
fun fraseDesdeSemilla(semilla: ByteArray): List<String> {
    require(semilla.size >= BYTES_SEMILLA) { "semilla: $BYTES_SEMILLA bytes" }
    return semillaAIndices(semilla).map { VOCABULARIO_RESCATE[it] }
}

// ── bloques ↔ semilla ────────────────────────────────────────────────────────

/**
 * Checksum de 16 bits sobre la semilla.
 *
 * No es seguridad: es para que un bloque mal copiado del cuaderno se detecte
 * al tipearlo, y no doce pantallas después con un "no se pudo descifrar".
 */
private fun checksum16(semilla: ByteArray): Int {
    val h = CryptoPlataforma.sha256(semilla)
    return ((h[0].toInt() and 0xFF) shl 8) or (h[1].toInt() and 0xFF)
}

/** Semilla canónica → 5 bloques de 4 (84 bits de semilla + 16 de checksum). */
fun bloquesDesdeSemilla(semilla: ByteArray): List<String> {
    require(semilla.size >= BYTES_SEMILLA) { "semilla: $BYTES_SEMILLA bytes" }
    val bits = mutableListOf<Boolean>()
    bits.addAll(bytesABits(semilla, BITS_SEMILLA))
    escribirBits(bits, checksum16(semilla.copyOf(BYTES_SEMILLA)), BITS_CHECKSUM)

    val chars = (0 until bits.size / 5).map { ALFABETO_BLOQUES[leerBits(bits, it * 5, 5)] }
    return (0 until BLOQUES_TARJETA).map { b ->
        chars.subList(b * LARGO_BLOQUE, (b + 1) * LARGO_BLOQUE).joinToString("")
    }
}

/**
 * 5 bloques → semilla canónica, verificando el checksum.
 *
 * El checksum es lo que convierte "escribiste mal un bloque" en un mensaje
 * inmediato en vez de un respaldo que parece bien y no abre.
 */
fun semillaDesdeBloques(bloques: List<String>): Result<ByteArray> {
    if (bloques.size != BLOQUES_TARJETA) {
        return Result.failure(IllegalArgumentException("Van $BLOQUES_TARJETA bloques, contaste ${bloques.size}."))
    }
    val bits = mutableListOf<Boolean>()
    bloques.forEachIndexed { b, bloque ->
        val limpio = bloque.trim().uppercase()
        if (limpio.length != LARGO_BLOQUE) {
            return Result.failure(
                IllegalArgumentException("El bloque ${b + 1} tiene ${limpio.length} caracteres, van $LARGO_BLOQUE."),
            )
        }
        for (c in limpio) {
            val v = ALFABETO_BLOQUES.indexOf(c)
            if (v < 0) {
                return Result.failure(
                    IllegalArgumentException("El bloque ${b + 1} tiene un carácter que no va: \"$c\"."),
                )
            }
            escribirBits(bits, v, 5)
        }
    }
    val semilla = bitsABytes(bits.subList(0, BITS_SEMILLA))
    val leido = leerBits(bits, BITS_SEMILLA, BITS_CHECKSUM)
    if (leido != checksum16(semilla)) {
        return Result.failure(
            IllegalArgumentException("Los bloques no cuadran. Revisa que no falte ninguna letra."),
        )
    }
    return Result.success(semilla)
}

// ── generación ───────────────────────────────────────────────────────────────

/**
 * Semilla → clave del negocio. Las dos formas salen de los **mismos** bits.
 *
 * Determinista: la misma semilla siempre da la misma frase y los mismos
 * bloques, y cada forma vuelve a esa semilla.
 */
fun generarClaveDelNegocio(semilla: ByteArray): ClaveDelNegocio {
    require(semilla.size >= BYTES_SEMILLA) { "semilla mínima $BYTES_SEMILLA bytes" }
    // Los bits que sobran del último byte se ponen en cero: la semilla
    // canónica son BITS_SEMILLA bits y nada más, o el round-trip no cierra.
    val canonica = indicesASemilla(semillaAIndices(semilla))
    return ClaveDelNegocio(
        palabras = fraseDesdeSemilla(canonica),
        bloques = bloquesDesdeSemilla(canonica),
    )
}

/**
 * Clave de producción: bytes del CSPRNG del aparato → frase + bloques.
 * No persiste la semilla (la dueña la tiene en el cuaderno).
 */
fun claveNuevaDelNegocio(): ClaveDelNegocio =
    generarClaveDelNegocio(CryptoPlataforma.randomBytes(BYTES_SEMILLA))

/** Semilla de demo fija - solo previews y tests. Nunca en alta real. */
fun claveDeDemostracion(): ClaveDelNegocio {
    val demo = byteArrayOf(11, 22, 33, 44, 55, 66, 77, 88, 99, 10, 20)
    return generarClaveDelNegocio(demo)
}
