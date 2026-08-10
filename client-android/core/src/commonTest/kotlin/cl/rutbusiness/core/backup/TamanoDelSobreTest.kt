package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.random.Random
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Cuánto pesa el sobre cifrado de un puesto de feria **de verdad**.
 *
 * Existe porque el costo del bucket (ADR-0023) se calcula con este número, y
 * "unos pocos KB" no es un número: a 1.000.000 de usuarios la diferencia entre
 * 30 KB y 300 KB por sobre es la diferencia entre US$ 100 y US$ 1.000 al mes.
 * La cuenta del ADR se cita desde acá, así que si el formato engorda, el test
 * lo dice antes que la factura.
 *
 * El puesto que se modela es una verdulería de feria libre: sábado cargado,
 * ~120 boletas, 1 a 5 líneas cada una, casi todo efectivo y algo de fiado a la
 * vecina conocida. Nombres, precios y largos de id son los reales del POS
 * (`LineaDeVenta`, `PosRepository.nuevaClave` = 32 hex).
 *
 * Determinista (`Random(semilla)`): el número tiene que ser el mismo mañana o
 * no sirve para comparar.
 */
class TamanoDelSobreTest {

    /** Lo que se vende en un puesto de verduras — nombre y precio por unidad. */
    private val catalogoFeria: List<Pair<String, Int>> = listOf(
        "Tomate larga vida kilo" to 1490,
        "Palta Hass kilo" to 5990,
        "Lechuga costina unidad" to 990,
        "Papa nueva malla 5 kilos" to 4990,
        "Cebolla kilo" to 890,
        "Zanahoria kilo" to 790,
        "Limon de pica kilo" to 2490,
        "Platano ecuatoriano kilo" to 1290,
        "Manzana fuji kilo" to 1590,
        "Naranja de jugo malla" to 2990,
        "Choclo docena" to 3990,
        "Poroto verde kilo" to 2790,
        "Zapallo camote trozo" to 1890,
        "Perejil atado" to 500,
        "Cilantro atado" to 500,
        "Ajo cabeza" to 700,
        "Frutilla bandeja" to 2490,
        "Palta chilena kilo" to 4490,
        "Pepino ensalada unidad" to 690,
        "Betarraga kilo" to 990,
    )

    private val clientasFiado = listOf(
        "Sra. Rosa del 3", "Don Manuel taller", "Vecina Carmen",
        "Jorge feria martes", "Sra. Elena",
    )

    private fun claveDeCobro(r: Random): String =
        buildString(32) { repeat(32) { append("0123456789abcdef"[r.nextInt(16)]) } }

    private fun idProducto(r: Random): String =
        "product:" + buildString(20) {
            repeat(20) { append("abcdefghijklmnopqrstuvwxyz0123456789"[r.nextInt(36)]) }
        }

    /**
     * Un día de puesto. [ventas] boletas con líneas y medios de pago repartidos
     * como se reparten en la calle: la mayoría efectivo, unas pocas fiadas.
     */
    private fun diaDeFeria(ventas: Int, semilla: Int = 20260809): List<VentaEnCola> {
        val r = Random(semilla)
        // Los ids de producto se repiten entre boletas: el puesto vende siempre
        // lo mismo. Inventar un id nuevo por línea inflaría el snapshot con
        // datos que el puesto real no tiene.
        val ids = catalogoFeria.map { idProducto(r) }
        val base = 1_754_700_000L
        return (0 until ventas).map { i ->
            // 1 a 5 líneas, cargado al 2-3 (una señora lleva tomate, cebolla y
            // cilantro, no una línea sola ni ocho).
            val nLineas = when (r.nextInt(10)) {
                0 -> 1
                in 1..4 -> 2
                in 5..7 -> 3
                8 -> 4
                else -> 5
            }
            val items = (0 until nLineas).map {
                val idx = r.nextInt(catalogoFeria.size)
                val (nombre, precio) = catalogoFeria[idx]
                LineaDeVenta(
                    product = ids[idx],
                    productName = nombre,
                    quantity = 1 + r.nextInt(3),
                    unitPrice = precio.toString(),
                )
            }
            val total = items.sumOf { it.unitPrice.toInt() * it.quantity }
            // ~8% fiado, el resto efectivo (transferencia todavía es rara en la feria).
            val fiado = r.nextInt(100) < 8
            VentaEnCola(
                clave = claveDeCobro(r),
                solicitud = SolicitudDeVenta(
                    items = items,
                    paymentMethod = if (fiado) "pos_fiado" else "pos_cash",
                    cashAmount = if (fiado) null else redondearBillete(total).toString(),
                    customer = if (fiado) clientasFiado[r.nextInt(clientasFiado.size)] else null,
                ),
                cobradaEn = base + i * 137L,
                lineas = items.size,
                intentos = r.nextInt(3),
            )
        }
    }

    /** Con qué billete paga: nadie paga $4.370 justo. */
    private fun redondearBillete(total: Int): Int =
        listOf(1000, 2000, 5000, 10000, 20000).firstOrNull { it >= total } ?: (total + 999) / 1000 * 1000

    private fun sobreDe(cola: List<VentaEnCola>): SobreCifradoV1 {
        val prep = prepararRespaldoDesdeCola(
            tenantId = "puesto-rosa",
            cola = cola,
            createdAtUnix = 1_754_700_000L,
            rubro = "feria",
            claveAes32 = ByteArray(32) { it.toByte() },
        ).getOrThrow()
        return prep.sobre ?: error("el puesto tiene ventas: tiene que haber sobre")
    }

    @Test
    fun `un dia de feria sin senal cabe en decenas de kilobytes`() {
        val cola = diaDeFeria(ventas = 120)
        val sobre = sobreDe(cola)
        val plano = empaquetarSnapshot(
            armarSnapshotDesdeCola("puesto-rosa", 1_754_700_000L, cola, "feria"),
        ).getOrThrow()

        println(
            "MEDIDO sabado de feria (120 boletas, ${cola.sumOf { it.lineas }} lineas): " +
                "plaintext ${plano.size} B · sobre ${sobre.envelopeBytes.size} B " +
                "· ${sobre.envelopeBytes.size / cola.size} B/boleta",
        )

        // Medido 2026-08-09: 65.795 B. Bandas anchas a propósito: esto no fija
        // el formato, fija el orden de magnitud con que se hizo la cuenta del
        // ADR-0023. Si el sobre se sale de acá, la cuenta hay que rehacerla, no
        // ajustar el número de arriba.
        assertTrue(
            sobre.envelopeBytes.size in 45_000..90_000,
            "sobre de un dia de feria = ${sobre.envelopeBytes.size} B, fuera de la banda del ADR-0023",
        )
    }

    @Test
    fun `la cola llena es el techo del sobre de hoy`() {
        // 200 = ColaDeVentas.MAXIMO. Es el sobre más grande que la app puede
        // producir hoy, así que es el que hay que poder pagar.
        val cola = diaDeFeria(ventas = 200)
        val sobre = sobreDe(cola)
        println("MEDIDO cola al tope (200 boletas): sobre ${sobre.envelopeBytes.size} B")
        // Medido 2026-08-09: 108.073 B. Es el techo real de hoy y es el número
        // con el que se eligió el tope por sobre de las cuotas (ADR-0023).
        assertTrue(
            sobre.envelopeBytes.size in 80_000..150_000,
            "cola al tope = ${sobre.envelopeBytes.size} B",
        )
    }

    @Test
    fun `el sobre agrega poco sobre el plaintext`() {
        // AES-GCM no comprime ni infla: ciphertext = plaintext, + tag de 16 B,
        // + "RB1\n" + header JSON. Sin esto la cuenta del ADR tendría que
        // llevar un factor de expansión y no lo lleva.
        val cola = diaDeFeria(ventas = 120)
        val plano = empaquetarSnapshot(
            armarSnapshotDesdeCola("puesto-rosa", 1_754_700_000L, cola, "feria"),
        ).getOrThrow()
        val sobre = sobreDe(cola)
        val overhead = sobre.envelopeBytes.size - plano.size
        println("MEDIDO overhead del sobre RB1: $overhead B (header + tag GCM)")
        assertTrue(overhead in 200..400, "overhead = $overhead B")
        assertEquals(plano.size.toLong() + overhead, sobre.meta.sizeBytes)
    }

    @Test
    fun `el snapshot con el negocio entero es el que hay que dimensionar`() {
        // El snapshot v1 sólo lleva la cola offline. El día que lleve el
        // negocio (catálogo + fiado + historia) va a pesar otra cosa, y el
        // bucket se dimensiona con ESE número, no con el de hoy. Se mide con
        // datos de la misma forma para que la proyección no sea a ojo.
        val r = Random(20260809)
        val catalogo = (0 until 120).joinToString(",", "[", "]") { i ->
            val (nombre, precio) = catalogoFeria[i % catalogoFeria.size]
            """{"id":"${idProducto(r)}","name":"$nombre $i","price":"$precio","stock":${r.nextInt(200)}}"""
        }
        val fiado = (0 until 60).joinToString(",", "[", "]") { i ->
            """{"id":"customer:${i}","name":"${clientasFiado[i % clientasFiado.size]} $i",""" +
                """"saldo":"${r.nextInt(90000)}","desde":${1_750_000_000 + i * 8600}}"""
        }
        // Un año de puesto: 100 boletas/día × 26 días × 12 meses.
        val historia = diaDeFeria(ventas = 600, semilla = 7)
        val snap = armarSnapshotDesdeCola(
            tenantId = "puesto-rosa",
            createdAtUnix = 1_754_700_000L,
            pendingSales = historia,
            rubro = "feria",
        ).let {
            it.copy(sections = it.sections + mapOf("catalogo" to catalogo, "fiado" to fiado))
        }
        val plano = empaquetarSnapshot(snap).getOrThrow()
        val porBoleta = plano.size / historia.size

        println(
            "MEDIDO negocio completo (120 productos + 60 fiados + 600 boletas): " +
                "plaintext ${plano.size} B · ${porBoleta} B/boleta → " +
                "un año (31.200 boletas) ≈ ${31_200L * porBoleta / 1024 / 1024} MB",
        )

        // Medido 2026-08-09: 569 B/boleta → 16 MB de historia al año, sin
        // comprimir. El costo por boleta es lo que se proyecta; el total de
        // este test es sólo la muestra con la que se midió. Es alto porque el
        // snapshot repite el nombre del producto en cada línea y
        // `encodeDefaults = true` escribe los campos en cero de la cola
        // (`intentos`, `rechazada`, `motivo`) en todas las boletas. Ver
        // `CompresionDelSnapshotTest`: comprimir antes de cifrar lo baja ~8x.
        assertTrue(porBoleta in 400..800, "costo por boleta = $porBoleta B")
    }
}
