package cl.rutbusiness.core.backup

/**
 * Huella visual del payload de rescate (ADR-0022).
 *
 * **No es un QR escaneable.** Es una grilla 21×21 de celdas negras/blancas
 * derivada del SHA-256 del payload, para que la dueña vea en pantalla algo
 * que "parece un código" y lo reconozca al copiar al cuaderno. El dibujo QR
 * real (ZXing / PDF) llega en un carril aparte; hasta entonces el payload
 * texto sigue siendo la fuente de verdad.
 *
 * Determinista: mismo payload → misma grilla.
 */

const val HUELLA_SIZE: Int = 21

/**
 * Matriz [fila][col] = true si la celda va rellena (oscuro).
 * Finder-like corners fijos (estilo QR) para que se vea "de código".
 */
fun huellaVisualDelPayload(payload: String): Array<BooleanArray> {
    val grid = Array(HUELLA_SIZE) { BooleanArray(HUELLA_SIZE) }
    val dig = CryptoPlataforma.sha256(payload.trim().encodeToByteArray())
    // Relleno de datos desde el hash (cíclico).
    var bi = 0
    for (y in 0 until HUELLA_SIZE) {
        for (x in 0 until HUELLA_SIZE) {
            val bit = (dig[bi % dig.size].toInt() ushr (bi % 8)) and 1
            grid[y][x] = bit == 1
            bi++
        }
    }
    // Esquinas fijas 7×7 (marcadores de posición estilo QR, no interoperables).
    pintarFinder(grid, 0, 0)
    pintarFinder(grid, 0, HUELLA_SIZE - 7)
    pintarFinder(grid, HUELLA_SIZE - 7, 0)
    return grid
}

private fun pintarFinder(grid: Array<BooleanArray>, oy: Int, ox: Int) {
    for (y in 0 until 7) {
        for (x in 0 until 7) {
            val borde = y == 0 || y == 6 || x == 0 || x == 6
            val centro = y in 2..4 && x in 2..4
            grid[oy + y][ox + x] = borde || centro
        }
    }
}

/** ASCII compacto para tests / logs (no UI). `#` = relleno, `.` = vacío. */
fun huellaAscii(payload: String): String {
    val g = huellaVisualDelPayload(payload)
    return buildString {
        for (y in 0 until HUELLA_SIZE) {
            for (x in 0 until HUELLA_SIZE) {
                append(if (g[y][x]) '#' else '.')
            }
            if (y < HUELLA_SIZE - 1) append('\n')
        }
    }
}
