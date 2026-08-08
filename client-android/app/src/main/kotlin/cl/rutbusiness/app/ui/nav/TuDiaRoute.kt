package cl.rutbusiness.app.ui.nav

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import cl.rutbusiness.app.ui.caja.CajaRoute
import cl.rutbusiness.app.ui.fiado.FiadoRoute
import cl.rutbusiness.app.ui.resumen.ResumenRoute
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository

/** Las tres cosas que se hacen desde "Tu día". */
private enum class RutinaDelDia { Resumen, Caja, Fiado }

/**
 * "Tu día": el resumen y las dos rutinas que salen de él.
 *
 * **Por qué la caja y el fiado no son pestañas propias.** La barra de abajo está
 * medida: tres etiquetas completas repartidas en 360dp con la letra al 200% ya
 * es el límite de lo que entra sin cortar una palabra — está escrito en
 * [BarraDeNavegacion] y lo prueba `BarraDeNavegacionEscalaTest`. Con cinco
 * pestañas cada una queda en 72dp y "Resumen" no cabe ni partiéndola, o sea que
 * la pestaña se queda sin nombre justo para la persona que subió la letra porque
 * ve poco. Cambiar eso para meter dos destinos sería pagar la accesibilidad de
 * toda la app con un toque de ahorro.
 *
 * Y el lugar no es un premio de consuelo: el resumen ya contesta "cuánta plata
 * hay en la caja" y "cuánto te deben", así que abrir la caja y cobrar el fiado
 * salen del mismo bloque que hace la pregunta. Son dos toques desde que se prende
 * el teléfono.
 *
 * Sub-navegación con un `enum` en `rememberSaveable`, igual que
 * [ContenedorDeDestinos] y por lo mismo: son tres pantallas, un `NavHost` traería
 * un back stack y un `ViewModelStore` por entrada para no ganar nada. El botón
 * físico de atrás lo maneja cada pantalla de adentro, y el suyo gana por ser el
 * más profundo.
 */
@Composable
internal fun TuDiaRoute(sesion: SessionRepository, estado: EstadoSesion.Activa) {
    var rutina by rememberSaveable { mutableStateOf(RutinaDelDia.Resumen) }

    // Volver de la caja o del fiado cambia justo los dos números que el resumen
    // muestra. Sin esto, la dueña abre la caja, vuelve, y "Tu día" le sigue
    // diciendo que no hay ninguna caja abierta — el `ViewModel` sobrevive al
    // cambio de pantalla y su carga inicial ya corrió.
    var vueltas by rememberSaveable { mutableStateOf(0) }

    val volver: () -> Unit = {
        vueltas += 1
        rutina = RutinaDelDia.Resumen
    }

    when (rutina) {
        RutinaDelDia.Resumen -> ResumenRoute(
            sesion = sesion,
            estado = estado,
            onIrALaCaja = { rutina = RutinaDelDia.Caja },
            onIrAlFiado = { rutina = RutinaDelDia.Fiado },
            recargasPedidas = vueltas,
        )

        RutinaDelDia.Caja -> CajaRoute(sesion = sesion, estado = estado, onVolver = volver)

        RutinaDelDia.Fiado -> FiadoRoute(sesion = sesion, estado = estado, onVolver = volver)
    }
}
