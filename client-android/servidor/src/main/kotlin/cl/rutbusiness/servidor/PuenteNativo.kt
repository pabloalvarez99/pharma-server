package cl.rutbusiness.servidor

/**
 * Puente JNI del server embebido. Solo ciclo de vida / probes (ADR-0021).
 * Nada de dominio: la app habla HTTP al puerto local después de arrancar.
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

    /**
     * H4: abre SurrealKV en [rutaAbsoluta], escribe un marker, cierra, reabre y
     * lee. Devuelve `"PROBE OK marker=… bytes=…"` o `"PROBE FAIL …"`.
     *
     * La ruta debe ser absoluta dentro del sandbox de la app
     * (p. ej. `context.filesDir.resolve("surreal-probe").absolutePath`).
     */
    @JvmStatic
    external fun nativeProbe(rutaAbsoluta: String): String
}
