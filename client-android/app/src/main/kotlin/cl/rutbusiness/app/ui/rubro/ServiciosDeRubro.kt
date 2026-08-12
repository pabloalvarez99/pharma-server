package cl.rutbusiness.app.ui.rubro

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.staticCompositionLocalOf
import cl.rutbusiness.core.rubro.PACK_OTRO
import cl.rutbusiness.core.rubro.RubroPack
import cl.rutbusiness.core.rubro.RubroPackRepository

/**
 * Pack de rubro vivo + repositorio, enchufado desde el `Activity`.
 *
 * Mismo patrón que offline/entrada: la plataforma arma el grafo; la UI solo
 * lee flags (`barcode`, `agent_home`, …) sin saber de HTTP.
 */
class ServiciosDeRubro(
    val repositorio: RubroPackRepository,
)

val LocalRubro = staticCompositionLocalOf<ServiciosDeRubro?> { null }

@Composable
fun ProveerRubro(servicios: ServiciosDeRubro, contenido: @Composable () -> Unit) {
    CompositionLocalProvider(LocalRubro provides servicios, content = contenido)
}

/**
 * Pack actual para gatear UI.
 *
 * Sin servicios (tests sueltos) se asume retail formal genérico: barcode on,
 * agent_home off - el comportamiento histórico de las pruebas de Cobrar.
 * Con repositorio, se suscribe al [StateFlow] para re-dibujar al cargar.
 */
@Composable
fun packActual(): RubroPack {
    val servicios = LocalRubro.current
    val repo = servicios?.repositorio
    if (repo == null) return PACK_OTRO
    val pack by repo.pack.collectAsState()
    return pack
}

/**
 * Si la UI tiene que hablar como en la feria (ADR-0022).
 *
 * El pack manda: `agent_home` es la bandera que dice "el agente es la casa", y
 * es lo que separa el copy de feria del de un rubro formal. El `rubro == "feria"`
 * queda de cinturón por si un server viejo manda el vertical sin las features.
 *
 * Vive acá y no adentro de cada pantalla porque "Hoy" y "Quién me debe" tienen
 * que contestar lo mismo: con la condición copiada en dos archivos, alcanza con
 * que una cambie para que el mismo teléfono hable como feriante en una pantalla
 * y como farmacia en la otra.
 */
@Composable
fun esFeria(): Boolean {
    val pack = packActual()
    return pack.features.agentHome || pack.rubro == "feria"
}
