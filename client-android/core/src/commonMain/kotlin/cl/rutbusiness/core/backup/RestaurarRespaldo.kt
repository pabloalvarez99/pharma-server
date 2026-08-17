package cl.rutbusiness.core.backup

/**
 * Restore local del snapshot (ADR-0022).
 *
 * La dueña tipea la llave del cuaderno; el cliente re-deriva la llave AES con
 * el salt del header del sobre y descifra. **Nunca** manda la frase al server.
 *
 * Fallo de frase = error claro. Sin "¿olvidaste tu clave?".
 */

data class RestauracionRespaldo(
    val snapshot: SnapshotBackupV1,
    val ventasEnCola: Int,
    val mensaje: String,
    /**
     * Cuántas ventas se fusionaron a la cola local (si el caller lo hizo).
     * -1 = no rehidrató (solo abrió el snapshot).
     */
    val rehidratadas: Int = -1,
)

/**
 * Descifra un sobre v1 con el material del cuaderno.
 *
 * [envelopeBytes] es el paquete `RB1\n`+header+ct (no el base64).
 */
fun restaurarDesdeSobre(
    material: MaterialRecuperacion,
    envelopeBytes: ByteArray,
): Result<RestauracionRespaldo> {
    if (envelopeBytes.isEmpty()) {
        return Result.failure(IllegalArgumentException("sobre vacío"))
    }
    val (header, _) = desempaquetarEnvelope(envelopeBytes)
        .getOrElse { return Result.failure(it) }

    if (header.formatVersion != FORMAT_VERSION) {
        return Result.failure(
            IllegalArgumentException(
                "format_version=${header.formatVersion} (esta app entiende $FORMAT_VERSION)",
            ),
        )
    }
    if (header.aead != AEAD_ALG) {
        return Result.failure(IllegalArgumentException("AEAD no soportado: ${header.aead}"))
    }

    val salt = hexToBytes(header.saltHex)
        ?: return Result.failure(IllegalArgumentException("salt del sobre inválido"))
    if (salt.size != KDF_SALT_LEN) {
        return Result.failure(IllegalArgumentException("salt del sobre: ${salt.size} bytes"))
    }

    // Iteraciones del header (no confiar en un default viejo del código).
    val iters = header.kdfIterations.coerceAtLeast(100_000)
    val key = try {
        when (header.kdf) {
            KDF_ALG, "pbkdf2-hmac-sha256" ->
                derivarClaveDeMaterial(material, salt, iterations = iters)
            else ->
                return Result.failure(
                    IllegalArgumentException(
                        "KDF del sobre (${header.kdf}) no está en esta app. " +
                            "Actualiza o usa el aparato que creó el respaldo.",
                    ),
                )
        }
    } catch (e: Exception) {
        return Result.failure(e)
    }

    val plain = descifrarSobreV1(key, envelopeBytes).getOrElse {
        return Result.failure(
            IllegalArgumentException(
                "No se pudo abrir el respaldo. Revisa las palabras del cuaderno.",
            ),
        )
    }
    val snap = desempaquetarSnapshot(plain).getOrElse { return Result.failure(it) }
    val n = snap.pendingSales.size
    return Result.success(
        RestauracionRespaldo(
            snapshot = snap,
            ventasEnCola = n,
            mensaje = mensajeApertura(n, rehidratadas = -1),
            rehidratadas = -1,
        ),
    )
}

/**
 * Mensaje honesto post-apertura (con o sin merge a la cola local).
 */
fun mensajeApertura(ventasEnSobre: Int, rehidratadas: Int): String = when {
    ventasEnSobre == 0 ->
        "Respaldo abierto. No traía ventas pendientes."
    rehidratadas < 0 ->
        if (ventasEnSobre == 1) {
            "Respaldo abierto: 1 venta pendiente."
        } else {
            "Respaldo abierto: $ventasEnSobre ventas pendientes."
        }
    rehidratadas == 0 && ventasEnSobre > 0 ->
        "Respaldo abierto: $ventasEnSobre venta(s) ya estaban en este teléfono " +
            "(o la cola está llena). No se duplicaron."
    rehidratadas == 1 ->
        "Listo: 1 venta del respaldo volvió a la cola. " +
            "Sale sola cuando haya señal."
    else ->
        "Listo: $rehidratadas ventas del respaldo volvieron a la cola. " +
            "Salen solas cuando haya señal."
}

/** Ajusta el mensaje tras fusionar a [cl.rutbusiness.core.offline.ColaDeVentas]. */
fun RestauracionRespaldo.conRehidratacion(agregadas: Int): RestauracionRespaldo =
    copy(
        rehidratadas = agregadas,
        mensaje = mensajeApertura(ventasEnCola, agregadas),
    )

/**
 * Atajo: material en texto + sobre en base64 (lo que la dueña pega o
 * el último preparado en pantalla).
 */
fun restaurarDesdeTextos(
    materialRaw: String,
    envelopeBase64: String,
): Result<RestauracionRespaldo> {
    val material = parsearMaterialRecuperacion(materialRaw).getOrElse {
        return Result.failure(IllegalArgumentException(mensajeErrorMaterial(materialRaw)))
    }
    val b64 = envelopeBase64.trim()
    if (b64.isEmpty()) {
        return Result.failure(IllegalArgumentException("Falta el paquete cifrado (base64)."))
    }
    val envelope = try {
        base64ToEnvelope(b64)
    } catch (e: Exception) {
        return Result.failure(IllegalArgumentException("El paquete no es base64 válido."))
    }
    return restaurarDesdeSobre(material, envelope)
}
