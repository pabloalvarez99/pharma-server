package cl.rutbusiness.core.offline

/**
 * Un dato guardado, con la hora en que se trajo del server.
 *
 * La fecha no es decoración: sin red, la diferencia entre "esto es de recién" y
 * "esto es de anteayer" es la diferencia entre un dato y una adivinanza. La
 * pantalla que muestra caché **siempre** muestra [antiguedad] al lado.
 */
data class Fechado<out T>(val valor: T, val guardadoEn: Long) {

    /**
     * Cuánto hace que se trajo, dicho como lo diría una persona.
     *
     * Redondea hacia abajo a propósito: "hace 1 hora" con 119 minutos encima
     * miente menos que "hace 2 horas" con 61. Y el número no se acompaña de
     * decimales — nadie necesita saber que fueron 23,4 minutos.
     *
     * Si el reloj del teléfono se movió para atrás (cambio de hora, arranque
     * antes de sincronizar con la red), la resta da negativa y se dice
     * "recién": inventar "hace -3 horas" sería peor.
     */
    fun antiguedad(ahora: Long): String {
        val minutos = (ahora - guardadoEn) / 60_000L
        return when {
            minutos < 1L -> "recién"
            minutos == 1L -> "hace 1 minuto"
            minutos < 60L -> "hace $minutos minutos"
            minutos < 120L -> "hace 1 hora"
            minutos < 24L * 60L -> "hace ${minutos / 60L} horas"
            minutos < 48L * 60L -> "de ayer"
            else -> "de hace ${minutos / (24L * 60L)} días"
        }
    }
}
