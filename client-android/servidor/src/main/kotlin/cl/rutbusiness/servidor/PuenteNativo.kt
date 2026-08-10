package cl.rutbusiness.servidor

/**
 * Puente JNI del server embebido. Solo ciclo de vida / probes (ADR-0021).
 *
 * H3: un string. H4+: probe KV y arranque/parada cuando el seam de `api` esté
 * en main.
 */
object PuenteNativo {
    init {
        System.loadLibrary("servidor_android")
    }

    /**
     * H3: devuelve un string fijo desde el .so (`"h3-ok"`).
     * Prueba build → jniLibs → dlopen → JNI sin server.
     */
    @JvmStatic
    external fun nativeSaludo(): String
}
