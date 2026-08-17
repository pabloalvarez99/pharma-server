package cl.rutbusiness.app.ui.entrada

/**
 * Copy de la tarjeta de rescate (ADR-0022): una hoja del cuaderno, no un
 * export técnico. 12 palabras + bloques + código para escanear después.
 * Sin seed, IP, sistema, servidor ni WhatsApp como CTA principal.
 */
internal data class CopyTarjeta(
    val tituloBarra: String,
    val subtituloBarra: String,
    val tituloHero: String,
    val cuerpoHero: String,
    val tituloPalabras: String,
    val tituloBloques: String,
    val tituloQr: String,
    val ayudaQr: String,
    val pieQrOk: String,
    val pieQrFallo: String,
    val tituloAnotar: String,
    val cuerpoAnotar: String,
    val tituloPagina: String,
    val ctaCopiar: String,
    val ctaCopiado: String,
    val ctaGuardarNota: String,
    val ctaImprimir: String,
    val piePagina: String,
    val ctaListo: String,
    val ctaSeguirSinAnotar: String,
)

/** Copy fijo day-1 feria: puesto + cuaderno. */
internal fun copyTarjeta(): CopyTarjeta = CopyTarjeta(
    tituloBarra = "Tu tarjeta de rescate",
    subtituloBarra = "Escribila en el cuaderno",
    tituloHero = "Esta es la llave de tu puesto",
    cuerpoHero = "Si te roban o se te rompe el teléfono, con esta llave " +
        "y tu cuenta de Google volvés a entrar a tus ventas y deudas. " +
        "Sin ella, el respaldo es basura: nosotros no podemos " +
        "recuperarla por vos.",
    tituloPalabras = "Las 12 palabras",
    tituloBloques = "Bloques (más fáciles con lápiz)",
    tituloQr = "Código para el cuaderno",
    ayudaQr = "Para escanear después. Solo los bloques del código " +
        "(no las 12 palabras). Podés copiarlo al cuaderno. " +
        "No lo mandes por WhatsApp.",
    pieQrOk = "Para escanear después (bloques). Las 12 palabras siguen " +
        "siendo solo del cuaderno.",
    pieQrFallo = "El dibujo no salió. Copiá el texto de abajo al cuaderno.",
    tituloAnotar = "Anotá esto YA en tu cuaderno",
    cuerpoAnotar = "Pegá esta hoja o copiá las palabras. " +
        "Si las perdés, perdés el historial del respaldo. " +
        "No las mandes por WhatsApp.",
    tituloPagina = "Una página para el cuaderno",
    ctaCopiar = "Copiar las palabras",
    ctaCopiado = "Copiado. Pegalo en Notas y anotalo",
    ctaGuardarNota = "Guardar en Notas",
    ctaImprimir = "Imprimir / PDF",
    piePagina = "Imprimí o guardá el PDF y pegalo en el cuaderno. " +
        "Las 12 palabras no van en el código (solo los bloques).",
    ctaListo = "Ya la anoté",
    ctaSeguirSinAnotar = "Seguir sin anotar (no recomendado)",
)
