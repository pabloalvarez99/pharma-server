package cl.rutbusiness.app.ui.common

import cl.rutbusiness.core.money.Dinero

/**
 * Los montos que escribe la dueña, en el camino de ida al server.
 *
 * Nada de acá calcula plata: se filtra lo que no es un monto y se traduce el
 * separador. El número que viaja es dígito por dígito el que se tipeó, sin
 * redondeo y sin cambio de escala — es lo que hace que la moneda del tenant
 * pueda tener 0 o 2 decimales sin que estas pantallas sepan cuál es.
 *
 * Vive acá y no en cada `ViewModel` porque la caja y el fiado tienen los mismos
 * campos de plata: dos copias de este filtro se desincronizan, y la que se quede
 * vieja va a mandarle al server un decimal que no puede leer.
 */

/**
 * Lo que se deja tipear en un campo de plata.
 *
 * El teclado numérico de Android igual ofrece símbolos según el fabricante, y un
 * `$` o un espacio llegan al server como un decimal inválido. Se aceptan los dos
 * separadores porque en el teclado latino la coma es la que está a mano.
 */
fun soloPlata(escrito: String): String =
    escrito.filter { it.isDigit() || it == '.' || it == ',' }

/**
 * El texto tal como lo entiende el server, o `null` si no es un decimal.
 *
 * La coma se pasa a punto porque `rust_decimal` sólo lee punto. Que devuelva
 * `null` y no un cero es deliberado: un cero inventado se cobra.
 */
fun montoParaElServidor(escrito: String): String? {
    val normalizado = escrito.trim().replace(',', '.')
    // Al menos un dígito. `Dinero` lee un punto suelto como cero -- su parser
    // rellena la parte entera vacía con "0" --, y un cero que nadie escribió es
    // exactamente lo que no puede pasar con plata: ese cero se abre como caja
    // vacía o se cobra.
    if (normalizado.none { it.isDigit() }) return null
    return if (Dinero.deTextoDeServidor(normalizado) != null) normalizado else null
}

/** Si lo escrito es un monto mayor que cero. Comparar no es calcular. */
fun esMasQueCero(escrito: String): Boolean {
    val monto = montoParaElServidor(escrito) ?: return false
    val leido = Dinero.deTextoDeServidor(monto) ?: return false
    return leido.unidades > 0L
}
