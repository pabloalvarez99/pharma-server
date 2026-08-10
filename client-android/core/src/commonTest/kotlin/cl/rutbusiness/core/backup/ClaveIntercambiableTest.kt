package cl.rutbusiness.core.backup

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import kotlin.test.fail

/**
 * La invariante que sostiene el respaldo del feriante: **las 12 palabras y los
 * 5 bloques abren el mismo sobre**.
 *
 * Existió el bug contrario y costaba el negocio entero: cada forma se derivaba
 * por su cuenta y el KDF corría sobre el texto mostrado, así que cifrar con la
 * frase y restaurar con el QR (que lleva bloques) daba llaves distintas y un
 * respaldo que no abría, sin ningún aviso. Estos tests están para que no
 * vuelva.
 */
class ClaveIntercambiableTest {

    private val salt = ByteArray(KDF_SALT_LEN) { (it * 7 + 3).toByte() }

    // Barato a propósito: acá se prueba que las llaves COINCIDAN, no el costo
    // del KDF. Con 210k iteraciones reales la suite se vuelve inusable.
    private val iters = 100_000

    @Test
    fun `frase y bloques de la misma clave dan la misma llave`() {
        repeat(64) { n ->
            val clave = generarClaveDelNegocio(ByteArray(BYTES_SEMILLA) { (n * 31 + it * 17).toByte() })

            val porFrase = derivarClaveDeMaterial(
                MaterialRecuperacion.Frase(clave.palabras), salt, iters,
            )
            val porBloques = derivarClaveDeMaterial(
                MaterialRecuperacion.Bloques(clave.bloques), salt, iters,
            )

            assertContentEquals(
                porFrase,
                porBloques,
                "clave $n: la frase y los bloques tienen que abrir el mismo sobre",
            )
        }
    }

    @Test
    fun `el sobre cifrado con la frase se abre con los bloques`() {
        val clave = claveNuevaDelNegocio()
        val datos = "venta: 3 kilos de tomate a 2000".encodeToByteArray()

        val llaveFrase = derivarClaveDeMaterial(MaterialRecuperacion.Frase(clave.palabras), salt, iters)
        val llaveBloques = derivarClaveDeMaterial(MaterialRecuperacion.Bloques(clave.bloques), salt, iters)

        val sobre = cifrarSobreV1(
            key = llaveFrase,
            plaintext = datos,
            tenantId = "feria-demo",
            uploadedAtUnix = 1_754_600_000L,
            salt = salt,
        ).getOrThrow()
        val abierto = descifrarSobreV1(llaveBloques, sobre.envelopeBytes).getOrThrow()

        assertContentEquals(datos, abierto, "el QR tiene que abrir lo que se cifró con las palabras")
    }

    @Test
    fun `la frase vuelve a la semilla y la semilla vuelve a la frase`() {
        repeat(64) { n ->
            val semilla = ByteArray(BYTES_SEMILLA) { (n * 13 + it * 29).toByte() }
            val clave = generarClaveDelNegocio(semilla)

            val desdeFrase = semillaDesdeFrase(clave.palabras).getOrThrow()
            val desdeBloques = semillaDesdeBloques(clave.bloques).getOrThrow()

            assertContentEquals(desdeFrase, desdeBloques, "las dos formas son la misma semilla")
            assertEquals(clave.palabras, fraseDesdeSemilla(desdeFrase), "round-trip de la frase")
            assertEquals(clave.bloques, bloquesDesdeSemilla(desdeBloques), "round-trip de los bloques")
        }
    }

    @Test
    fun `el vocabulario son 128 palabras distintas`() {
        // 128 = 2^7: cada palabra son 7 bits exactos, sin sesgo de módulo.
        assertEquals(128, VOCABULARIO_RESCATE.size)
        // Tuvo `café` y `pasaje` repetidos: con una palabra en dos índices,
        // decodificar la frase del cuaderno era adivinar.
        assertEquals(128, VOCABULARIO_RESCATE.toSet().size, "no puede haber palabras repetidas")
        assertTrue(
            VOCABULARIO_RESCATE.all { it == normalizarPalabra(it) },
            "el vocabulario se guarda ya normalizado (sin tildes, minúscula)",
        )
    }

    @Test
    fun `acepta la frase sin tildes y en mayusculas`() {
        val clave = claveDeDemostracion()
        val comoLaEscribe = clave.palabras.map { it.uppercase() }

        assertContentEquals(
            semillaDesdeFrase(clave.palabras).getOrThrow(),
            semillaDesdeFrase(comoLaEscribe).getOrThrow(),
            "una tilde de menos en el cuaderno no puede costar el respaldo",
        )
        // "limon" escrito "limón" tiene que entrar igual.
        assertEquals(INDICE_VOCABULARIO["limon"], INDICE_VOCABULARIO[normalizarPalabra("Limón")])
    }

    @Test
    fun `un bloque mal copiado se detecta al tipearlo`() {
        val clave = claveDeDemostracion()
        val roto = clave.bloques.toMutableList()
        // Cambia un solo carácter, como quien lee mal su propia letra.
        roto[2] = roto[2].let { b -> (if (b[0] == 'A') 'B' else 'A') + b.substring(1) }

        val r = semillaDesdeBloques(roto)
        assertTrue(r.isFailure, "el checksum tiene que atajar un bloque mal copiado")
        assertNotNull(r.exceptionOrNull()?.message)
    }

    @Test
    fun `una palabra que no existe dice cual es`() {
        val clave = claveDeDemostracion()
        val roto = clave.palabras.toMutableList().also { it[6] = "zzzz" }

        val e = semillaDesdeFrase(roto).exceptionOrNull() ?: fail("tenía que fallar")
        assertTrue(
            e.message!!.contains("7"),
            "el mensaje tiene que decir qué palabra revisar, decía: ${e.message}",
        )
    }

    @Test
    fun `el parser reconoce las dos formas y el payload del QR`() {
        val clave = claveDeDemostracion()

        val frase = parsearMaterialRecuperacion(clave.fraseCompleta()).getOrThrow()
        assertTrue(frase is MaterialRecuperacion.Frase)

        val bloques = parsearMaterialRecuperacion(clave.bloquesCompletos()).getOrThrow()
        assertTrue(bloques is MaterialRecuperacion.Bloques)

        assertContentEquals(
            semillaDeMaterial(frase).getOrThrow(),
            semillaDeMaterial(bloques).getOrThrow(),
            "lo tipeado de una u otra forma converge a la misma semilla",
        )
    }
}
