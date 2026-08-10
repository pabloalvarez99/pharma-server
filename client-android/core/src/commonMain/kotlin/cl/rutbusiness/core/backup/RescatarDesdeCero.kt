package cl.rutbusiness.core.backup

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado

/**
 * Teléfono nuevo, app recién instalada, nada adentro (ADR-0023).
 *
 * Éste es el caso que justifica que el respaldo exista. Los otros dos caminos
 * —listar y bajar con sesión— sirven para un aparato que todavía funciona, y un
 * aparato que todavía funciona no necesita el respaldo. La persona que sí lo
 * necesita perdió el teléfono: no tiene sesión, no tiene token guardado, no
 * tiene la app. Lo único que conserva es la tarjeta del cuaderno.
 *
 * Lo que hace falta de esa tarjeta, y nada más:
 *
 * - el **nombre del negocio** (slug), impreso arriba;
 * - las **12 palabras** o los **5 bloques** (cualquiera de los dos: son los
 *   mismos 84 bits, ver [ClaveDelNegocio]).
 *
 * Los dos PBKDF2 —el de la prueba de retiro y el de la llave del sobre— corren
 * en este teléfono. El server nunca ve ninguno de los dos secretos: recibe una
 * prueba que sólo sirve para decir "sí, bajá ese sobre", y devuelve bytes que
 * no puede abrir.
 *
 * ## Se usa con un [ApiFactory] sin token
 *
 * ```kotlin
 * val api = ApiFactory(baseUrl) { null }   // no hay sesión, y está bien
 * val r = rescatarRespaldoDesdeCero(UserBackupApi(api), slug, materialRaw)
 * ```
 *
 * Si esto necesitara un token, el respaldo no existiría.
 */
suspend fun rescatarRespaldoDesdeCero(
    backup: UserBackupApi,
    tenantSlug: String,
    materialRaw: String,
): Result<RestauracionRespaldo> {
    val slug = PruebaDeRetiro.normalizarSlug(tenantSlug)
    if (slug.isEmpty()) {
        return Result.failure(
            IllegalArgumentException("Escribí el nombre del negocio, tal como está en la tarjeta."),
        )
    }

    // Se valida el material ANTES de salir a la red. Una palabra mal escrita se
    // detecta acá, con el número de palabra, en vez de volver como un 404 del
    // server que no distingue "escribiste mal" de "ese negocio no existe".
    val material = parsearMaterialRecuperacion(materialRaw).getOrElse {
        return Result.failure(IllegalArgumentException(mensajeErrorMaterial(materialRaw)))
    }
    semillaDeMaterial(material).getOrElse {
        return Result.failure(IllegalArgumentException(mensajeErrorMaterial(materialRaw)))
    }

    val prueba = PruebaDeRetiro.derivarHex(material, slug).getOrElse {
        return Result.failure(IllegalArgumentException(mensajeErrorMaterial(materialRaw)))
    }

    val descarga = when (val r = backup.rescatar(slug, prueba)) {
        is Resultado.Ok -> r.valor
        // Todo lo que falla del lado del server vuelve 404 sin cuerpo, a
        // propósito. El mensaje enumera lo revisable en vez de fingir que sabe
        // cuál de las tres causas fue.
        is Resultado.Falla -> return Result.failure(
            IllegalStateException(mensajeRescateFallido(slug)),
        )
    }

    val envelope = try {
        base64ToEnvelope(descarga.ciphertextBase64)
    } catch (e: Exception) {
        return Result.failure(IllegalStateException("El respaldo llegó dañado. Probá de nuevo."))
    }

    // El segundo PBKDF2, con el salt que viene DENTRO del sobre. Por eso el de
    // la prueba de retiro tenía que usar un salt distinto y derivable del slug:
    // sin bajar el sobre no hay salt, y sin prueba no hay sobre.
    return restaurarDesdeSobre(material, envelope)
}
