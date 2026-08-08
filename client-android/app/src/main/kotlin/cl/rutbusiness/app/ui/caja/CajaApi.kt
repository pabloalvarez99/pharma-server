package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.parameter
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * La caja del día: abrirla, moverle plata, arquearla y cerrarla.
 *
 * **Ni un peso se calcula acá.** Lo que debería haber en el cajón lo resuelve
 * `domain::cash_register::service::compute_summary` (apertura + ventas en
 * efectivo + entradas − salidas) y la diferencia del cierre la resuelve
 * `domain::invariants::discrepancy` (contado − esperado). Esta clase pide esos
 * números y los transporta. Rehacer la resta en Kotlin significaría que la
 * pantalla de arqueo puede decir un número distinto del que quedó grabado en el
 * cierre, y ese número no es un dato: es una acusación.
 *
 * Van a mano y no por el generador de OpenAPI porque los handlers declaran sus
 * bodies como `serde_json::Value` (`crates/api/src/v1/cash_register.rs`) y el
 * generador baja eso a `JsonElement`. Los tipos de acá son el espejo de
 * `crates/domain/src/cash_register/model.rs`.
 *
 * Ninguna llamada lanza: todas vuelven en [Resultado].
 */
class CajaApi(private val api: ApiFactory) {

    /**
     * Las cajas físicas configuradas: `GET /api/v1/cajas`.
     *
     * Un almacén de una sola caja recibe una lista de uno (o vacía, si nadie
     * configuró ninguna) y la pantalla no pregunta nada. Elegir caja sólo tiene
     * sentido cuando hay más de una, y es lo que decide dónde descuenta stock la
     * venta: la sesión hereda la sucursal de la caja.
     */
    suspend fun cajas(): Resultado<List<CajaFisicaDto>> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/cajas") {
            parameter("active", true)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * La caja abierta ahora, si hay: `GET /api/v1/cash-sessions?status=open`.
     *
     * El server permite **una caja abierta por cajero**, así que la primera de
     * la lista es la de quien está usando el teléfono.
     */
    suspend fun sesionAbierta(): Resultado<SesionDeCajaDto?> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/cash-sessions") {
            parameter("status", "open")
            parameter("limit", 5)
        }.exigirExito(api.baseUrl).body<List<SesionDeCajaDto>>().firstOrNull()
    }

    /** Abrir la caja: `POST /api/v1/cash-sessions`. */
    suspend fun abrir(apertura: AperturaDeCaja): Resultado<SesionDeCajaDto> = llamar(api) {
        api.http.post("${api.baseUrl}/api/v1/cash-sessions") {
            contentType(ContentType.Application.Json)
            setBody(apertura)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * El estado de la caja: `GET /api/v1/cash-sessions/{id}/arqueo`.
     *
     * En una caja abierta el server rellena `session.closing_cash_expected` con
     * lo que debería haber **ahora**, sin cerrar nada. Es lo que muestra la
     * pantalla de estado y lo que hace innecesario que el teléfono sume.
     */
    suspend fun arqueo(sesionId: String): Resultado<ArqueoDeCajaDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/cash-sessions/$sesionId/arqueo")
            .exigirExito(api.baseUrl)
            .body()
    }

    /** Lo que entró y salió del cajón: `GET /api/v1/cash-sessions/{id}/movements`. */
    suspend fun movimientos(sesionId: String): Resultado<List<MovimientoDeCajaDto>> =
        llamar(api) {
            api.http.get("${api.baseUrl}/api/v1/cash-sessions/$sesionId/movements")
                .exigirExito(api.baseUrl)
                .body()
        }

    /** Sacar o meter plata: `POST /api/v1/cash-sessions/{id}/movements`. */
    suspend fun moverPlata(
        sesionId: String,
        movimiento: NuevoMovimiento,
    ): Resultado<MovimientoDeCajaDto> = llamar(api) {
        api.http.post("${api.baseUrl}/api/v1/cash-sessions/$sesionId/movements") {
            contentType(ContentType.Application.Json)
            setBody(movimiento)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Cerrar con el conteo real: `POST /api/v1/cash-sessions/{id}/close`.
     *
     * Devuelve la sesión ya cerrada con `discrepancia` adentro. **Ésa** es la
     * diferencia que ve la dueña; no hay otra.
     */
    suspend fun cerrar(sesionId: String, cierre: CierreDeCaja): Resultado<ArqueoDeCajaDto> =
        llamar(api) {
            api.http.post("${api.baseUrl}/api/v1/cash-sessions/$sesionId/close") {
                contentType(ContentType.Application.Json)
                setBody(cierre)
            }.exigirExito(api.baseUrl).body()
        }
}

// --- lo que se le manda al server -------------------------------------------
//
// Los montos viajan como texto decimal, igual que vuelven: `rust_decimal` los
// lee con `serde(with = "rust_decimal::serde::str")` y nunca pasan por un
// `Double`. Los campos opcionales se omiten cuando son `null` -- `ApiFactory`
// configura `explicitNulls = false` -- y el server los toma por ausencia.

@Serializable
data class AperturaDeCaja(
    /** Con cuánta plata arranca el cajón. Texto decimal, no un número. */
    @SerialName("opening_cash") val apertura: String,
    /** La caja física elegida (`register:<id>`), si el negocio tiene más de una. */
    val register: String? = null,
    /**
     * Nombre libre de la caja, sólo cuando no se eligió una física. Si va
     * [register], el server toma el nombre del record y éste se ignora.
     */
    @SerialName("register_name") val nombreDeCaja: String? = null,
    val notes: String? = null,
)

@Serializable
data class NuevoMovimiento(
    /** `retiro` saca plata del cajón; `ingreso` la mete. El server valida. */
    val tipo: String,
    val amount: String,
    /** Por qué. El server lo exige, y con razón: un movimiento sin motivo es
     *  plata que desapareció. */
    val reason: String,
) {
    companion object {
        const val RETIRO = "retiro"
        const val INGRESO = "ingreso"
    }
}

@Serializable
data class CierreDeCaja(
    /** Lo que la dueña contó de verdad en el cajón. */
    @SerialName("closing_cash_counted") val contado: String,
    val notes: String? = null,
)

// --- lo que contesta el server ----------------------------------------------

/** Una caja física de `GET /api/v1/cajas`. */
@Serializable
data class CajaFisicaDto(
    val id: String = "",
    val name: String = "",
    val code: String? = null,
    val active: Boolean = true,
    val branch: String? = null,
)

/** Una sesión de caja. Espejo de `CashSessionDto`. */
@Serializable
data class SesionDeCajaDto(
    val id: String = "",
    @SerialName("register_name") val nombreDeCaja: String = "",
    val register: String? = null,
    /** Con cuánto se abrió. */
    @SerialName("opening_cash") val apertura: String = "0",
    @SerialName("opening_notes") val notaDeApertura: String? = null,
    /** Lo que se contó al cerrar. `null` mientras la caja siga abierta. */
    @SerialName("closing_cash_counted") val contado: String? = null,
    /**
     * Lo que **debería** haber. En una caja abierta lo rellena el arqueo; en una
     * cerrada es el que quedó grabado.
     */
    @SerialName("closing_cash_expected") val esperado: String? = null,
    /**
     * `contado − esperado`, calculado por el server. Negativo = falta plata,
     * positivo = sobra, cero = cuadró. `null` mientras no se haya cerrado.
     */
    val discrepancia: String? = null,
    @SerialName("closing_notes") val notaDeCierre: String? = null,
    @SerialName("opened_at") val abiertaEn: String = "",
    @SerialName("closed_at") val cerradaEn: String? = null,
    val status: String = "",
) {
    val abierta: Boolean get() = status == "open"
}

/** `GET /api/v1/cash-sessions/{id}/arqueo` y la respuesta del cierre. */
@Serializable
data class ArqueoDeCajaDto(
    val session: SesionDeCajaDto = SesionDeCajaDto(),
    /** Lo cobrado en efectivo mientras la caja estuvo abierta. */
    @SerialName("cash_sales") val ventasEnEfectivo: String = "0",
    @SerialName("movements_in") val entradas: String = "0",
    @SerialName("movements_out") val salidas: String = "0",
)

/** Un movimiento manual del cajón. */
@Serializable
data class MovimientoDeCajaDto(
    val id: String = "",
    val tipo: String = "",
    val amount: String = "0",
    val reason: String = "",
    @SerialName("created_at") val creadoEn: String = "",
) {
    val esRetiro: Boolean get() = tipo == NuevoMovimiento.RETIRO
}
