package cl.rutbusiness.app.ui.scanner

/**
 * Decide si una lectura de la cámara es un producto nuevo o el mismo de recién.
 *
 * El detector corre sobre cada frame: un tarro quieto frente al lente reporta
 * su EAN treinta veces por segundo. Sin este filtro, apoyar el producto un
 * segundo en el mostrador le cobraría treinta unidades al cliente.
 *
 * **La regla no es "un código cada X ms".** Es "el código tiene que
 * desaparecer del encuadre antes de volver a contar": cada avistamiento
 * -aceptado o no- refresca el reloj de ese código. Así,
 *
 * - dejar el producto quieto frente al lente cuenta **una** vez, dure lo que
 *   dure;
 * - retirarlo y volver a pasarlo cuenta **dos**, que es cómo la cajera carga
 *   dos unidades iguales sin buscar el botón de cantidad;
 * - dos productos distintos a la vista se cuentan uno cada uno, sin que el
 *   segundo desbloquee al primero.
 *
 * Un simple "último código + ventana fija" falla en los tres casos.
 */
class AntiRebote(private val ventanaMs: Long = VENTANA_MS) {

    /** Código → cuándo se lo vio por última vez. Acotado por [purgar]. */
    private val vistos = LinkedHashMap<String, Long>()

    /**
     * `true` si [codigo] tiene que entrar al carrito.
     *
     * @param ahoraMs reloj **monótono** en milisegundos. Monótono y no de pared
     *   porque un ajuste de hora del sistema en medio de una venta no puede
     *   duplicar una línea.
     */
    fun aceptar(codigo: String, ahoraMs: Long): Boolean {
        purgar(ahoraMs)
        val visto = vistos.put(codigo, ahoraMs)
        return visto == null
    }

    /** Vuelve al estado inicial: se usa al abrir la cámara para otra venta. */
    fun olvidar() = vistos.clear()

    private fun purgar(ahoraMs: Long) {
        vistos.entries.removeAll { ahoraMs - it.value >= ventanaMs }
    }

    companion object {
        /**
         * Cuánto tiene que estar fuera de cuadro un código para volver a contar.
         *
         * 900 ms es más que el hueco entre frames del peor aparato (a 10 fps son
         * 100 ms) y menos que lo que tarda una persona en retirar un producto y
         * pasar el siguiente. Más corto duplica líneas; más largo hace que
         * pasar dos unidades iguales se sienta roto.
         */
        const val VENTANA_MS = 900L
    }
}
