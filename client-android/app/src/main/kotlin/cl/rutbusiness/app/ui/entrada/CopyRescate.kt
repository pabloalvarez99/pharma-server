package cl.rutbusiness.app.ui.entrada

/**
 * Copy de la pantalla de rescate (ADR-0023): teléfono nuevo + tarjeta de papel.
 *
 * Puro y testeable. Con `pideDireccion=false` (nube / APK de feria) **nunca**
 * nombra computador, IP, servidor, lab ni Hetzner: habla de puesto/negocio,
 * tarjeta y 12 palabras. Con dirección on-prem sí se puede decir computador.
 */
internal data class CopyRescate(
    val tituloBarra: String,
    val subtituloBarra: String,
    val tituloCartel: String,
    val cuerpoCartel: String,
    val tituloCampos: String,
    val labelNombre: String,
    val ayudaNombre: String,
    /** `null` cuando la URL ya viene del APK: no se muestra el campo. */
    val labelDireccion: String?,
    val placeholderDireccion: String?,
    val ayudaDireccion: String?,
    val tituloPalabras: String,
    val labelPalabras: String,
    val ayudaPalabras: String,
    val avisoPrivacidad: String,
    val ctaRescatar: String,
    val ctaBuscando: String,
    val ctaEntrar: String,
    val tituloFalla: String,
    val tituloListo: String,
    /** URL inválida o vacía al rescatar. */
    val errorDireccion: String,
)

/**
 * @param pideDireccion true = on-prem LAN (campo de computador/IP).
 * @param esFeria true = vocabulario de feria ("puesto").
 */
internal fun copyRescate(
    pideDireccion: Boolean,
    esFeria: Boolean,
): CopyRescate {
    val cosa = if (esFeria) "puesto" else "negocio"
    return CopyRescate(
        tituloBarra = "Recuperar mi $cosa",
        subtituloBarra = "Con la tarjeta de papel",
        tituloCartel = "Si perdiste el teléfono o lo cambiaste",
        cuerpoCartel = if (esFeria) {
            "Con la tarjeta de papel del cuaderno volvés a tu puesto. " +
                "No hace falta tu clave: las 12 palabras son la llave."
        } else {
            "Con la tarjeta que la app te pidió anotar volvés a tu $cosa. " +
                "No hace falta tu clave: la tarjeta es la llave."
        },
        tituloCampos = if (pideDireccion) {
            "¿Dónde está tu $cosa?"
        } else {
            "¿Cómo se llama tu $cosa?"
        },
        labelNombre = "Nombre corto del $cosa",
        ayudaNombre = "El que está escrito arriba en la tarjeta.",
        labelDireccion = if (pideDireccion) "Dirección del computador" else null,
        placeholderDireccion = if (pideDireccion) "192.168.1.10:8080" else null,
        ayudaDireccion = if (pideDireccion) {
            "La misma que usabas para entrar."
        } else {
            null
        },
        tituloPalabras = "Las 12 palabras de la tarjeta",
        labelPalabras = "Las 12 palabras (o los 5 bloques)",
        ayudaPalabras = "Copialas separadas por espacios, en el mismo orden. " +
            "No importan mayúsculas ni tildes.",
        avisoPrivacidad = "La tarjeta no sale de este teléfono. Lo que viaja es una prueba " +
            "derivada de ella, que sólo sirve para pedir tu paquete y no para abrirlo.",
        ctaRescatar = "Traer mi respaldo",
        ctaBuscando = "Buscando tu respaldo…",
        ctaEntrar = "Entrar a mi $cosa",
        tituloFalla = "No se pudo traer el respaldo",
        tituloListo = "Listo",
        errorDireccion = if (pideDireccion) {
            "Revisá la dirección: algo como 192.168.1.10:8080 o app.rutbusiness.cl."
        } else {
            "RutAgent no responde. Reintentá en un momento."
        },
    )
}
