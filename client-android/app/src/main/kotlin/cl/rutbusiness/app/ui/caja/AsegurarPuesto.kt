package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado

/**
 * Resultado de dejar el puesto de feria listo para vender.
 *
 * No calcula plata: solo abre con $0 o trata el 409 ("ya tiene caja abierta")
 * como éxito y recarga la sesión abierta.
 */
sealed class ResultadoPuesto {
    data class Abierto(val sesion: SesionDeCajaDto) : ResultadoPuesto()
    data class Falla(val error: AppError) : ResultadoPuesto()
}

/**
 * Asegura una sesión de caja abierta para el día de feria.
 *
 * 1. Si ya hay sesión `open` → listo.
 * 2. Si no → `POST` con apertura `"0"`, nombre `"puesto"`, nota «Día de feria».
 * 3. Si el server contesta 409 / "ya tiene una caja" → vuelve a leer la abierta
 *    y la trata como éxito (otro flujo la abrió primero).
 *
 * Puro sobre [CajaApi]: testeable sin ViewModel ni Compose.
 */
suspend fun asegurarPuestoAbierto(caja: CajaApi): ResultadoPuesto {
    when (val abierta = caja.sesionAbierta()) {
        is Resultado.Ok -> {
            val sesion = abierta.valor
            if (sesion != null) return ResultadoPuesto.Abierto(sesion)
        }
        is Resultado.Falla -> return ResultadoPuesto.Falla(abierta.error)
    }

    val apertura = AperturaDeCaja(
        apertura = "0",
        register = null,
        nombreDeCaja = NOMBRE_PUESTO_FERIA,
        notes = NOTA_DIA_DE_FERIA,
    )

    return when (val abierta = caja.abrir(apertura)) {
        is Resultado.Ok -> ResultadoPuesto.Abierto(abierta.valor)
        is Resultado.Falla -> {
            if (!esCajaYaAbierta(abierta.error)) {
                return ResultadoPuesto.Falla(abierta.error)
            }
            when (val deNuevo = caja.sesionAbierta()) {
                is Resultado.Ok -> {
                    val sesion = deNuevo.valor
                    if (sesion != null) ResultadoPuesto.Abierto(sesion)
                    else ResultadoPuesto.Falla(abierta.error)
                }
                is Resultado.Falla -> ResultadoPuesto.Falla(deNuevo.error)
            }
        }
    }
}

/** Variante con [ApiFactory] para pantallas que no tienen [CajaApi] armado. */
suspend fun asegurarPuestoAbierto(api: ApiFactory): ResultadoPuesto =
    asegurarPuestoAbierto(CajaApi(api))

internal const val NOMBRE_PUESTO_FERIA = "puesto"
internal const val NOTA_DIA_DE_FERIA = "Día de feria"
