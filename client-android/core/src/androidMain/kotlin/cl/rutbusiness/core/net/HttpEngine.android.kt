package cl.rutbusiness.core.net

import io.ktor.client.engine.HttpClientEngine
import io.ktor.client.engine.okhttp.OkHttp

/**
 * OkHttp y no el motor nativo de Android a propósito: en Android 5 (API 21-22)
 * `HttpsURLConnection` trae TLS 1.2 **apagado** por defecto, así que cualquier
 * server en la nube con TLS moderno falla. OkHttp lo prende solo.
 */
actual fun defaultHttpClientEngine(): HttpClientEngine = OkHttp.create()
