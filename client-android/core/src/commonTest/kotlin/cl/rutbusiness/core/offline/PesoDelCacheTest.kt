package cl.rutbusiness.core.offline

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import kotlin.test.Test
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.builtins.ListSerializer

/**
 * Cuánto pesa lo que este módulo guarda.
 *
 * El aparato objetivo tiene 1-2 GB de RAM **para todo el teléfono**, así que
 * "cachear" no puede convertirse en "tener el negocio entero residente". Estos
 * dos techos son la parte de esa regla que se puede medir, y están escritos como
 * prueba y no como comentario para que crezcan sólo a propósito: el día que
 * alguien suba el tope de productos o le agregue campos al DTO, esto se cae y
 * hay que mirar el número.
 *
 * Se mide el **texto que va a disco**, que es lo que se puede afirmar sin
 * inventar: el objeto en memoria depende del runtime, pero es del mismo orden
 * -las cadenas son casi todo el contenido- y además sólo vive mientras la
 * pantalla que lo pidió está abierta.
 */
class PesoDelCacheTest {

    /**
     * Un producto con los campos llenos como los manda el server.
     *
     * Nombres largos a propósito: un catálogo real tiene "Aceite de maravilla
     * Chef 900 ml botella", no "prod1". Medir con datos cortos daría un número
     * bonito y falso.
     */
    private fun producto(indice: Int) = ProductDto(
        active = true,
        createdAt = "2026-08-06T22:02:33.936803200Z",
        id = "product:y3cobx0ugdnzrb5ha0i$indice",
        name = "Aceite de maravilla Chef 900 ml botella $indice",
        physicalStock = true,
        prescriptionType = "direct",
        price = "2790",
        slug = "aceite-de-maravilla-chef-900-ml-botella-$indice",
        stock = 16,
        updatedAt = "2026-08-07T00:00:50.541253200Z",
        barcode = "780123456789$indice",
        costPrice = "1953",
    )

    @Test
    fun `el catalogo guardado entra en un cuarto de mega`() = runTest {
        val disco = AlmacenDeMentira()
        val cache = CacheDeLecturas(disco) { 0L }
        val catalogo = (1..TOPE_DE_PRODUCTOS).map { producto(it) }

        cache.guardar(
            ClaveDeCache.de(ClaveDeCache.Que.Catalogo, "http://192.168.1.10:8080"),
            ListSerializer(ProductDto.serializer()),
            catalogo,
        )

        val bytes = disco.leer(
            ClaveDeCache.de(ClaveDeCache.Que.Catalogo, "http://192.168.1.10:8080").archivo,
        )!!.length

        assertTrue(
            bytes <= TECHO_CATALOGO,
            "el catálogo de $TOPE_DE_PRODUCTOS productos pesa ${bytes / 1024} KB y el techo " +
                "son ${TECHO_CATALOGO / 1024} KB. Si subió el tope de productos o el DTO " +
                "engordó, mirá el número antes de subir el techo: esto vive en un teléfono " +
                "de 1 GB.",
        )
    }

    @Test
    fun `la cola llena entra en medio mega`() = runTest {
        val disco = AlmacenDeMentira()
        val cola = ColaDeVentas(disco) { 0L }
        cola.cargar()

        // El peor caso permitido: la cola en su tope, con ventas de mostrador
        // grandes (diez líneas cada una).
        repeat(ColaDeVentas.MAXIMO) { n ->
            cola.encolar(
                VentaEnCola(
                    clave = "clave-de-idempotencia-de-32-hex-$n",
                    solicitud = SolicitudDeVenta(
                        items = (1..10).map {
                            LineaDeVenta(
                                product = "product:y3cobx0ugdnzrb5ha0i$it",
                                productName = "Aceite de maravilla Chef 900 ml botella $it",
                                quantity = 2,
                                unitPrice = "2790",
                            )
                        },
                        paymentMethod = "pos_cash",
                    ),
                    cobradaEn = 1_700_000_000_000L,
                    lineas = 10,
                ),
            )
        }

        val bytes = disco.leer("ventas-pendientes.json")!!.length

        assertTrue(
            bytes <= TECHO_COLA,
            "la cola llena pesa ${bytes / 1024} KB y el techo son ${TECHO_COLA / 1024} KB",
        )
    }

    private companion object {
        /** El mismo `LIMITE_CACHE` de `CobrarViewModel`. */
        const val TOPE_DE_PRODUCTOS = 200

        /** 256 KB. Un cuarto de mega para el mostrador entero es barato. */
        const val TECHO_CATALOGO = 256 * 1024

        /** 512 KB. Son 200 ventas de diez líneas: un día entero sin sistema. */
        const val TECHO_COLA = 512 * 1024
    }
}
