package cl.rutbusiness.core.money

/**
 * Un monto de dinero tal como lo mandó el server, con su escala intacta.
 *
 * El server serializa `rust_decimal` como texto (`"1490"`, `"12.50"`) justo
 * para que nadie lo meta en un `Double` en el camino. Acá se guarda igual: un
 * entero sin escalar más la cantidad de decimales. Nada toca punto flotante.
 *
 * **Lo que esta clase NO hace**: impuestos, descuentos, redondeo de moneda ni
 * vuelto. Eso lo calcula el server y punto. Acá sólo hay dos operaciones, y
 * existen únicamente para el total que el cajero necesita ver mientras arma el
 * carrito — ver [cl.rutbusiness.core.pos.Carrito.total].
 */
data class Dinero(
    /** El monto sin coma. `12.50` con [escala] 2 es `1250`. */
    val unidades: Long,
    /** Cuántos decimales tiene [unidades]. */
    val escala: Int,
) {
    operator fun plus(otro: Dinero): Dinero {
        val comun = maxOf(escala, otro.escala)
        return Dinero(escalarA(comun).unidades + otro.escalarA(comun).unidades, comun)
    }

    /** Cantidad de un producto: un entero, nunca una fracción. */
    operator fun times(cantidad: Int): Dinero = Dinero(unidades * cantidad, escala)

    fun escalarA(nueva: Int): Dinero {
        if (nueva == escala) return this
        require(nueva > escala) { "escalar hacia abajo perdería dígitos" }
        var u = unidades
        repeat(nueva - escala) { u *= 10 }
        return Dinero(u, nueva)
    }

    /** Vuelve al mismo texto que entiende el server. */
    fun aTextoDeServidor(): String {
        val negativo = unidades < 0
        val digitos = if (unidades < 0) (-unidades).toString() else unidades.toString()
        val signo = if (negativo) "-" else ""
        if (escala == 0) return signo + digitos
        val relleno = digitos.padStart(escala + 1, '0')
        val corte = relleno.length - escala
        return signo + relleno.substring(0, corte) + "." + relleno.substring(corte)
    }

    companion object {
        val CERO = Dinero(0, 0)

        /**
         * Lee lo que mandó el server. Devuelve `null` si no es un decimal, que
         * es la única respuesta honesta: inventar un cero mostraría "$0" donde
         * hay plata de verdad.
         */
        fun deTextoDeServidor(texto: String): Dinero? {
            val limpio = texto.trim()
            if (limpio.isEmpty()) return null

            val negativo = limpio.startsWith('-')
            val sinSigno = limpio.removePrefix("-").removePrefix("+")
            val partes = sinSigno.split('.')
            if (partes.size > 2) return null

            val entera = partes[0].ifEmpty { "0" }
            val decimal = partes.getOrElse(1) { "" }
            if (entera.any { !it.isDigit() } || decimal.any { !it.isDigit() }) return null

            val unidades = (entera + decimal).toLongOrNull() ?: return null
            return Dinero(if (negativo) -unidades else unidades, decimal.length)
        }
    }
}
