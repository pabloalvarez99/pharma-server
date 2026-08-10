package cl.rutbusiness.app.ui.impresora

import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.core.pos.MedioDePago
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Todo lo que va en el papel, tal como lo manda el server.
 *
 * Es el espejo completo de `ReceiptDto` (`crates/domain/src/sales/model.rs`),
 * incluidos los tres campos que `ComprobanteDto` no declara —`datetime`,
 * `card_amount` y `cashier`—, porque la boleta del escritorio los imprime y
 * ésta tiene que salir igual.
 *
 * Los montos viajan como texto decimal y se escriben con la [Moneda] del
 * tenant. **Acá no se calcula ni un peso**: el vuelto, el subtotal y el total
 * los mandó el server. Si el papel dijera un número distinto al que se cobró,
 * el papel sería la prueba de un error.
 */
@Serializable
data class BoletaImprimible(
    @SerialName("order_id") val orderId: String = "",
    @SerialName("folio_or_number") val folio: String = "",
    /** RFC 3339 UTC, como lo serializa `chrono`. */
    val datetime: String = "",
    @SerialName("tenant_name") val tenantName: String = "",
    val items: List<LineaDeBoleta> = emptyList(),
    val subtotal: String = "0",
    val discount: String = "0",
    val total: String = "0",
    @SerialName("payment_method") val paymentMethod: String = "",
    @SerialName("cash_amount") val cashAmount: String? = null,
    @SerialName("card_amount") val cardAmount: String? = null,
    val change: String? = null,
    val cashier: String? = null,
    @SerialName("footer_note") val footerNote: String = "",
)

@Serializable
data class LineaDeBoleta(
    val name: String = "",
    val qty: Long = 0,
    @SerialName("unit_price") val unitPrice: String = "0",
    @SerialName("line_total") val lineTotal: String = "0",
)

/**
 * Arma los bytes de la boleta.
 *
 * Port de `build_ticket_bytes` en `client/src-tauri/src/commands/print.rs`, con
 * el mismo orden y los mismos rótulos: encabezado, ítems, total en grande,
 * cómo pagó, vuelto, quién atendió, pie y corte.
 *
 * **Una desviación deliberada**, y va anotada porque es la única: el escritorio
 * traduce sólo `pos_cash`, `pos_card` y `pos_mixed`, y para cualquier otro
 * código imprime el código crudo. Eso hoy pondría "pos_transferencia" en la
 * boleta de un cliente. Acá se traducen también los medios que el server sumó
 * después ([MedioDePago], `pos_debit`, `pos_credit`, `pos_fiado`). No es un
 * formato nuevo: es el mismo formato sin el agujero.
 *
 * @param desfaseMinutos minutos de diferencia entre UTC y la hora del teléfono,
 *   para que la boleta lleve la hora del negocio y no la de Greenwich.
 * @param conCajon agrega el pulso de apertura al final. Opcional y apagado por
 *   defecto: la mayoría de los negocios no tiene cajón.
 */
internal fun bytesDeBoleta(
    boleta: BoletaImprimible,
    moneda: Moneda,
    ancho: AnchoDePapel,
    desfaseMinutos: Int,
    conCajon: Boolean = false,
): ByteArray {
    val b = ConstructorDeBoleta(ancho)

    b.negrita(boleta.tenantName)
    b.centrada("Boleta N° ${boleta.folio}")
    b.centrada(fechaHoraLegible(boleta.datetime, desfaseMinutos))
    b.separador()

    boleta.items.forEach { linea ->
        b.item(
            nombre = linea.name,
            cantidad = linea.qty.toString(),
            unitario = moneda.formatear(linea.unitPrice),
            total = moneda.formatear(linea.lineTotal),
        )
    }

    b.separador()

    // Mismo criterio que el escritorio: el descuento sólo aparece si lo hubo.
    // Una línea "Descuento $0" en cada boleta gasta papel y confunde.
    if (!esCeroDelServidor(boleta.discount)) {
        b.fila("Descuento", "-${moneda.formatear(boleta.discount)}")
    }

    imprimirTotal(b, moneda.formatear(boleta.total), ancho)
    b.fila("Pago", etiquetaDeMedioDePago(boleta.paymentMethod))

    boleta.cashAmount?.let { b.fila("Recibido", moneda.formatear(it)) }
    boleta.cardAmount?.let { b.fila("Tarjeta", moneda.formatear(it)) }
    boleta.change?.let { b.fila("Vuelto", moneda.formatear(it)) }
    boleta.cashier?.takeIf { it.isNotBlank() }?.let { b.fila("Atendió", it) }

    b.separador()
    if (boleta.footerNote.isNotBlank()) {
        b.centrada(boleta.footerNote)
    }

    b.cortar()
    if (conCajon) b.abrirCajon()

    return b.construir()
}

/**
 * El total, siempre entero.
 *
 * A doble ancho entran la mitad de las columnas: **16** en un rollo de 58 mm.
 * "TOTAL $5.370" entra justo, pero "TOTAL S/ 5.370,50" no, y el escritorio ahí
 * corta — imprimiría `TOTAL S/ 5.370,5`, que es otro monto. Un número cortado
 * en una boleta es peor que un número más chico: el papel es la prueba de lo
 * que se cobró.
 *
 * Por eso hay tres escalones, en orden de preferencia y sin perder nunca un
 * dígito: todo junto en grande; el rótulo en tamaño normal y el monto solo en
 * grande; y si ni el monto solo entra a doble ancho, la fila normal con puntos.
 * El tercero sólo puede pasar con montos de ocho cifras en un rollo angosto.
 */
private fun imprimirTotal(b: ConstructorDeBoleta, monto: String, ancho: AnchoDePapel) {
    val columnasEnGrande = ancho.columnas / 2
    val juntos = "TOTAL $monto"

    when {
        juntos.length <= columnasEnGrande -> b.grande(juntos)
        monto.length <= columnasEnGrande -> {
            b.izquierda("TOTAL")
            b.grande(monto)
        }
        else -> b.fila("TOTAL", monto)
    }
}

/** El código del server dicho como lo diría una persona. */
internal fun etiquetaDeMedioDePago(codigo: String): String =
    MedioDePago.entries.firstOrNull { it.codigo == codigo }?.etiqueta
        ?: when (codigo) {
            "pos_card" -> "Tarjeta"
            "pos_debit" -> "Tarjeta de débito"
            "pos_credit" -> "Tarjeta de crédito"
            "pos_mixed" -> "Mixto"
            else -> codigo
        }

/**
 * Si un monto del server es cero, preguntándole al valor y no al texto.
 *
 * El server manda `"0"`, `"0.00"` o `"0.0000"` para la misma nada según de qué
 * consulta salga; comparar cadenas falla en dos de los tres casos y la boleta
 * saldría con una línea de descuento que no existe.
 */
private fun esCeroDelServidor(monto: String): Boolean =
    monto.trim().removePrefix("-").all { it == '0' || it == '.' } && monto.any { it.isDigit() }

/**
 * `2026-08-07T00:25:31.123Z` a `06-08-2026 21:25`.
 *
 * En la hora del teléfono, no en UTC: la boleta la lee una persona parada en
 * el mostrador, y una boleta de las 21:25 fechada a las 00:25 del día
 * siguiente es un problema cuando alguien la va a cambiar.
 *
 * Aritmética a mano y no `java.time` porque `minSdk` es 21 y `java.time` llega
 * en 26. Si el texto no se entiende, se devuelve tal cual: una fecha rara en
 * el papel es mejor que una boleta que no salió.
 */
internal fun fechaHoraLegible(rfc3339: String, desfaseMinutos: Int): String {
    val segundos = epochSegundosDeRfc3339(rfc3339) ?: return rfc3339
    val locales = segundos + desfaseMinutos * 60L

    val dias = Math.floorDiv(locales, 86_400L)
    val enElDia = locales - dias * 86_400L
    val (anio, mes, dia) = fechaCivil(dias)

    fun dos(n: Long) = n.toString().padStart(2, '0')
    return "${dos(dia)}-${dos(mes)}-$anio ${dos(enElDia / 3600)}:${dos((enElDia / 60) % 60)}"
}

/**
 * Lee un RFC 3339 UTC sin traer una librería de fechas.
 *
 * Sólo acepta el sufijo `Z`, que es lo único que `chrono` serializa para un
 * `DateTime<Utc>`. Cualquier otra cosa devuelve `null` y la boleta imprime el
 * texto crudo.
 */
private fun epochSegundosDeRfc3339(texto: String): Long? {
    val limpio = texto.trim()
    if (limpio.length < 20 || limpio[10] != 'T' || !limpio.endsWith("Z")) return null

    val anio = limpio.substring(0, 4).toLongOrNull() ?: return null
    val mes = limpio.substring(5, 7).toLongOrNull() ?: return null
    val dia = limpio.substring(8, 10).toLongOrNull() ?: return null
    val hora = limpio.substring(11, 13).toLongOrNull() ?: return null
    val minuto = limpio.substring(14, 16).toLongOrNull() ?: return null
    val segundo = limpio.substring(17, 19).toLongOrNull() ?: return null

    return diasDeEpoca(anio, mes, dia) * 86_400L + hora * 3600L + minuto * 60L + segundo
}

/** `days_from_civil` de Howard Hinnant, el mismo algoritmo que usa `chrono`. */
private fun diasDeEpoca(anio: Long, mes: Long, dia: Long): Long {
    val y = if (mes <= 2) anio - 1 else anio
    val era = Math.floorDiv(y, 400L)
    val anioDeLaEra = y - era * 400L
    val diaDelAnio = (153L * (if (mes > 2) mes - 3 else mes + 9) + 2L) / 5L + dia - 1L
    val diaDeLaEra = anioDeLaEra * 365L + anioDeLaEra / 4L - anioDeLaEra / 100L + diaDelAnio
    return era * 146_097L + diaDeLaEra - 719_468L
}

/** `civil_from_days`, la vuelta del anterior. */
private fun fechaCivil(diasDeEpoca: Long): Triple<Long, Long, Long> {
    val z = diasDeEpoca + 719_468L
    val era = Math.floorDiv(z, 146_097L)
    val diaDeLaEra = z - era * 146_097L
    val anioDeLaEra =
        (diaDeLaEra - diaDeLaEra / 1_460L + diaDeLaEra / 36_524L - diaDeLaEra / 146_096L) / 365L
    val anio = anioDeLaEra + era * 400L
    val diaDelAnio = diaDeLaEra - (365L * anioDeLaEra + anioDeLaEra / 4L - anioDeLaEra / 100L)
    val mesCorrido = (5L * diaDelAnio + 2L) / 153L
    val dia = diaDelAnio - (153L * mesCorrido + 2L) / 5L + 1L
    val mes = if (mesCorrido < 10L) mesCorrido + 3L else mesCorrido - 9L
    return Triple(if (mes <= 2L) anio + 1L else anio, mes, dia)
}
