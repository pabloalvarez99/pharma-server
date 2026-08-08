package cl.rutbusiness.core.backup

/**
 * Validación pura del material de la tarjeta de rescate (ADR-0022).
 *
 * Usado en restore y en tests: la dueña tipea palabras o bloques; acá se
 * comprueba la **forma** antes de intentar Argon2id / descifrado.
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
 * - Si hay 12 tokens → frase.
 * - Si hay 8 de 4 alnum (con `-` o espacios) → bloques.
 * - Si es payload QR `rutbusiness-rescue:v1:…` → bloques del payload.
 */
fun parsearMaterialRecuperacion(raw: String): Result<MaterialRecuperacion> {
    val s = raw.trim()
    if (s.isEmpty()) return Result.failure(IllegalArgumentException("vacío"))

    parsearPayloadQrRescate(s)?.let { (_, blocks) ->
        val parts = blocks.split('-', ' ').filter { it.isNotEmpty() }
        if (parts.size == 8 && parts.all { it.length == 4 }) {
            return Result.success(MaterialRecuperacion.Bloques(parts.map { it.uppercase() }))
        }
    }

    val tokens = s.split(Regex("\\s+")).filter { it.isNotEmpty() }
    if (tokens.size == 12) {
        return Result.success(MaterialRecuperacion.Frase(tokens.map { it.lowercase() }))
    }

    val partes = s.split(Regex("[-\\s]+")).filter { it.isNotEmpty() }
    if (partes.size == 8 && partes.all { it.length == 4 && it.all(Char::isLetterOrDigit) }) {
        return Result.success(MaterialRecuperacion.Bloques(partes.map { it.uppercase() }))
    }

    return Result.failure(
        IllegalArgumentException(
            "Se esperan 12 palabras o 8 bloques de 4 (o el código del QR).",
        ),
    )
}

fun mensajeErrorMaterial(raw: String): String {
    val s = raw.trim()
    if (s.isEmpty()) return "Escribí las palabras o los bloques de tu tarjeta."
    val tokens = s.split(Regex("\\s+")).filter { it.isNotEmpty() }
    if (tokens.size != 12 && tokens.size != 8) {
        return "Van 12 palabras o 8 bloques. Contaste ${tokens.size}."
    }
    return "Revisá que no falte ninguna letra. Los bloques son de 4 caracteres."
}
