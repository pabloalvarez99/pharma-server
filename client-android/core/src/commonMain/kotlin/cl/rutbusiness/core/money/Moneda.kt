package cl.rutbusiness.core.money

/**
 * La moneda del negocio y cómo se escribe su plata.
 *
 * No hay ningún "CLP" clavado en las pantallas: el código sale del server
 * (`GET /api/v1/settings/money.currency`) y de ahí se derivan los decimales.
 * Un tenant en Perú tiene que ver `S/ 12,50` con dos decimales y uno en Chile
 * `$1.490` sin ninguno, con el mismo código de UI.
 */
data class Moneda(
    /** ISO-4217: `CLP`, `USD`, `PEN`. */
    val codigo: String,
    /** Decimales que la moneda usa de verdad. */
    val decimales: Int,
    /** Lo que la gente escribe adelante del número. */
    val simbolo: String,
) {
    /**
     * Escribe un monto que vino del server.
     *
     * Dos cuidados que parecen detalle y no lo son:
     *
     * - **Nunca se recorta un dígito distinto de cero.** Si la moneda declara 0
     *   decimales y el server igual mandó `1490.50`, se muestran los 50: cortar
     *   ahí sería mostrar en pantalla un número que no es el que se cobra.
     * - Si el texto no se entiende, se devuelve tal cual en vez de un `$0`.
     *   Un cero inventado es peor que un número raro: el cero se cobra.
     */
    fun formatear(montoDeServidor: String): String {
        val dinero = Dinero.deTextoDeServidor(montoDeServidor) ?: return montoDeServidor
        return formatear(dinero)
    }

    fun formatear(monto: Dinero): String {
        val texto = monto.aTextoDeServidor()
        val negativo = texto.startsWith('-')
        val sinSigno = texto.removePrefix("-")

        val entera = sinSigno.substringBefore('.')
        val decimalCruda = sinSigno.substringAfter('.', "")
        // Se rellena hasta los decimales de la moneda, y se estira más si el
        // server mandó dígitos significativos de más.
        val significativa = decimalCruda.trimEnd('0')
        val largo = maxOf(decimales, significativa.length)
        val decimal = decimalCruda.padEnd(largo, '0').take(largo)

        val agrupada = entera
            .reversed()
            .chunked(3)
            .joinToString(SEPARADOR_MILES)
            .reversed()

        val cuerpo = if (decimal.isEmpty()) agrupada else agrupada + SEPARADOR_DECIMAL + decimal
        val signo = if (negativo) "-" else ""
        return signo + simbolo + cuerpo
    }

    companion object {
        // Formato es-CL: punto para los miles, coma para los decimales. Es lo
        // que lee el operador en Chile y en casi toda la región.
        private const val SEPARADOR_MILES = "."
        private const val SEPARADOR_DECIMAL = ","

        /** Monedas sin decimales (ISO-4217 exponente 0). */
        private val SIN_DECIMALES = setOf(
            "CLP", "JPY", "KRW", "PYG", "VND", "ISK", "UGX", "RWF",
            "XAF", "XOF", "XPF", "GNF", "KMF", "VUV", "BIF", "DJF",
        )

        /** Monedas de tres decimales (exponente 3). */
        private val TRES_DECIMALES = setOf("BHD", "IQD", "JOD", "KWD", "LYD", "OMR", "TND")

        private val SIMBOLOS = mapOf(
            "CLP" to "$",
            "ARS" to "$",
            "COP" to "$",
            "MXN" to "$",
            "UYU" to "$",
            "USD" to "US$",
            "EUR" to "€",
            "PEN" to "S/ ",
            "BRL" to "R$ ",
            "BOB" to "Bs ",
            "PYG" to "₲",
        )

        /**
         * Arma la moneda a partir del código que informó el server.
         *
         * Una moneda que no conocemos cae en **2 decimales**, nunca en 0:
         * mostrar de menos parte el monto a la mitad, mostrar de más sólo se ve
         * raro.
         */
        fun de(codigo: String): Moneda {
            val iso = codigo.trim().uppercase()
            val decimales = when (iso) {
                in SIN_DECIMALES -> 0
                in TRES_DECIMALES -> 3
                else -> 2
            }
            return Moneda(
                codigo = iso,
                decimales = decimales,
                simbolo = SIMBOLOS[iso] ?: "$iso ",
            )
        }

        /**
         * Lo que se usa mientras el server no diga otra cosa.
         *
         * El endpoint `money.currency` existe recién con el country pack; hasta
         * que esa rama llegue, el server contesta 404 y **este** es el valor que
         * se aplica. Está acá y no repartido por las pantallas justamente para
         * que el día que el server hable, no haya nada que cazar.
         *
         * Va declarado **al final** del companion a propósito: las propiedades
         * de un `companion object` se inicializan en orden de declaración, y
         * arriba de las tablas esto arrancaba con `SIN_DECIMALES` todavía en
         * `null` y tumbaba la app en el primer frame.
         */
        val POR_DEFECTO = de("CLP")
    }
}
