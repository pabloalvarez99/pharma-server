package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import java.io.ByteArrayOutputStream
import java.util.zip.Deflater
import java.util.zip.DeflaterOutputStream
import kotlin.random.Random
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * Cuánto se achica el snapshot si se comprime **antes** de cifrar.
 *
 * Es la palanca de costo más grande del respaldo y por eso se mide, no se
 * supone: el JSON del snapshot repite el nombre del producto en cada línea,
 * repite las claves del objeto en cada venta, y `encodeDefaults = true` escribe
 * `"intentos":0,"rechazada":false,"motivo":null` en las 120 boletas del día.
 * AES-GCM no comprime nada (ciphertext = plaintext + 16 B de tag), así que lo
 * que se sube hoy es ese JSON entero.
 *
 * Vive en `androidUnitTest` y no en `commonTest` porque `java.util.zip` no está
 * en el stdlib común. Es una medición del contenido, no del formato del sobre:
 * no depende de la plataforma.
 *
 * **Por qué comprimir antes de cifrar es seguro acá.** La objeción estándar
 * (CRIME/BREACH) necesita que el atacante inyecte plaintext elegido en el mismo
 * contexto de compresión que un secreto, y mida el largo repetidas veces. Un
 * respaldo es un archivo entero, escrito por la dueña, cifrado una vez: no hay
 * canal de inyección ni oráculo de largo. Lo que sí filtra el largo es el orden
 * de magnitud del negocio — y eso ya lo filtra el sobre sin comprimir.
 */
class CompresionDelSnapshotTest {

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
    )

    private fun diaDeFeria(ventas: Int): List<VentaEnCola> {
        val r = Random(20260809)
        val ids = catalogoFeria.map {
            "product:" + buildString(20) {
                repeat(20) { append("abcdefghijklmnopqrstuvwxyz0123456789"[r.nextInt(36)]) }
            }
        }
        return (0 until ventas).map { i ->
            val nLineas = 1 + r.nextInt(4)
            val items = (0 until nLineas).map {
                val idx = r.nextInt(catalogoFeria.size)
                val (nombre, precio) = catalogoFeria[idx]
                LineaDeVenta(ids[idx], nombre, 1 + r.nextInt(3), precio.toString())
            }
            VentaEnCola(
                clave = buildString(32) { repeat(32) { append("0123456789abcdef"[r.nextInt(16)]) } },
                solicitud = SolicitudDeVenta(items, "pos_cash", "5000", null),
                cobradaEn = 1_754_700_000L + i * 137L,
                lineas = items.size,
            )
        }
    }

    private fun desinflar(bytes: ByteArray): Int {
        val out = ByteArrayOutputStream()
        DeflaterOutputStream(out, Deflater(Deflater.BEST_COMPRESSION)).use { it.write(bytes) }
        return out.size()
    }

    @Test
    fun `comprimir antes de cifrar achica el sobre varias veces`() {
        val cola = diaDeFeria(120)
        val plano = empaquetarSnapshot(
            armarSnapshotDesdeCola("puesto-rosa", 1_754_700_000L, cola, "feria"),
        ).getOrThrow()
        val comprimido = desinflar(plano)
        val factor = plano.size.toDouble() / comprimido

        println(
            "MEDIDO compresion del snapshot (120 boletas): " +
                "${plano.size} B → $comprimido B · factor ${"%.1f".format(factor)}x",
        )

        // El JSON del snapshot es repetitivo por construcción. Si algún día
        // deja de comprimir, es que cambió el contenido (¿blobs? ¿base64
        // adentro?) y la cuenta de costo del ADR-0023 hay que rehacerla.
        assertTrue(factor > 4.0, "factor de compresion = $factor, esperado > 4x")
    }

    @Test
    fun `un ano de historia comprime todavia mejor`() {
        val cola = diaDeFeria(3_000)
        val plano = empaquetarSnapshot(
            armarSnapshotDesdeCola("puesto-rosa", 1_754_700_000L, cola, "feria"),
        ).getOrThrow()
        val comprimido = desinflar(plano)
        val factor = plano.size.toDouble() / comprimido
        println(
            "MEDIDO compresion con historia (3.000 boletas): " +
                "${plano.size} B → $comprimido B · factor ${"%.1f".format(factor)}x",
        )
        assertTrue(factor > 6.0, "factor con historia = $factor")
    }
}
