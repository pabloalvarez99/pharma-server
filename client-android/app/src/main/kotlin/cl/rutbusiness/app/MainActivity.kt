package cl.rutbusiness.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import cl.rutbusiness.app.ui.RutBusinessApp

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
            RutBusinessApp(sesion = container.sesion)
        }
    }
}
