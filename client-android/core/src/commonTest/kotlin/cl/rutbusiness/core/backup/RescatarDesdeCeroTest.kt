package cl.rutbusiness.core.backup

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.respondError
import io.ktor.client.request.HttpRequestData
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import io.ktor.utils.io.ByteReadChannel
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * **Teléfono nuevo, app recién instalada, nada adentro.**
 *
 * Este archivo prueba el caso que justifica que el respaldo exista. Todo lo
 * demás —listar, bajar con sesión— sirve para un aparato que todavía funciona,
 * y un aparato que todavía funciona no necesita respaldo.
 *
 * El guion es literal: un sábado de feria se cobra sin señal, se sube el sobre
 * cifrado, el teléfono se pierde. Al día siguiente hay otro teléfono, sin
 * sesión y sin token, y una tarjeta escrita a mano. Al final el negocio tiene
 * que volver.
 *
 * Las tres cosas que se afirman, y ninguna es decorativa:
 *
 * 1. El rescate viaja **sin** `Authorization`. Si necesitara token no serviría.
 * 2. Lo que el teléfono manda al subir es un hash; lo que manda al rescatar es
 *    la prueba. El "server" de mentira de acá hace la misma comparación que el
 *    de verdad —`SHA-256(prueba) == hash guardado`— y por eso el test falla si
 *    las dos derivaciones se desincronizan.
 * 3. Del cable no sale plaintext: se revisa el cuerpo de la subida contra un
 *    marcador que está dentro del snapshot.
 */
class RescatarDesdeCeroTest {

    private val json = Json { ignoreUnknownKeys = true }

    private val slug = "puesto-rosa"
    private val tenantId = "tenant:puesto-rosa"

    /** Va adentro del snapshot. No puede aparecer en ningún byte del cable. */
    private val marcador = "MARCADOR-CLIENTE-QUE-NO-DEBE-SALIR-4b1e"

    private fun cola() = listOf(
        VentaEnCola(
            clave = "idem-1",
            solicitud = SolicitudDeVenta(
                items = listOf(
                    LineaDeVenta(
                        product = "product:tomate",
                        productName = marcador,
                        quantity = 3,
                        unitPrice = "990",
                    ),
                ),
                paymentMethod = "pos_cash",
            ),
            cobradaEn = 1_700_000_000L,
            lineas = 1,
        ),
    )

    /**
     * El bucket y el índice, del tamaño que hace falta: un sobre y el hash de
     * retiro con el que se lo puede pedir sin sesión.
     */
    private class ServerDeMentira {
        var ciphertextBase64: String? = null
        var hashGuardado: String? = null
        var pidioRescateSinToken: Boolean? = null
        var cuerpoDeSubida: String? = null
    }

    private fun motor(estado: ServerDeMentira) = MockEngine { req ->
        when {
            req.url.encodedPath == USER_BACKUP_UPLOAD_PATH -> {
                val cuerpo = leerCuerpo(req)
                estado.cuerpoDeSubida = cuerpo
                val obj = json.parseToJsonElement(cuerpo).jsonObject
                estado.ciphertextBase64 = obj["ciphertext_base64"]!!.jsonPrimitive.content
                estado.hashGuardado = obj["retrieval_hash_hex"]?.jsonPrimitive?.content
                respond(
                    """{"accepted":true,"backup_id":"20260809T120000Z-abc123abc123"}""",
                    HttpStatusCode.OK,
                    headersOf(HttpHeaders.ContentType, "application/json"),
                )
            }

            req.url.encodedPath == USER_BACKUP_RESCUE_PATH -> {
                estado.pidioRescateSinToken = req.headers[HttpHeaders.Authorization] == null
                val obj = json.parseToJsonElement(leerCuerpo(req)).jsonObject
                val prueba = obj["retrieval_proof_hex"]!!.jsonPrimitive.content
                val slugPedido = obj["tenant_slug"]!!.jsonPrimitive.content

                // Exactamente lo que hace la ruta real: hashear la prueba y
                // compararla con lo guardado. Nunca descifra nada.
                val hashDeLaPrueba = PruebaDeRetiro.hashHex(hexToBytes(prueba)!!)
                val calza = slugPedido == "puesto-rosa" &&
                    estado.hashGuardado != null &&
                    hashDeLaPrueba == estado.hashGuardado

                if (!calza) {
                    // 404 uniforme para todo lo que falla, igual que el server.
                    respondError(HttpStatusCode.NotFound)
                } else {
                    respond(
                        """
                        {"meta":{"tenant_id":"tenant:puesto-rosa","format_version":1,
                          "ciphertext_sha256_hex":"${"0".repeat(64)}","size_bytes":1,
                          "uploaded_at_unix":1700000000},
                         "ciphertext_base64":"${estado.ciphertextBase64}",
                         "backup_id":"20260809T120000Z-abc123abc123"}
                        """.trimIndent(),
                        HttpStatusCode.OK,
                        headersOf(HttpHeaders.ContentType, "application/json"),
                    )
                }
            }

            else -> respondError(HttpStatusCode.NotFound)
        }
    }

    private suspend fun leerCuerpo(req: HttpRequestData): String {
        val contenido = req.body
        return when (contenido) {
            is io.ktor.http.content.TextContent -> contenido.text
            is io.ktor.http.content.ByteArrayContent -> contenido.bytes().decodeToString()
            else -> ByteReadChannel(ByteArray(0)).toString()
        }
    }

    @Test
    fun el_sabado_se_sube_el_domingo_se_pierde_el_telefono_y_el_negocio_vuelve() = runTest {
        val estado = ServerDeMentira()
        val motor = motor(estado)

        // La tarjeta que la dueña escribió en el cuaderno el día del alta.
        val clave = claveDeDemostracion()
        val material = MaterialRecuperacion.Frase(clave.palabras)

        // --- sábado: teléfono viejo, con sesión ---------------------------------
        val prep = prepararRespaldoDesdeCola(
            tenantId = tenantId,
            cola = cola(),
            createdAtUnix = 1_700_000_000L,
            materialRecuperacion = material,
            tenantSlug = slug,
        ).getOrThrow()

        val sobre = assertNotNull(prep.sobre, "tenía que cifrar")
        val hash = assertNotNull(
            prep.retrievalHashHex,
            "sin hash de retiro el respaldo sólo se baja desde el teléfono que se pierde",
        )

        val viejo = ApiFactory("http://localhost:8080", motor = motor) { "token-del-telefono-viejo" }
        val resp = UserBackupApi(viejo).subirSobre(sobre, hash)
        assertTrue(resp is cl.rutbusiness.core.net.Resultado.Ok, "la subida tenía que aceptarse")
        assertTrue(resp.valor.accepted)

        // Lo que viajó por el cable no lleva el negocio en claro.
        val cuerpo = assertNotNull(estado.cuerpoDeSubida)
        assertTrue(marcador !in cuerpo, "salió plaintext en el cuerpo de la subida")
        assertTrue(clave.fraseCompleta() !in cuerpo, "salió la frase del cuaderno")
        assertTrue(hash in cuerpo, "el hash de retiro no llegó al server")

        // --- domingo: teléfono nuevo, sin nada ---------------------------------
        // Sin token, y a propósito: es lo que tiene quien perdió el aparato.
        val nuevo = ApiFactory("http://localhost:8080", motor = motor) { null }

        val restaurado = rescatarRespaldoDesdeCero(
            backup = UserBackupApi(nuevo),
            tenantSlug = "  Puesto-Rosa ", // tal como lo tipearía alguien apurado
            materialRaw = clave.fraseCompleta(),
        ).getOrThrow()

        assertEquals(true, estado.pidioRescateSinToken, "el rescate no puede depender de un JWT")
        assertEquals(1, restaurado.ventasEnCola)
        assertEquals(
            marcador,
            restaurado.snapshot.pendingSales.single().solicitud.items.single().productName,
        )
        assertEquals(tenantId, restaurado.snapshot.tenantId)
    }

    /** Con los bloques del QR en vez de las palabras: mismo resultado. */
    @Test
    fun los_bloques_del_qr_tambien_abren_desde_cero() = runTest {
        val estado = ServerDeMentira()
        val motor = motor(estado)
        val clave = claveDeDemostracion()

        val prep = prepararRespaldoDesdeCola(
            tenantId = tenantId,
            cola = cola(),
            createdAtUnix = 1_700_000_000L,
            materialRecuperacion = MaterialRecuperacion.Frase(clave.palabras),
            tenantSlug = slug,
        ).getOrThrow()
        UserBackupApi(ApiFactory("http://localhost:8080", motor = motor) { "t" })
            .subirSobre(prep.sobre!!, prep.retrievalHashHex)

        val r = rescatarRespaldoDesdeCero(
            backup = UserBackupApi(ApiFactory("http://localhost:8080", motor = motor) { null }),
            tenantSlug = slug,
            materialRaw = payloadQrRescate(slug, clave.bloques)!!,
        ).getOrThrow()

        assertEquals(1, r.ventasEnCola)
    }

    /**
     * Una palabra mal copiada se detecta **en el teléfono**, con el número de
     * palabra, y no sale a la red. El 404 del server no distingue "escribiste
     * mal" de "ese negocio no existe", así que si esto llegara al cable la
     * persona recibiría el mensaje equivocado.
     */
    @Test
    fun una_palabra_mal_escrita_ni_siquiera_sale_a_la_red() = runTest {
        val estado = ServerDeMentira()
        val motor = motor(estado)
        val palabras = claveDeDemostracion().palabras.toMutableList()
        palabras[6] = "helicoptero" // no está en el vocabulario

        val r = rescatarRespaldoDesdeCero(
            backup = UserBackupApi(ApiFactory("http://localhost:8080", motor = motor) { null }),
            tenantSlug = slug,
            materialRaw = palabras.joinToString(" "),
        )

        assertTrue(r.isFailure)
        assertTrue(
            r.exceptionOrNull()!!.message!!.contains("7"),
            "tiene que decir cuál palabra: ${r.exceptionOrNull()?.message}",
        )
        assertNull(estado.pidioRescateSinToken, "no tenía que llamar al server")
    }

    /** Tarjeta de otro negocio: 404 uniforme, y un mensaje que se puede accionar. */
    @Test
    fun la_tarjeta_de_otro_negocio_no_abre_este() = runTest {
        val estado = ServerDeMentira()
        val motor = motor(estado)
        val clave = claveDeDemostracion()

        val prep = prepararRespaldoDesdeCola(
            tenantId = tenantId,
            cola = cola(),
            createdAtUnix = 1_700_000_000L,
            materialRecuperacion = MaterialRecuperacion.Frase(clave.palabras),
            tenantSlug = slug,
        ).getOrThrow()
        UserBackupApi(ApiFactory("http://localhost:8080", motor = motor) { "t" })
            .subirSobre(prep.sobre!!, prep.retrievalHashHex)

        // Semilla distinta, misma forma. La prueba no calza y el sobre no baja.
        val otra = generarClaveDelNegocio(byteArrayOf(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11))
        val r = rescatarRespaldoDesdeCero(
            backup = UserBackupApi(ApiFactory("http://localhost:8080", motor = motor) { null }),
            tenantSlug = slug,
            materialRaw = otra.fraseCompleta(),
        )

        assertTrue(r.isFailure)
        val msg = r.exceptionOrNull()!!.message!!
        assertTrue(slug in msg, "el mensaje tiene que mostrar el slug que se usó: $msg")
    }

    /**
     * Sin slug no se calcula el hash, y sin hash el sobre sube igual pero
     * después no se puede rescatar. Es un modo degradado real (apps viejas), así
     * que se fija que sea **explícito** y no un silencio.
     */
    @Test
    fun sin_slug_el_sobre_sube_pero_queda_sin_carril_de_rescate() = runTest {
        val prep = prepararRespaldoDesdeCola(
            tenantId = tenantId,
            cola = cola(),
            createdAtUnix = 1_700_000_000L,
            materialRecuperacion = MaterialRecuperacion.Frase(claveDeDemostracion().palabras),
        ).getOrThrow()

        assertNotNull(prep.sobre)
        assertNull(prep.retrievalHashHex)
    }
}
