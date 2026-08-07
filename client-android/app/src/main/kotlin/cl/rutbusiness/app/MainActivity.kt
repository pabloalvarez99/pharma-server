package cl.rutbusiness.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import cl.rutbusiness.app.ui.RutBusinessApp
import cl.rutbusiness.app.ui.impresora.ProveerImpresora

/**
 * Único `Activity` de la app. Su trabajo es exactamente uno: montar Compose.
 *
 * Disciplina CMP: todo lo que está debajo de [RutBusinessApp] es Kotlin común y
 * no conoce `android.*`. Este archivo es la frontera; acá sí vale importar
 * Android porque es el punto de entrada de la plataforma.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val container = (application as RutBusinessApplication).container
        setContent {
            // La impresora entra por acá, en la frontera de plataforma, y baja
            // por `CompositionLocal`. `RutBusinessApp` y las pantallas del
            // medio no se enteran de que existe: la única que la pide es la
            // tarjeta de boleta, en el fondo de la pantalla de cobro.
            ProveerImpresora(container.impresora) {
                RutBusinessApp(sesion = container.sesion)
            }
        }
    }
}
