package cl.rutbusiness.core.net

import io.ktor.client.engine.HttpClientEngine

/**
 * Motor HTTP de la plataforma.
 *
 * Es uno de los dos únicos puntos `expect/actual` del proyecto: en Android es
 * OkHttp (que además habilita TLS 1.2 en aparatos Android 5, donde el stack
 * nativo lo trae apagado); en iOS será Darwin. Nada más de la capa de red
 * cambia entre plataformas.
 */
expect fun defaultHttpClientEngine(): HttpClientEngine
