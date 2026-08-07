package cl.rutbusiness.app.ui.nav

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.systemBars
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier

/**
 * El armazón de la navegación: qué pestaña está elegida, qué se dibuja arriba
 * de la barra, y qué hace el botón físico de atrás.
 *
 * No conoce ninguna pantalla — recibe [contenido] y le pasa el destino. Eso no
 * es abstracción por gusto: es lo que deja probar la parte que **puede
 * romperse** (que el estado de una pestaña sobreviva al cambio a otra) sin
 * levantar un servidor ni un carrito real. `NavegacionTest` monta este mismo
 * contenedor con pantallas de mentira y verifica lo que el producto promete.
 *
 * **Por qué el estado sobrevive.** El [rememberSaveableStateHolder] guarda el
 * `rememberSaveable` de cada destino bajo su propia llave, y lo devuelve al
 * volver. Los `ViewModel` no dependen de esto: `viewModel()` los cuelga del
 * `ViewModelStore` del `Activity` y salir de la composición no los limpia. Las
 * dos capas juntas son lo que hace que un carrito a medias siga ahí después de
 * ir a preguntarle algo al agente.
 */
@Composable
internal fun ContenedorDeDestinos(
    modifier: Modifier = Modifier,
    inicial: Destino = Destino.Agente,
    contenido: @Composable (Destino) -> Unit,
) {
    var destino by rememberSaveable { mutableStateOf(inicial) }
    val estados = rememberSaveableStateHolder()

    // Atrás desde una pestaña secundaria vuelve al inicio en vez de cerrar la
    // app de golpe. En [inicial] queda deshabilitado y el sistema hace lo suyo:
    // en la pantalla de entrada, atrás sí significa salir, y atraparlo ahí
    // dejaría a la dueña sin forma de cerrar la app.
    //
    // Las pantallas de adentro registran su propio `BackHandler` -- Cobrar lo
    // usa para volver de Pago a Buscar -- y el más profundo gana, así que atrás
    // deshace el último paso antes de cambiar de pestaña. Es el orden en que la
    // gente espera que se deshagan las cosas.
    BackHandler(enabled = destino != inicial) {
        destino = inicial
    }

    Column(modifier = modifier.fillMaxSize()) {
        // `weight(1f)` y no `fillMaxSize()`: el contenido ocupa lo que sobra
        // después de la barra. Con `fillMaxSize` la barra queda empujada fuera
        // de la pantalla, y al 200% eso pasa siempre.
        Column(
            modifier = Modifier
                .weight(1f)
                // La barra de abajo ya se separó de la barra de gestos del
                // sistema, y las pantallas de adentro piden ese mismo margen por
                // su cuenta -- el redactor del agente y la barra de cobrar de
                // `PasoBuscar` lo hacían cuando eran lo último de la pantalla.
                // Sin consumirlo acá, el margen se aplica dos veces y queda una
                // franja muerta de dos centímetros entre el botón de enviar y
                // las pestañas.
                //
                // Se consume `systemBars` y no `safeDrawing`: `safeDrawing`
                // incluye el teclado, y consumirlo dejaría el campo de texto
                // tapado justo mientras se escribe.
                .consumeWindowInsets(WindowInsets.systemBars.only(WindowInsetsSides.Bottom)),
        ) {
            estados.SaveableStateProvider(destino.name) {
                contenido(destino)
            }
        }

        BarraDeNavegacion(
            actual = destino,
            onElegir = { elegido -> destino = elegido },
        )
    }
}
