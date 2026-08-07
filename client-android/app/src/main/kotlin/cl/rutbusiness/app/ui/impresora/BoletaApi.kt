package cl.rutbusiness.app.ui.impresora

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import cl.rutbusiness.core.session.SessionRepository
import io.ktor.client.call.body
import io.ktor.client.request.get

/**
 * De dónde sale lo que se imprime.
 *
 * `GET /api/v1/orders/{id}/receipt`, el mismo comprobante que ya muestra la
 * pantalla de cobro. Se vuelve a pedir en vez de reusar el `ComprobanteDto` que
 * el ViewModel tiene en memoria por dos razones:
 *
 * 1. `ComprobanteDto` no declara `datetime`, `card_amount` ni `cashier`, y los
 *    tres van en el papel del escritorio. Pedirlo acá con el DTO completo evita
 *    tocar el modelo compartido y evita que la boleta del teléfono salga
 *    distinta de la del PC.
 * 2. **Reimprimir tiene que andar con la app recién abierta.** Se guarda el id
 *    de la orden, no la boleta armada; el papel dice lo que dice la venta
 *    guardada, aunque hayan pasado tres ventas o un reinicio en el medio.
 *
 * El endpoint no está en el OpenAPI (no tiene `#[utoipa::path]`), así que va a
 * mano, igual que `PosRepository.comprobante`.
 */
class BoletaApi(private val api: ApiFactory) {

    suspend fun boleta(ordenId: String): Resultado<BoletaImprimible> = llamar(api.baseUrl) {
        api.http.get("${api.baseUrl}/api/v1/orders/$ordenId/receipt")
            .exigirExito(api.baseUrl)
            .body()
    }
}

/**
 * De dónde saca el `ViewModel` lo que hay que imprimir.
 *
 * Existe por la misma razón que [EnlaceDeImpresora]: para que los caminos de
 * falla —impresora apagada, sin permiso, fuera de alcance— se puedan probar sin
 * levantar un servidor. Ésos son el 80% del trabajo real de este encargo y
 * tienen que estar cubiertos por prueba, no por la buena intención de alguien
 * que enchufó una impresora una vez.
 */
interface FuenteDeBoletas {
    suspend fun boleta(ordenId: String): Intento<BoletaImprimible>
}

/** La de verdad: le pregunta al server con el token de la sesión activa. */
class BoletasDelServidor(private val sesion: SessionRepository) : FuenteDeBoletas {
    override suspend fun boleta(ordenId: String): Intento<BoletaImprimible> {
        val api = sesion.apiActiva()
            ?: return Intento.Fallo(FallaDeImpresion.SinDatosDeLaBoleta("sin sesión activa"))

        return when (val r = BoletaApi(api).boleta(ordenId)) {
            is Resultado.Ok -> Intento.Ok(r.valor)
            is Resultado.Falla -> Intento.Fallo(
                FallaDeImpresion.SinDatosDeLaBoleta(r.error.technical ?: r.error.userMessage),
            )
        }
    }
}
