package cl.rutbusiness.app

import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import cl.rutbusiness.app.camara.CamaraDeCodigosCameraX
import cl.rutbusiness.app.ui.RutBusinessApp
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos

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
        val camara = if (packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)) {
            CamaraDeCodigosCameraX
        } else {
            // Sin cámara en el aparato, la pantalla de cobrar ni siquiera
            // ofrece el botón de escanear. Preguntarlo acá, una vez, evita que
            // cada pantalla tenga que saber de `PackageManager`.
            null
        }

        setContent {
            // El único enchufe entre la plataforma y la UI: de acá para abajo
            // nadie sabe que existe CameraX.
            CompositionLocalProvider(LocalCamaraDeCodigos provides camara) {
                RutBusinessApp(sesion = container.sesion)
            }
        }
    }
}
