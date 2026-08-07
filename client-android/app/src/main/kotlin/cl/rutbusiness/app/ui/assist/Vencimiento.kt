package cl.rutbusiness.app.ui.assist

/**
 * Cuánto le queda de vida al token de confirmación, y cómo se dice eso en
 * palabras que la dueña entienda.
 *
 * Un timestamp en pantalla no le sirve a nadie. "Tienes unos minutos" sí.
 */
object Vencimiento {

    /**
     * Segundos que quedan, calculados desde el `expires_at` del server.
     *
     * **El reloj del teléfono no es confiable** y por eso el resultado se
     * acota. Los aparatos del piso de hardware son viejos y muchos andan con
     * la hora corrida; si se restara a ciegas, un teléfono adelantado media
     * hora mostraría "vencida" una propuesta recién nacida, y uno atrasado
     * diría que quedan 30 minutos de algo que dura 3. El recorte a
     * [MINIMO_SEGUNDOS]..[MAXIMO_SEGUNDOS] convierte un reloj malo en, como
     * mucho, un texto conservador — nunca en una propuesta imposible de
     * confirmar.
     *
     * La verdad final la tiene el server igual: si el token venció, `/act`
     * lo rechaza y la pantalla lo cuenta.
     *
     * @param expiresAtRfc3339 el `expires_at` tal cual vino.
     * @param ahoraEpochSegundos el reloj del teléfono, en segundos UTC.
     * @return segundos restantes, o `null` si la fecha no se pudo leer.
     */
    fun segundosRestantes(expiresAtRfc3339: String, ahoraEpochSegundos: Long): Long? {
        val vence = epochSegundosDesdeRfc3339(expiresAtRfc3339) ?: return null
        val crudo = vence - ahoraEpochSegundos
        return crudo.coerceIn(MINIMO_SEGUNDOS, MAXIMO_SEGUNDOS)
    }

    /**
     * El vencimiento dicho en castellano.
     *
     * Tres tramos y nada más. La precisión al segundo no ayuda a decidir y
     * mete presión: un reloj corriendo es lo último que necesita alguien que
     * está leyendo con calma si el gasto que va a guardar es el correcto.
     */
    fun enPalabras(segundosRestantes: Long): String = when {
        segundosRestantes <= 0 -> "Ya pasó el tiempo para confirmarla."
        segundosRestantes <= 45 -> "Queda poco tiempo para confirmarla."
        else -> "Tienes unos minutos para confirmarla."
    }

    /** Piso del recorte: siempre hay margen para leer y apretar. */
    const val MINIMO_SEGUNDOS: Long = 20

    /**
     * Techo del recorte. El server usa `TOKEN_TTL_SECS = 180`; esto es
     * deliberadamente más alto para no contradecirlo si algún día lo suben,
     * pero lo bastante bajo como para atajar un reloj corrido de horas.
     */
    const val MAXIMO_SEGUNDOS: Long = 15 * 60

    /**
     * Lee `YYYY-MM-DDTHH:MM:SS(.frac)?(Z|±HH:MM)` a segundos epoch UTC.
     *
     * Escrito a mano en vez de usar `java.time`: el proyecto es `minSdk 21` y
     * no tiene desugaring, así que `Instant.parse` revienta en un Android 5 o
     * 6 — justo los aparatos que este producto quiere servir. Y agregar
     * `kotlinx-datetime` sería una dependencia entera para leer una fecha, en
     * un teléfono al que le importa cada megabyte del APK.
     *
     * Devuelve `null` ante cualquier cosa que no calce, sin lanzar: una fecha
     * ilegible degrada a "no sabemos cuánto queda", no a una app caída.
     */
    fun epochSegundosDesdeRfc3339(texto: String): Long? {
        val s = texto.trim()
        if (s.length < 19) return null
        if (s[4] != '-' || s[7] != '-' || (s[10] != 'T' && s[10] != ' ')) return null
        if (s[13] != ':' || s[16] != ':') return null

        val anio = s.substring(0, 4).toIntOrNull() ?: return null
        val mes = s.substring(5, 7).toIntOrNull() ?: return null
        val dia = s.substring(8, 10).toIntOrNull() ?: return null
        val hora = s.substring(11, 13).toIntOrNull() ?: return null
        val minuto = s.substring(14, 16).toIntOrNull() ?: return null
        val segundo = s.substring(17, 19).toIntOrNull() ?: return null

        if (mes !in 1..12 || dia !in 1..31) return null
        if (hora !in 0..23 || minuto !in 0..59 || segundo !in 0..60) return null

        // Desplazamiento de zona. El server manda `Z`, pero leerlo igual evita
        // un error de 3 horas si algún día manda `-03:00`.
        val resto = s.substring(19)
        val offsetSegundos = when {
            resto.isEmpty() || resto.startsWith("Z") || resto.startsWith("z") -> 0L
            else -> {
                val signoIndice = resto.indexOfFirst { it == '+' || it == '-' }
                if (signoIndice < 0) {
                    0L
                } else {
                    val signo = if (resto[signoIndice] == '-') -1L else 1L
                    val zona = resto.substring(signoIndice + 1).replace(":", "")
                    if (zona.length < 4) return null
                    val zh = zona.substring(0, 2).toLongOrNull() ?: return null
                    val zm = zona.substring(2, 4).toLongOrNull() ?: return null
                    signo * (zh * 3600 + zm * 60)
                }
            }
        }

        val dias = diasDesdeEpoch(anio, mes, dia)
        return dias * 86_400L + hora * 3600L + minuto * 60L + segundo - offsetSegundos
    }

    /**
     * Días entre 1970-01-01 y la fecha dada, algoritmo `days_from_civil` de
     * Howard Hinnant. Es aritmética pura: sin tablas, sin bisiestos a mano y
     * sin bibliotecas.
     */
    private fun diasDesdeEpoch(anio: Int, mes: Int, dia: Int): Long {
        val y = (if (mes <= 2) anio - 1 else anio).toLong()
        val era = (if (y >= 0) y else y - 399) / 400
        val yoe = y - era * 400
        val mp = (mes + 9) % 12
        val doy = (153L * mp + 2) / 5 + dia - 1
        val doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
        return era * 146_097 + doe - 719_468
    }
}
