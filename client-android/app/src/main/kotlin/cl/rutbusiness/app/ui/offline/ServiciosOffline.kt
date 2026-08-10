package cl.rutbusiness.app.ui.offline

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.State
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.staticCompositionLocalOf
import cl.rutbusiness.core.offline.CacheDeLecturas
import cl.rutbusiness.core.offline.ColaDeVentas
import cl.rutbusiness.core.offline.ConexionConElNegocio
import cl.rutbusiness.core.offline.DespachadorDeVentas
import cl.rutbusiness.core.offline.VentaEnCola

/**
 * Lo que la app necesita para andar sin señal, enchufado desde el `Activity`.
 *
 * Mismo patrón que la impresora y la cámara: la plataforma se arma arriba, la
 * UI recibe interfaces y ningún composable importa `android.*`. Todo lo de acá
 * es Kotlin común y compilaría igual para iOS.
 */
class ServiciosOffline(
    val conexion: ConexionConElNegocio,
    val cola: ColaDeVentas,
    val cache: CacheDeLecturas,
    val despachador: DespachadorDeVentas,
    /** El reloj de pared, para poder decir "datos de hace 20 minutos". */
    val reloj: () -> Long,
)

/**
 * `null` cuando nadie los proveyó.
 *
 * Las pruebas de otras pantallas montan composables sueltos sin todo el grafo,
 * y una pantalla que revienta por no encontrar la cola sería una pantalla que
 * no se puede probar. Sin servicios offline la app se comporta como antes de
 * este trabajo: asume que hay red y falla si no la hay.
 */
val LocalOffline = staticCompositionLocalOf<ServiciosOffline?> { null }

@Composable
fun ProveerOffline(servicios: ServiciosOffline, contenido: @Composable () -> Unit) {
    CompositionLocalProvider(LocalOffline provides servicios, content = contenido)
}

/**
 * `true` mientras se esté llegando al sistema del negocio.
 *
 * Sin servicios provistos devuelve `true`: es el comportamiento de siempre, y
 * asumir "sin conexión" en una prueba haría que media UI se dibujara en su
 * estado degradado sin que nadie lo hubiera pedido.
 */
@Composable
fun hayConexion(): Boolean {
    val servicios = LocalOffline.current ?: return true
    return servicios.conexion.hayConexion.collectAsState().value
}

/** Las ventas que están esperando salir, para el cartel y la lista. */
@Composable
fun ventasEnCola(): State<List<VentaEnCola>>? =
    LocalOffline.current?.cola?.ventas?.collectAsState()
