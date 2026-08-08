package cl.rutbusiness.app.ui.impresora

/**
 * Los bytes que entiende una impresora térmica de boleta.
 *
 * Port directo de `client/src-tauri/src/escpos.rs`, el que ya imprime en el
 * escritorio. Se porta y no se reinventa por una razón concreta: la boleta que
 * sale del teléfono y la que sale del PC del negocio tienen que ser **la misma
 * boleta**. Un negocio con las dos cosas no puede tener dos formatos según
 * quién cobró.
 *
 * Alcance, igual que en el escritorio: modo texto, que es lo único que todos
 * los clones de 58 y 80 mm soportan de verdad. Sin logos raster ni QR — una
 * boleta SII impresa por rollo es trabajo del módulo DTE.
 *
 * Sin `android.` ni nada de plataforma: son bytes. Por eso se prueba entero en
 * la JVM, sin impresora y sin teléfono, que es lo único de este encargo que se
 * puede verificar sin hardware.
 */
internal object EscPos {
    const val ESC: Byte = 0x1B
    const val GS: Byte = 0x1D
    const val DLE: Byte = 0x10
    const val EOT: Byte = 0x04

    /**
     * Pregunta en tiempo real por el sensor de papel: `DLE EOT 4`.
     *
     * La impresora contesta **un byte** sin pasar por el buffer de impresión,
     * así que se puede preguntar antes de mandar la boleta. Los bits 5 y 6
     * ([SIN_PAPEL]) prendidos significan que se acabó el rollo.
     *
     * Muchos clones baratos no contestan nada. Eso **no** es un error: se
     * imprime igual. Ver [EstadoDePapel].
     */
    val CONSULTA_DE_PAPEL = byteArrayOf(DLE, EOT, 4)

    /** Bits 5|6 de la respuesta a [CONSULTA_DE_PAPEL]: no queda papel. */
    const val SIN_PAPEL = 0x60

    /**
     * Pulso de apertura del cajón de dinero: `ESC p m t1 t2`.
     *
     * - `m = 0` es el pin 2 del conector, el default de las Epson TM y de los
     *   clones que se venden en Chile.
     * - `t1 = 25` (~50 ms encendido), `t2 = 250` (~500 ms apagado), en unidades
     *   de 2 ms.
     *
     * El cajón no tiene cable propio: cuelga de la impresora por un RJ-11, así
     * que abrirlo es mandarle estos bytes a la misma impresora.
     */
    val PULSO_DE_CAJON = byteArrayOf(ESC, 'p'.code.toByte(), 0, 25, 250.toByte())
}

/** Lo que contestó la impresora cuando se le preguntó por el papel. */
internal enum class EstadoDePapel {
    /** Contestó y hay papel. */
    Hay,

    /** Contestó y se acabó. */
    NoHay,

    /**
     * No contestó. La mayoría de los clones baratos no implementan la consulta
     * en tiempo real, así que esto es lo **normal**, no una falla: se imprime
     * igual y si no hay papel el cajero lo va a ver.
     */
    NoContesta,
}

/** Ancho del rollo, que es lo único que cambia la forma de la boleta. */
enum class AnchoDePapel(val columnas: Int, val etiqueta: String, val descripcion: String) {
    /** El rollo angosto, el más común en el negocio chico. */
    Mm58(32, "58 mm", "El rollo angosto, del ancho de un teléfono"),

    /** El rollo ancho, el de las cajas de supermercado. */
    Mm80(48, "80 mm", "El rollo ancho, el de las cajas de supermercado"),
}

/**
 * Arma una boleta byte a byte.
 *
 * El texto se guarda como `String` y se codifica al final, porque la página de
 * códigos de la impresora no es UTF-8 y hay que traducir carácter por carácter.
 */
internal class ConstructorDeBoleta(private val ancho: AnchoDePapel) {

    // ESC @ — inicializa: limpia el buffer y resetea negrita, tamaño y
    // alineación. Sin esto, una boleta hereda el estado que dejó la anterior.
    private val buf = ArrayList<Byte>(512).apply {
        add(EscPos.ESC)
        add('@'.code.toByte())
    }

    /**
     * Escribe texto traducido a la página de códigos de la impresora.
     *
     * Los clones que se venden en Chile hablan CP437/CP858, donde las vocales
     * con tilde y la ñ existen pero **no** en las posiciones de UTF-8. Un
     * carácter que no está en la tabla degrada a `?` en vez de reventar la
     * boleta entera: una boleta con un signo raro se entrega igual; una boleta
     * que no salió, no.
     */
    private fun escribir(texto: String) {
        for (ch in texto) {
            buf.add(
                when (ch) {
                    'á' -> 0xA0.toByte()
                    'é' -> 0x82.toByte()
                    'í' -> 0xA1.toByte()
                    'ó' -> 0xA2.toByte()
                    'ú' -> 0xA3.toByte()
                    'ñ' -> 0xA4.toByte()
                    'Ñ' -> 0xA5.toByte()
                    'ü' -> 0x81.toByte()
                    'Á' -> 0xB5.toByte()
                    '°' -> 0xF8.toByte()
                    else -> if (ch.code in 0..127) ch.code.toByte() else '?'.code.toByte()
                },
            )
        }
    }

    /** Una línea, cortada al ancho del rollo. */
    private fun linea(texto: String) {
        escribir(texto.take(ancho.columnas))
        buf.add('\n'.code.toByte())
    }

    fun izquierda(texto: String) = apply { linea(texto) }

    /** Centrada: `ESC a 1`, y vuelve a la izquierda. */
    fun centrada(texto: String) = apply {
        buf.addAll(listOf(EscPos.ESC, 'a'.code.toByte(), 1))
        linea(texto)
        buf.addAll(listOf(EscPos.ESC, 'a'.code.toByte(), 0))
    }

    /** Negrita: `ESC E 1`. */
    fun negrita(texto: String) = apply {
        buf.addAll(listOf(EscPos.ESC, 'E'.code.toByte(), 1))
        linea(texto)
        buf.addAll(listOf(EscPos.ESC, 'E'.code.toByte(), 0))
    }

    /**
     * Doble alto y doble ancho, centrado: `GS ! 0x11`. Es el total.
     *
     * Al doble de ancho entran la mitad de las columnas, así que el corte es a
     * la mitad. Es el número que el cliente mira desde el otro lado del
     * mostrador.
     */
    fun grande(texto: String) = apply {
        buf.addAll(listOf(EscPos.ESC, 'a'.code.toByte(), 1, EscPos.GS, '!'.code.toByte(), 0x11))
        escribir(texto.take(ancho.columnas / 2))
        buf.addAll(
            listOf(
                '\n'.code.toByte(),
                EscPos.GS,
                '!'.code.toByte(),
                0x00,
                EscPos.ESC,
                'a'.code.toByte(),
                0,
            ),
        )
    }

    /** `etiqueta ....... valor`, ocupando el ancho completo. */
    fun fila(etiqueta: String, valor: String) = apply {
        val usado = etiqueta.length + valor.length
        val puntos = (ancho.columnas - usado).coerceAtLeast(1)
        linea(etiqueta + ".".repeat(puntos) + valor)
    }

    /** Raya separadora de guiones. */
    fun separador() = apply { linea("-".repeat(ancho.columnas)) }

    /**
     * Un ítem: el nombre en su propia línea, y abajo `cantidad x precio .. total`.
     *
     * El nombre va solo porque un producto chileno rara vez entra en media
     * línea de 32 columnas junto a dos montos.
     */
    fun item(nombre: String, cantidad: String, unitario: String, total: String) = apply {
        linea(nombre)
        fila("$cantidad x $unitario", total)
    }

    fun avanzar(lineas: Int) = apply {
        repeat(lineas) { buf.add('\n'.code.toByte()) }
    }

    /**
     * Corte parcial: `GS V 1`, precedido de tres líneas en blanco.
     *
     * Las tres líneas no son decoración: el cabezal está unos milímetros antes
     * de la cuchilla, y sin ellas el corte se come el pie de la boleta.
     */
    fun cortar() = apply {
        avanzar(3)
        buf.addAll(listOf(EscPos.GS, 'V'.code.toByte(), 1))
    }

    /** Abre el cajón al final de la boleta. */
    fun abrirCajon() = apply { buf.addAll(EscPos.PULSO_DE_CAJON.toList()) }

    fun construir(): ByteArray = buf.toByteArray()
}
