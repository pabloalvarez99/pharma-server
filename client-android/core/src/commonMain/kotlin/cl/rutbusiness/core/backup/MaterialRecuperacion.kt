package cl.rutbusiness.core.backup

/**
 * Lo que la dueña tipea o escanea para recuperar su respaldo (ADR-0022).
 *
 * Las dos formas son intercambiables: cualquiera decodifica a la **misma**
 * semilla canónica (ver [ClaveDelNegocio]). Cifrar con las palabras y
 * restaurar con el QR tiene que funcionar, y por eso acá sólo se valida la
 * *forma*; la verdad la tiene [semillaDeMaterial].
 */

sealed class MaterialRecuperacion {
    data class Frase(val palabras: List<String>) : MaterialRecuperacion() {
        fun unida(): String = palabras.joinToString(" ")
    }

    data class Bloques(val bloques: List<String>) : MaterialRecuperacion() {
        fun unidos(): String = bloques.joinToString("-")
    }
}

sealed class ErrorMaterial {
    data object Vacio : ErrorMaterial()
    data class FraseMal(val palabras: Int) : ErrorMaterial()
    data class BloquesMal(val detalle: String) : ErrorMaterial()
}

/**
 * Parsea lo que tipeó la dueña.
 *
 * - Payload QR `rutbusiness-rescue:v1:…` → bloques.
 * - 12 tokens → frase.
 * - 5 grupos de 4 alfanuméricos (con `-` o espacios) → bloques.
 *
 * Sólo mira la forma. Que las palabras existan y que los bloques cuadren con
 * su checksum lo resuelve [semillaDeMaterial], que es quien puede decir
 * exactamente cuál está mal.
 */
fun parsearMaterialRecuperacion(raw: String): Result<MaterialRecuperacion> {
    val s = raw.trim()
    if (s.isEmpty()) return Result.failure(IllegalArgumentException("vacío"))

    parsearPayloadQrRescate(s)?.let { (_, blocks) ->
        val parts = blocks.split('-', ' ').filter { it.isNotEmpty() }
        if (parts.size == BLOQUES_TARJETA && parts.all { it.length == LARGO_BLOQUE }) {
            return Result.success(MaterialRecuperacion.Bloques(parts.map { it.uppercase() }))
        }
    }

    val tokens = s.split(Regex("\\s+")).filter { it.isNotEmpty() }
    if (tokens.size == PALABRAS_FRASE) {
        return Result.success(MaterialRecuperacion.Frase(tokens.map { it.lowercase() }))
    }

    val partes = s.split(Regex("[-\\s]+")).filter { it.isNotEmpty() }
    if (partes.size == BLOQUES_TARJETA && partes.all { it.length == LARGO_BLOQUE && it.all(Char::isLetterOrDigit) }) {
        return Result.success(MaterialRecuperacion.Bloques(partes.map { it.uppercase() }))
    }

    return Result.failure(
        IllegalArgumentException(
            "Se esperan $PALABRAS_FRASE palabras o $BLOQUES_TARJETA bloques de $LARGO_BLOQUE (o el código del QR).",
        ),
    )
}

/**
 * Mensaje para la pantalla.
 *
 * Cuenta lo que escribió y lo compara con lo que se espera: "contaste 11" le
 * dice qué hacer, "clave inválida" no.
 */
fun mensajeErrorMaterial(raw: String): String {
    val s = raw.trim()
    if (s.isEmpty()) return "Escribí las palabras o los bloques de tu tarjeta."

    val tokens = s.split(Regex("\\s+")).filter { it.isNotEmpty() }
    val partes = s.split(Regex("[-\\s]+")).filter { it.isNotEmpty() }

    // Si la forma calza, el problema es el contenido: que lo diga quien sabe cuál.
    parsearMaterialRecuperacion(s).getOrNull()?.let { material ->
        semillaDeMaterial(material).exceptionOrNull()?.let { e ->
            return e.message ?: "Revisá la tarjeta: algo no cuadra."
        }
    }

    if (tokens.size != PALABRAS_FRASE && partes.size != BLOQUES_TARJETA) {
        return "Van $PALABRAS_FRASE palabras o $BLOQUES_TARJETA bloques. " +
            "Contaste ${tokens.size} palabra(s)."
    }
    return "Revisá que no falte ninguna letra. Los bloques son de $LARGO_BLOQUE caracteres."
}
