package cl.rutbusiness.app.ui.impresora

import cl.rutbusiness.core.money.Moneda
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * La boleta entera, línea por línea.
 *
 * Es el papel que se lleva el cliente: si una cifra sale distinta de la que se
 * cobró, el papel es la prueba de un error que nadie va a poder explicar en el
 * mostrador. Por eso se verifica el texto exacto y no "que imprima algo".
 */
class BoletaTest {

    private val boleta = BoletaImprimible(
        orderId = "order:abc",
        folio = "1042",
        datetime = "2026-08-07T00:25:31.123Z",
        tenantName = "Almacén Doña Rosa",
        items = listOf(
            LineaDeBoleta(name = "Arroz Grado 1", qty = 2, unitPrice = "1290", lineTotal = "2580"),
            LineaDeBoleta(name = "Aceite 1L", qty = 1, unitPrice = "2790", lineTotal = "2790"),
        ),
        subtotal = "5370",
        discount = "0",
        total = "5370",
        paymentMethod = "pos_cash",
        cashAmount = "10000",
        change = "4630",
        cashier = "Rosa",
        footerNote = "Gracias por su compra",
    )

    /** Chile: UTC-4. Es el desfase con el que se leería en el mostrador. */
    private val chile = -4 * 60

    private fun lineas(
        b: BoletaImprimible = boleta,
        ancho: AnchoDePapel = AnchoDePapel.Mm58,
        moneda: Moneda = Moneda.POR_DEFECTO,
    ): List<String> =
        loQueSaleEnElPapel(bytesDeBoleta(b, moneda, ancho, desfaseMinutos = chile))
            .split("\n")
            .map { it.trim() }
            .filter { it.isNotEmpty() }

    /**
     * Lee la boleta como la lee una persona: sólo el texto.
     *
     * Salta las secuencias de control **por largo**, no filtrando bytes bajos:
     * el parámetro de `ESC a 1` es imprimible y colarlo pondría una "a" al
     * principio de cada línea centrada. De paso, si alguien inventa una
     * secuencia que este lector no conoce, la prueba se cae — que es lo que
     * queremos, porque una secuencia rara es papel malgastado o una impresora
     * confundida.
     */
    private fun loQueSaleEnElPapel(bytes: ByteArray): String {
        val texto = StringBuilder()
        var i = 0
        while (i < bytes.size) {
            val b = bytes[i].toInt() and 0xFF
            val siguiente = (bytes.getOrNull(i + 1)?.toInt() ?: 0) and 0xFF
            val largo = when {
                b == 0x1B && siguiente == '@'.code -> 2
                b == 0x1B && siguiente == 'a'.code -> 3
                b == 0x1B && siguiente == 'E'.code -> 3
                b == 0x1B && siguiente == 'p'.code -> 5
                b == 0x1D && siguiente == '!'.code -> 3
                b == 0x1D && siguiente == 'V'.code -> 3
                b == 0x10 && siguiente == 0x04 -> 3
                b == 0x1B || b == 0x1D || b == 0x10 ->
                    throw AssertionError("secuencia de control desconocida en $i")
                else -> 0
            }
            if (largo > 0) {
                i += largo
            } else {
                texto.append(desdeLaPaginaDeCodigos(b))
                i++
            }
        }
        return texto.toString()
    }

    /** La vuelta de la traducción que hace `ConstructorDeBoleta`. */
    private fun desdeLaPaginaDeCodigos(byte: Int): Char = when (byte) {
        0xA0 -> 'á'
        0x82 -> 'é'
        0xA1 -> 'í'
        0xA2 -> 'ó'
        0xA3 -> 'ú'
        0xA4 -> 'ñ'
        0xA5 -> 'Ñ'
        0x81 -> 'ü'
        0xB5 -> 'Á'
        0xF8 -> '°'
        else -> byte.toChar()
    }

    @Test
    fun `el encabezado trae negocio, folio y fecha`() {
        val l = lineas()
        assertEquals("Almacén Doña Rosa", l[0])
        assertEquals("Boleta N° 1042", l[1])
        // 00:25 UTC del 7 es 20:25 del 6 en Chile: la boleta lleva la hora del
        // negocio, no la de Greenwich. Fecharla un día adelante es un problema
        // el día que alguien viene a cambiar el producto.
        assertEquals("06-08-2026 20:25", l[2])
    }

    @Test
    fun `cada item sale con su nombre y su linea de cantidad`() {
        val l = lineas()
        assertTrue("falta el nombre del producto", l.contains("Arroz Grado 1"))
        val linea = l.first { it.startsWith("2 x") }
        assertTrue("«$linea» tiene que llevar el precio unitario", linea.startsWith("2 x \$1.290"))
        assertTrue("«$linea» tiene que cerrar con el total de la línea", linea.endsWith("\$2.580"))
    }

    @Test
    fun `el total va en grande y los pagos abajo`() {
        val l = lineas()
        assertTrue("falta el total", l.any { it == "TOTAL \$5.370" })
        assertTrue("falta el medio de pago", l.any { it.startsWith("Pago") && it.endsWith("Efectivo") })
        assertTrue("falta lo recibido", l.any { it.startsWith("Recibido") && it.endsWith("\$10.000") })
        assertTrue("falta el vuelto", l.any { it.startsWith("Vuelto") && it.endsWith("\$4.630") })
        assertTrue("falta quién atendió", l.any { it.startsWith("Atendió") && it.endsWith("Rosa") })
        assertTrue("falta el pie", l.contains("Gracias por su compra"))
    }

    /** Sin descuento no se imprime una línea de descuento. Gasta papel y confunde. */
    @Test
    fun `un descuento en cero no ocupa una linea`() {
        assertFalse(lineas().any { it.startsWith("Descuento") })
        assertFalse(lineas(boleta.copy(discount = "0.00")).any { it.startsWith("Descuento") })
    }

    @Test
    fun `un descuento de verdad sale con signo menos`() {
        val l = lineas(boleta.copy(discount = "500"))
        val linea = l.first { it.startsWith("Descuento") }
        assertTrue("«$linea» tiene que restar", linea.endsWith("-\$500"))
    }

    /**
     * El escritorio imprime el código crudo para todo lo que no sea efectivo,
     * tarjeta o mixto. Eso hoy pondría "pos_transferencia" en la boleta de un
     * cliente, que es la única desviación deliberada del formato portado.
     */
    @Test
    fun `los medios que el escritorio no conoce igual salen en castellano`() {
        assertTrue(
            lineas(boleta.copy(paymentMethod = "pos_transferencia"))
                .any { it.startsWith("Pago") && it.endsWith("Transferencia") },
        )
        assertTrue(
            lineas(boleta.copy(paymentMethod = "pos_fiado"))
                .any { it.startsWith("Pago") && it.endsWith("Fiado") },
        )
        assertTrue(
            lineas(boleta.copy(paymentMethod = "pos_debit"))
                .any { it.startsWith("Pago") && it.endsWith("Tarjeta de débito") },
        )
    }

    /** Sin vuelto y sin cajero, esas filas no existen: no salen vacías. */
    @Test
    fun `lo que el server no manda no ocupa una fila`() {
        val l = lineas(boleta.copy(change = null, cashier = null, cashAmount = null))
        assertFalse(l.any { it.startsWith("Vuelto") })
        assertFalse(l.any { it.startsWith("Atendió") })
        assertFalse(l.any { it.startsWith("Recibido") })
    }

    /**
     * La moneda es por tenant y la boleta la respeta.
     *
     * Un negocio en soles imprime `S/ 5.370,50`, con dos decimales, sin que
     * nadie toque este archivo. Si acá hubiera un "CLP" clavado, el papel
     * diría un monto distinto al que cobró el server.
     */
    @Test
    fun `una moneda con decimales se imprime con sus decimales`() {
        val l = lineas(boleta.copy(total = "5370.50"), moneda = Moneda.de("PEN"))
        assertTrue("falta el total en soles: $l", l.any { it.contains("S/ 5.370,50") })
    }

    /**
     * El total nunca sale cortado, aunque no entre a doble ancho.
     *
     * En 58 mm el tamaño grande deja 16 columnas. "TOTAL S/ 5.370,50" son 17 y
     * el escritorio ahí corta el último dígito: imprimiría `S/ 5.370,5`, que es
     * **otro monto**. Acá el rótulo baja a tamaño normal y el número se queda
     * entero, porque el papel es la prueba de lo que se cobró.
     */
    @Test
    fun `un total que no entra a doble ancho no pierde un digito`() {
        val l = lineas(boleta.copy(total = "5370.50"), moneda = Moneda.de("PEN"))

        assertTrue("el rótulo tiene que quedar en su propia línea", l.contains("TOTAL"))
        assertTrue("el monto tiene que salir entero", l.contains("S/ 5.370,50"))
        assertTrue(
            "ningún dígito puede haberse perdido: $l",
            l.none { it.contains("S/ 5.370,5") && !it.contains("S/ 5.370,50") },
        )
    }

    /** Si entra junto, va junto: es la forma preferida y la del escritorio. */
    @Test
    fun `un total que entra a doble ancho va junto al rotulo`() {
        assertTrue(lineas().any { it == "TOTAL \$5.370" })
    }

    /** Las filas ocupan el ancho del rollo, y son 32 o 48 según el papel. */
    @Test
    fun `la fila de pago llena el ancho del rollo`() {
        val angosta = lineas(ancho = AnchoDePapel.Mm58).first { it.startsWith("Pago") }
        val ancha = lineas(ancho = AnchoDePapel.Mm80).first { it.startsWith("Pago") }
        assertEquals(32, angosta.length)
        assertEquals(48, ancha.length)
    }

    /** El separador también, que es lo que hace que la boleta se vea derecha. */
    @Test
    fun `los separadores miden el ancho del rollo`() {
        assertTrue(lineas(ancho = AnchoDePapel.Mm58).contains("-".repeat(32)))
        assertTrue(lineas(ancho = AnchoDePapel.Mm80).contains("-".repeat(48)))
    }

    @Test
    fun `el cajon solo se abre si se pide`() {
        val sinCajon = bytesDeBoleta(boleta, Moneda.POR_DEFECTO, AnchoDePapel.Mm58, chile)
        val conCajon = bytesDeBoleta(boleta, Moneda.POR_DEFECTO, AnchoDePapel.Mm58, chile, conCajon = true)

        assertFalse(
            "sin pedirlo no puede salir el pulso",
            sinCajon.toList().windowed(5).any { it == EscPos.PULSO_DE_CAJON.toList() },
        )
        assertTrue(
            "pidiéndolo tiene que salir al final",
            conCajon.toList().takeLast(5) == EscPos.PULSO_DE_CAJON.toList(),
        )
    }

    /** Una fecha que no se entiende sale cruda: mejor rara que sin boleta. */
    @Test
    fun `una fecha ilegible no impide imprimir`() {
        assertEquals("ayer", fechaHoraLegible("ayer", chile))
        assertEquals("", fechaHoraLegible("", chile))
    }

    @Test
    fun `la fecha respeta el huso del telefono`() {
        assertEquals("07-08-2026 00:25", fechaHoraLegible("2026-08-07T00:25:31Z", 0))
        assertEquals("06-08-2026 20:25", fechaHoraLegible("2026-08-07T00:25:31Z", -240))
        assertEquals("07-08-2026 02:25", fechaHoraLegible("2026-08-07T00:25:31Z", 120))
    }

    /** El 29 de febrero es donde toda aritmética de fechas propia se cae. */
    @Test
    fun `la fecha aguanta un bisiesto`() {
        assertEquals("29-02-2028 12:00", fechaHoraLegible("2028-02-29T12:00:00Z", 0))
    }
}
