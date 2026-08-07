package cl.rutbusiness.app.ui.resumen

/**
 * Los bordes del día, en UTC, como los entiende el server.
 *
 * Dos decisiones que hay que dejar escritas porque no son obvias:
 *
 * **Por qué UTC y no la hora de Chile.** `crates/api/src/v1/dashboard.rs`
 * arma su "hoy" con `today_range()`, que es el día UTC, y `sales_daily` agrupa
 * por día UTC. Si el teléfono pidiera "ayer" en horario local, las dos cifras
 * que la pantalla pone una al lado de la otra estarían medidas con reglas
 * distintas y la comparación mentiría. Se usa la misma convención que el
 * server, aunque el server tenga la suya discutible: una venta de las 21:30 en
 * Chile cae en el día UTC siguiente. Cambiarlo es trabajo del server, no del
 * cliente, y el día que cambie esto lo sigue sin tocar nada.
 *
 * **Por qué no `java.time`.** `minSdk` es 21 y `java.time` recién existe en
 * 26; encender desugaring por dos fechas no se paga. La aritmética es sobre
 * milisegundos de época, que en UTC no tiene saltos de hora ni de verano, y la
 * conversión a fecha civil es el algoritmo de Hinnant — exacto, sin tablas.
 *
 * Sin `java.*` ni `android.*` a propósito: esto compila igual para iOS el día
 * que se mueva a `core`.
 */
internal object DiaUtc {

    const val MILIS_POR_DIA = 86_400_000L

    /** Medianoche UTC del día en que cae [ahora]. */
    fun comienzoDelDia(ahora: Long): Long = ahora - pisoModulo(ahora, MILIS_POR_DIA)

    /**
     * `[desde, hasta]` cubriendo **ayer** completo, listo para mandar como
     * `from`/`to`.
     *
     * El cierre es el último milisegundo de ayer y no la medianoche de hoy: el
     * filtro del server es inclusivo en los dos extremos, así que terminar en
     * `00:00:00` de hoy metería las ventas del primer milisegundo de hoy en el
     * total de ayer.
     */
    fun rangoDeAyer(ahora: Long): Pair<String, String> {
        val comienzoDeHoy = comienzoDelDia(ahora)
        return rfc3339(comienzoDeHoy - MILIS_POR_DIA) to rfc3339(comienzoDeHoy - 1L)
    }

    /** `2026-08-05T00:00:00Z`, que es lo que `DateTime<Utc>` sabe leer. */
    fun rfc3339(milisDeEpoca: Long): String {
        val dias = pisoDivision(milisDeEpoca, MILIS_POR_DIA)
        val enElDia = milisDeEpoca - dias * MILIS_POR_DIA
        val (anio, mes, dia) = fechaCivil(dias)

        val hora = enElDia / 3_600_000L
        val minuto = (enElDia / 60_000L) % 60L
        val segundo = (enElDia / 1_000L) % 60L

        return "${dosDigitos(anio, 4)}-${dosDigitos(mes, 2)}-${dosDigitos(dia, 2)}" +
            "T${dosDigitos(hora, 2)}:${dosDigitos(minuto, 2)}:${dosDigitos(segundo, 2)}Z"
    }

    /**
     * Día de época a `(año, mes, día)`.
     *
     * `civil_from_days` de Howard Hinnant, el mismo que usa `chrono` del lado
     * del server. Vale para cualquier fecha del calendario gregoriano
     * proléptico, así que no hay borde de año que probar.
     */
    private fun fechaCivil(diasDeEpoca: Long): Triple<Long, Long, Long> {
        // Se corre el origen al 1 de marzo del año 0: con marzo como primer mes,
        // el día bisiesto queda al final y la aritmética no necesita casos.
        val z = diasDeEpoca + 719_468L
        val era = pisoDivision(z, 146_097L)
        val diaDeLaEra = z - era * 146_097L
        val anioDeLaEra = (
            diaDeLaEra - diaDeLaEra / 1_460L + diaDeLaEra / 36_524L - diaDeLaEra / 146_096L
            ) / 365L
        val anio = anioDeLaEra + era * 400L
        val diaDelAnio = diaDeLaEra - (365L * anioDeLaEra + anioDeLaEra / 4L - anioDeLaEra / 100L)
        val mesCorrido = (5L * diaDelAnio + 2L) / 153L
        val dia = diaDelAnio - (153L * mesCorrido + 2L) / 5L + 1L
        val mes = if (mesCorrido < 10L) mesCorrido + 3L else mesCorrido - 9L
        return Triple(if (mes <= 2L) anio + 1L else anio, mes, dia)
    }

    private fun dosDigitos(valor: Long, ancho: Int): String =
        valor.toString().padStart(ancho, '0')

    /** División entera que redondea hacia abajo también con negativos. */
    private fun pisoDivision(a: Long, b: Long): Long {
        val q = a / b
        return if (a % b != 0L && (a xor b) < 0L) q - 1L else q
    }

    /** Resto siempre no negativo, para que 1969 no rompa el cálculo. */
    private fun pisoModulo(a: Long, b: Long): Long = a - pisoDivision(a, b) * b
}
