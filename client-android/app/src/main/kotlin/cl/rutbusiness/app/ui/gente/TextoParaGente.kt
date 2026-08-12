package cl.rutbusiness.app.ui.gente

/**
 * El texto que la dueña le manda a su gente.
 *
 * Puro y sin `android.*` a propósito (ADR-0021): el mensaje se arma acá y se
 * prueba sin `Activity`, sin `Intent` y sin Robolectric. Lo único que hace
 * Android es abrir el selector con el texto ya escrito — ver
 * [CompartirConGente].
 *
 * **Nada de WhatsApp Business API.** El mensaje sale del teléfono de la dueña,
 * por la app que ella elija en el selector del sistema, desde su propio número.
 * No hay servidor de mensajería, no hay plantillas aprobadas, no hay costo por
 * conversación, y el cliente ve el número de siempre en vez de uno comercial.
 *
 * ## Por qué el monto llega formateado y no como número
 *
 * [mensajeDeuda] tiene dos formas. La que recibe un `String` es la que usan las
 * pantallas, y existe por la regla de plata del proyecto: los montos vienen del
 * server como texto decimal y los escribe la `Moneda` del tenant, que también
 * viene del server. Un negocio que factura en soles tiene que poder mandar el
 * mensaje en soles sin tocar este archivo.
 *
 * La que recibe un `Long` formatea es-CL —miles con punto— y está para el caso
 * en que quien llama ya tiene pesos enteros en la mano. Las dos escriben la
 * misma frase; lo único que cambia es quién formateó el número.
 */

/**
 * Recordatorio de una cuenta pendiente.
 *
 * @param nombre como está escrito en la cuenta. Vacío se tolera: alguien fiado
 *   sin nombre completo existe, y el saludo se acomoda en vez de quedar
 *   `"Hola , ..."`.
 * @param monto ya formateado con la [cl.rutbusiness.core.money.Moneda] del
 *   negocio, tal cual lo muestra la pantalla. No se recalcula ni se reescribe:
 *   el mensaje que sale por el chat dice exactamente el número que la dueña
 *   estaba mirando.
 */
fun mensajeDeuda(nombre: String, monto: String): String =
    saludo(nombre) + "me debes $monto."

/**
 * Ídem, cuando quien llama tiene pesos enteros y no un monto ya escrito.
 * Formatea es-CL: `5000` queda `$5.000`.
 */
fun mensajeDeuda(nombre: String, pesos: Long): String =
    mensajeDeuda(nombre, pesosEsCl(pesos))

/**
 * Una línea para contar cómo fue el día.
 *
 * @param resumenCorto lo que la pantalla ya muestra, **ya formateado**. Esta
 *   función no suma, no convierte y no infiere: si inventara un monto, el
 *   número del chat y el de la pantalla podrían discrepar, y el que la gente
 *   guarda es el del chat.
 */
fun mensajeHoy(resumenCorto: String): String {
    val limpio = resumenCorto.trim()
    return if (limpio.isEmpty()) "Hoy en el negocio." else "Hoy en el negocio: $limpio"
}

/**
 * `"Hola Don Juan, "` — o `"Hola, "` si no hay nombre.
 *
 * Se recorta el nombre porque llega de un campo escrito a mano en el mostrador,
 * donde el espacio de más es la norma y no la excepción.
 */
private fun saludo(nombre: String): String {
    val limpio = nombre.trim()
    return if (limpio.isEmpty()) "Hola, " else "Hola $limpio, "
}

/**
 * Un entero de pesos escrito como se lee en Chile: miles con punto.
 *
 * Es el mismo criterio que usa `Moneda` (`SEPARADOR_MILES = "."`), replicado
 * acá para los pesos enteros. Se agrupa sobre el valor absoluto y el signo se
 * pega al final: agrupar con el `-` adentro mete un punto después del signo en
 * cuanto el número pasa de cuatro dígitos.
 */
internal fun pesosEsCl(pesos: Long): String {
    val negativo = pesos < 0
    // `Long.MIN_VALUE` no tiene positivo que lo represente, así que se agrupa
    // sobre el texto sin signo en vez de sobre `-pesos`.
    val digitos = pesos.toString().removePrefix("-")

    val agrupado = digitos
        .reversed()
        .chunked(3)
        .joinToString(".")
        .reversed()

    val conSimbolo = "\$$agrupado"
    return if (negativo) "-$conSimbolo" else conSimbolo
}
