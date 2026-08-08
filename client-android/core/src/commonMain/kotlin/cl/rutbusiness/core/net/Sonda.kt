package cl.rutbusiness.core.net

import io.ktor.client.request.get
import io.ktor.http.isSuccess

/**
 * ¿Está contestando el sistema del negocio?
 *
 * `GET /health/ready` es público, no lleva token y contesta un JSON de dos
 * líneas. Se usa para una sola cosa: cuando el sistema operativo avisa que
 * volvió el wifi, preguntar si además volvió el server — el enlace no dice
 * nada sobre si el PC del local está prendido.
 *
 * No pasa por [llamar] a propósito: una sonda que falla **es** la respuesta que
 * se está buscando, no un error que reportar. Si entrara por el embudo normal
 * se avisaría a sí misma y el estado se mordería la cola.
 */
suspend fun ApiFactory.contestaElServidor(): Boolean =
    runCatching { http.get("$baseUrl/health/ready").status.isSuccess() }.getOrDefault(false)
