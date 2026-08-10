package cl.rutbusiness.app.ui.impresora

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Los bytes que salen al papel.
 *
 * Son las mismas pruebas que tiene `client/src-tauri/src/escpos.rs`, portadas
 * una por una y con los mismos números. No es duplicación por gusto: son el
 * contrato de que la boleta del teléfono y la del PC del negocio son la misma
 * boleta. Si un día alguien cambia un ancho de columna de un lado, este archivo
 * es lo que avisa que el otro lado quedó distinto.
 *
 * Corre en la JVM sin impresora y sin teléfono, que es exactamente lo que hace
 * verificable la parte de este encargo que no depende de tener el hardware.
 */
class EscPosTest {

    private fun bytes(ancho: AnchoDePapel, armar: ConstructorDeBoleta.() -> Unit): ByteArray =
        ConstructorDeBoleta(ancho).apply(armar).construir()

    @Test
    fun `la boleta abre con init y cierra con corte`() {
        val salida = bytes(AnchoDePapel.Mm58) { izquierda("hola"); cortar() }

        assertArrayEquals(byteArrayOf(0x1B, '@'.code.toByte()), salida.copyOfRange(0, 2))
        assertArrayEquals(
            byteArrayOf(0x1D, 'V'.code.toByte(), 1),
            salida.copyOfRange(salida.size - 3, salida.size),
        )
    }

    /** 32 columnas en 58 mm: lo que no entra se corta, no se envuelve solo. */
    @Test
    fun `una linea larga se corta al ancho del rollo`() {
        val salida = bytes(AnchoDePapel.Mm58) { izquierda("x".repeat(100)) }
        assertEquals(2 + 32 + 1, salida.size)
    }

    @Test
    fun `en 80 mm entran 48 columnas`() {
        val salida = bytes(AnchoDePapel.Mm80) { izquierda("x".repeat(100)) }
        assertEquals(2 + 48 + 1, salida.size)
    }

    /**
     * Las tildes y la ñ existen en la impresora, pero **no** en la posición de
     * UTF-8. Sin esta traducción, "Ñuñoa" sale como jeroglíficos en la boleta
     * de un cliente.
     */
    @Test
    fun `las tildes se traducen a la pagina de codigos de la impresora`() {
        val salida = bytes(AnchoDePapel.Mm80) { izquierda("áéíñ") }
        assertArrayEquals(
            byteArrayOf(0xA0.toByte(), 0x82.toByte(), 0xA1.toByte(), 0xA4.toByte()),
            salida.copyOfRange(2, 6),
        )
    }

    /** Un carácter que la impresora no tiene degrada, no rompe la boleta. */
    @Test
    fun `un caracter desconocido sale como interrogacion y no tumba la boleta`() {
        val salida = bytes(AnchoDePapel.Mm58) { izquierda("café 中") }
        val texto = salida.copyOfRange(2, salida.size - 1).map { it.toInt() and 0xFF }
        assertTrue("el ideograma tenía que degradar a '?'", texto.contains('?'.code))
    }

    @Test
    fun `la fila rellena el ancho completo con puntos`() {
        val salida = bytes(AnchoDePapel.Mm58) { fila("TOTAL", "$1.000") }
        val cuerpo = salida.copyOfRange(2, salida.size - 1)
        val texto = cuerpo.map { (it.toInt() and 0xFF).toChar() }.joinToString("")

        assertEquals(32, texto.length)
        assertTrue("«$texto» tiene que empezar con la etiqueta", texto.startsWith("TOTAL"))
        assertTrue("«$texto» tiene que terminar con el monto", texto.endsWith("$1.000"))
    }

    /** Etiqueta y valor más largos que el rollo: al menos un punto de separación. */
    @Test
    fun `una fila que no entra no se queda sin separador`() {
        val salida = bytes(AnchoDePapel.Mm58) { fila("E".repeat(20), "V".repeat(20)) }
        val texto = salida.copyOfRange(2, salida.size - 1)
            .map { (it.toInt() and 0xFF).toChar() }
            .joinToString("")
        assertEquals(32, texto.length)
        assertTrue("tiene que quedar el punto separador", texto.contains("."))
    }

    @Test
    fun `el total va en doble tamano y a la mitad de columnas`() {
        val salida = bytes(AnchoDePapel.Mm58) { grande("T".repeat(40)) }

        assertTrue(
            "falta GS ! 0x11, que es lo que agranda el total",
            salida.toList().windowed(3).any {
                it == listOf<Byte>(0x1D, '!'.code.toByte(), 0x11)
            },
        )
        // 16 columnas al doble de ancho, no 32.
        val letras = salida.count { it == 'T'.code.toByte() }
        assertEquals(16, letras)
    }

    @Test
    fun `el pulso del cajon es ESC p con la duracion estandar`() {
        assertArrayEquals(
            byteArrayOf(0x1B, 'p'.code.toByte(), 0, 25, 250.toByte()),
            EscPos.PULSO_DE_CAJON,
        )

        val salida = bytes(AnchoDePapel.Mm58) { abrirCajon() }
        assertArrayEquals(EscPos.PULSO_DE_CAJON, salida.copyOfRange(2, salida.size))
    }

    /** El corte deja pasar papel primero: si no, la cuchilla se come el pie. */
    @Test
    fun `el corte viene despues de tres avances`() {
        val salida = bytes(AnchoDePapel.Mm58) { cortar() }
        assertArrayEquals(
            byteArrayOf('\n'.code.toByte(), '\n'.code.toByte(), '\n'.code.toByte()),
            salida.copyOfRange(2, 5),
        )
    }

    @Test
    fun `el item pone el nombre solo y despues cantidad por precio`() {
        val salida = bytes(AnchoDePapel.Mm58) {
            item("Arroz Grado 1 kilo", "2", "$1.290", "$2.580")
        }
        val lineas = String(
            salida.copyOfRange(2, salida.size).map { (it.toInt() and 0xFF).toChar() }
                .joinToString("").toCharArray(),
        ).trimEnd('\n').split("\n")

        assertEquals(2, lineas.size)
        assertEquals("Arroz Grado 1 kilo", lineas[0])
        assertTrue("«${lineas[1]}» tiene que abrir con la cantidad", lineas[1].startsWith("2 x $1.290"))
        assertTrue("«${lineas[1]}» tiene que cerrar con el total", lineas[1].endsWith("$2.580"))
    }

    /** La consulta de papel es en tiempo real: tres bytes, sin encolar nada. */
    @Test
    fun `la consulta de papel es DLE EOT 4`() {
        assertArrayEquals(byteArrayOf(0x10, 0x04, 4), EscPos.CONSULTA_DE_PAPEL)
    }
}
