package cl.rutbusiness.core.format

/**
 * Formato de peso chileno.
 *
 * El server manda los decimales como texto (`"1490"`, `"1490.00"`) para no
 * perder precisión en el camino. Acá se muestran como los lee cualquiera en
 * Chile: `$1.490`, sin decimales, porque el peso no tiene centavos.
 *
 * Escrito a mano y no con `NumberFormat` porque esto vive en `commonMain`: la
 * misma función tiene que servir en iOS sin cambiar una línea.
 */
fun formatearCLP(monto: String): String {
    val entero = monto.trim().substringBefore('.').substringBefore(',')
    val negativo = entero.startsWith('-')
    val digitos = entero.trimStart('-', '+').ifEmpty { "0" }
    if (digitos.any { !it.isDigit() }) return monto

    val conPuntos = digitos
        .reversed()
        .chunked(3)
        .joinToString(".")
        .reversed()

    val signo = if (negativo) "-" else ""
    return signo + "$" + conPuntos
}
