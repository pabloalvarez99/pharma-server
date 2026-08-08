package cl.rutbusiness.app.ui.fiado

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * El fiado: quién le debe al negocio y cuánto ha ido pagando.
 *
 * **Ningún saldo se calcula acá.** `balance` es `total_charged − total_paid`
 * resuelto en `domain::credit::service`, y el total por cobrar es la suma que
 * hace el server sobre todos los deudores. Sumar los saldos en el teléfono daría
 * otro número el día que un cargo entre entre dos pantallas, y la dueña estaría
 * cobrando por un total que no existe.
 *
 * Los DTOs son el espejo de `crates/domain/src/credit/model.rs`. Ninguna llamada
 * lanza: todas vuelven en [Resultado].
 */
class FiadoApi(private val api: ApiFactory) {

    /**
     * Quién debe y cuánto: `GET /api/v1/reports/por-cobrar`.
     *
     * El server devuelve **sólo** a los que tienen deuda viva, ya ordenados por
     * saldo de mayor a menor. La pantalla respeta ese orden: reordenar en el
     * teléfono sería inventar un criterio distinto del que usa el resto del
     * sistema.
     */
    suspend fun deudores(): Resultado<DeudoresDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/reports/por-cobrar")
            .exigirExito(api.baseUrl)
            .body()
    }

    /** La cuenta corriente de un cliente: `GET /api/v1/customers/{id}/cuenta`. */
    suspend fun cuenta(clienteId: String): Resultado<CuentaDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/customers/$clienteId/cuenta")
            .exigirExito(api.baseUrl)
            .body()
    }

    /**
     * Registrar un pago del cliente: `POST /api/v1/customers/{id}/abono`.
     *
     * El server valida que el monto sea mayor que cero y que no pase de la
     * deuda; los dos rechazos vuelven con su mensaje en español y se muestran
     * tal cual.
     */
    suspend fun registrarAbono(
        clienteId: String,
        abono: NuevoAbono,
    ): Resultado<MovimientoDeCuentaDto> = llamar(api) {
        api.http.post("${api.baseUrl}/api/v1/customers/$clienteId/abono") {
            contentType(ContentType.Application.Json)
            setBody(abono)
        }.exigirExito(api.baseUrl).body()
    }
}

// --- lo que se le manda al server -------------------------------------------

@Serializable
data class NuevoAbono(
    /** Cuánto está pagando, en texto decimal. */
    val amount: String,
    /**
     * La caja abierta donde entra el billete (`cash_register_session:<id>`).
     *
     * Va sólo cuando paga en efectivo: así el abono entra al arqueo y el cierre
     * cuadra. Un pago por transferencia no toca el cajón y por eso viaja sin
     * este campo — mandarlo igual haría aparecer plata que nadie va a encontrar
     * al contar.
     */
    @SerialName("cash_session") val cajaAbierta: String? = null,
    val note: String? = null,
)

// --- lo que contesta el server ----------------------------------------------

/** `GET /api/v1/reports/por-cobrar`. */
@Serializable
data class DeudoresDto(
    @SerialName("total_por_cobrar") val total: String = "0",
    @SerialName("debtor_count") val cuantos: Int = 0,
    val rows: List<DeudorDto> = emptyList(),
)

/** Un cliente con deuda viva. */
@Serializable
data class DeudorDto(
    /** `customer:<id>`, que es lo que piden los otros dos endpoints. */
    val customer: String = "",
    val name: String = "",
    val phone: String? = null,
    val balance: String = "0",
    /** Último movimiento, para saber si la deuda está viva o quedó dormida. */
    @SerialName("last_movement") val ultimoMovimiento: String = "",
)

/** `GET /api/v1/customers/{id}/cuenta`. */
@Serializable
data class CuentaDto(
    val customer: String = "",
    /** `total_charged − total_paid`, resuelto por el server. Positivo = debe. */
    val balance: String = "0",
    @SerialName("total_charged") val totalFiado: String = "0",
    @SerialName("total_paid") val totalAbonado: String = "0",
    /** Los movimientos, del más nuevo al más viejo. */
    val entries: List<MovimientoDeCuentaDto> = emptyList(),
)

/** Un movimiento de la cuenta corriente: o se fió, o se pagó. */
@Serializable
data class MovimientoDeCuentaDto(
    val id: String = "",
    /** `cargo` = se llevó fiado; `abono` = pagó. El monto siempre es positivo. */
    val kind: String = "",
    val amount: String = "0",
    /** La venta que originó el cargo, cuando salió del mostrador. */
    val order: String? = null,
    val note: String? = null,
    @SerialName("created_at") val creadoEn: String = "",
) {
    val esAbono: Boolean get() = kind == "abono"
}
