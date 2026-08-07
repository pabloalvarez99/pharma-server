package cl.rutbusiness.app.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import cl.rutbusiness.app.ui.assist.AssistRoute
import cl.rutbusiness.app.ui.cobrar.CobrarRoute
import cl.rutbusiness.app.ui.login.LoginRoute
import cl.rutbusiness.app.ui.nav.ContenedorDeDestinos
import cl.rutbusiness.app.ui.nav.Destino
import cl.rutbusiness.app.ui.nav.TuDiaRoute
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbLoadingState
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Raíz de la UI. Kotlin común: ni un solo `import android.*` de acá para abajo.
 *
 * [RbTheme] envuelve **todo** una sola vez, acá en la raíz. Debajo nadie lee
 * `MaterialTheme`: los tokens salen del design system, que es el que carga las
 * garantías de contraste y de tamaño táctil.
 *
 * Ése fue el punto en disputa al juntar las dos ramas. La otra dejaba login y
 * productos en `MaterialTheme` a propósito, para no moverles los colores sin que
 * nadie los hubiera revisado. El cuidado era legítimo; el precio, una app con
 * dos sistemas de color conviviendo, donde el que quedaba afuera era justo el
 * que tiene el contraste medido — `RbContrastTest` rompe el build si alguien
 * mueve un valor, y de las pantallas en `MaterialTheme` no sabe nada. Una
 * pantalla que se ve mal bajo el design system es un bug que se arregla y se
 * nota; media app sin garantía de contraste no se nota hasta que alguien no
 * puede leerla.
 */
@Composable
fun RutBusinessApp(sesion: SessionRepository) {
    val estado by sesion.estado.collectAsState()

    RbTheme {
        when (val actual = estado) {
            EstadoSesion.Cargando -> RbLoadingState(label = "Abriendo RutBusiness...")
            is EstadoSesion.SinSesion -> LoginRoute(sesion = sesion, estado = actual)
            is EstadoSesion.Activa -> ConSesion(sesion = sesion, estado = actual)
        }
    }
}

/**
 * Qué ve la dueña una vez adentro.
 *
 * Arranca en el **agente**. Directiva del founder: la dueña no navega menús, le
 * habla al negocio; todo lo demás existe para cuando hablar no alcanza.
 *
 * La navegación dejó de ser un booleano. Con tres destinos, un `if` obliga a
 * elegir cuál se pierde — y en este merge el que se perdía era `CobrarRoute`,
 * la pantalla más importante del producto.
 *
 * Sigue sin haber `navigation-compose`, y ahora la razón es más fuerte que "son
 * pocos destinos": esto es una barra de pestañas, no una pila. Las tres
 * pantallas son hermanas y ninguna se apila sobre otra, así que un `NavHost`
 * traería un back stack que habría que desarmar, `saveState`/`restoreState` que
 * ajustar, y un `ViewModelStore` por entrada de la pila — que es exactamente lo
 * que pone en riesgo el carrito. Un `enum` y un `SaveableStateHolder` hacen lo
 * mismo con lo que ya está en el binario; el armazón vive en
 * [ContenedorDeDestinos] y este archivo sólo dice qué pantalla es cada destino.
 *
 * Perder un carrito a medias es el peor bug posible en un punto de venta, así
 * que no queda en promesa: lo prueba `NavegacionTest`.
 */
@Composable
private fun ConSesion(sesion: SessionRepository, estado: EstadoSesion.Activa) {
    ContenedorDeDestinos { destino ->
        when (destino) {
            Destino.Agente -> AssistRoute(sesion = sesion)
            Destino.Cobrar -> CobrarRoute(sesion = sesion, estado = estado)
            // "Tu día" no es sólo el resumen: de ahí salen las dos rutinas
            // diarias -- abrir y cerrar la caja, y cobrar el fiado. Por qué no
            // son pestañas propias está explicado en [TuDiaRoute].
            Destino.Resumen -> TuDiaRoute(sesion = sesion, estado = estado)
        }
    }
}
