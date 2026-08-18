package cl.rutbusiness.app.ui.nav

import androidx.activity.compose.BackHandler
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import cl.rutbusiness.app.ui.caja.CajaRoute
import cl.rutbusiness.app.ui.fiado.FiadoRoute
import cl.rutbusiness.app.ui.gastos.GastosRoute
import cl.rutbusiness.app.ui.menu.MenuDelPuestoRoute
import cl.rutbusiness.app.ui.menu.PantallaDeCambiarRubro
import cl.rutbusiness.app.ui.menu.PantallaDeImpresoraDelMenu
import cl.rutbusiness.app.ui.resumen.ResumenRoute
import cl.rutbusiness.app.ui.ventas.VentasRoute
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository

/**
 * "Tu día": el resumen y todo lo que cuelga de él — caja, fiado, y el menú
 * del puesto con sus propias pantallas (impresora, cambiar de rubro).
 *
 * **Por qué la caja y el fiado no son pestañas propias.** La barra de abajo
 * está medida: tres etiquetas completas repartidas en 360dp con la letra al
 * 200% ya es el límite de lo que entra sin cortar una palabra — está escrito
 * en [BarraDeNavegacion] y lo prueba `BarraDeNavegacionEscalaTest`. Con cinco
 * pestañas cada una queda en 72dp y una etiqueta de dos palabras no cabe ni
 * partiéndola, o sea que la pestaña se queda sin nombre justo para la
 * persona que subió la letra porque ve poco.
 *
 * **Una sola pila, no dos `enum` anidados.** Hasta la ola 27 esto era un
 * `enum RutinaDelDia` con su `when`, y `MenuDelPuestoScreen.kt` tenía adentro
 * un segundo `enum SeccionDelMenu` con el suyo — dos dueños de la
 * navegación, sin back stack, cada uno manejando su propio botón físico de
 * atrás. La consecuencia: `CajaRoute` era alcanzable desde los dos (el
 * resumen y el menú) con semánticas de "volver" distintas según por dónde se
 * entraba. Con [rememberPilaDeNavegacion] hay una sola pila para todo el
 * árbol — ver [Pantalla] para el detalle y para dónde entran las pantallas
 * de las olas 29 a 33 — así que da lo mismo qué empujó [Pantalla.Caja]:
 * volver siempre saca la última pantalla empujada, y ninguna pantalla
 * necesita saber quién la abrió.
 *
 * Sigue sin `navigation-compose` (prohibido: sin dependencias de Gradle
 * nuevas) y sin `ViewModelStore` por entrada: los `ViewModel` de Caja, Fiado
 * e Impresora cuelgan del `Activity`, igual que en [ContenedorDeDestinos].
 */
@Composable
internal fun TuDiaRoute(sesion: SessionRepository, estado: EstadoSesion.Activa) {
    val pila = rememberPilaDeNavegacion(Pantalla.Resumen)

    // Volver de la caja o del fiado cambia justo los dos números que el
    // resumen muestra. Sin esto, la dueña abre la caja, vuelve, y "Tu día"
    // le sigue diciendo que no hay ninguna caja abierta — el `ViewModel`
    // sobrevive al cambio de pantalla y su carga inicial ya corrió.
    //
    // Se mira `pila.actual` **antes** de sacarla (la pantalla que se está
    // por abandonar), no la que queda arriba: da lo mismo si `Caja` se
    // empujó desde el resumen o desde el menú, lo que importa es de dónde se
    // vuelve, no adónde.
    var vueltas by rememberSaveable { mutableStateOf(0) }
    val volver: () -> Unit = {
        if (pila.actual == Pantalla.Caja || pila.actual == Pantalla.Fiado) vueltas += 1
        pila.volver()
    }

    // Mismo patrón que `ContenedorDeDestinos`: un solo `BackHandler` acá
    // arriba, habilitado mientras no se esté en la raíz. Las pantallas de
    // adentro (Caja, Fiado) pueden registrar el suyo propio para sus propios
    // pasos internos -ese es más profundo y gana-, pero "salir de esta
    // pantalla y volver a la anterior" es una sola regla y vive acá.
    BackHandler(enabled = !pila.enRaiz) { volver() }

    when (pila.actual) {
        Pantalla.Resumen -> ResumenRoute(
            sesion = sesion,
            estado = estado,
            onIrALaCaja = { pila.ir(Pantalla.Caja) },
            onIrAlFiado = { pila.ir(Pantalla.Fiado) },
            onAbrirMenu = { pila.ir(Pantalla.Menu) },
            recargasPedidas = vueltas,
        )

        Pantalla.Caja -> CajaRoute(sesion = sesion, estado = estado, onVolver = volver)

        Pantalla.Fiado -> FiadoRoute(sesion = sesion, estado = estado, onVolver = volver)

        Pantalla.Menu -> MenuDelPuestoRoute(
            onVolver = volver,
            onAbrirCaja = { pila.ir(Pantalla.Caja) },
            onAbrirImpresora = { pila.ir(Pantalla.Impresora) },
            onAbrirRubro = { pila.ir(Pantalla.Rubro) },
            onAbrirHistorialVentas = { pila.ir(Pantalla.HistorialVentas) },
            onAbrirGastos = { pila.ir(Pantalla.Gastos) },
        )

        Pantalla.Impresora -> PantallaDeImpresoraDelMenu(onVolver = volver)

        Pantalla.Rubro -> PantallaDeCambiarRubro(sesion = sesion, onVolver = volver)

        Pantalla.HistorialVentas -> VentasRoute(sesion = sesion, estado = estado, onVolver = volver)

        Pantalla.Gastos -> GastosRoute(sesion = sesion, estado = estado, onVolver = volver)
    }
}
