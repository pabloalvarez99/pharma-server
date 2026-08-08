package cl.rutbusiness.app

import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import cl.rutbusiness.app.camara.CamaraDeCodigosCameraX
import cl.rutbusiness.app.ui.RutBusinessApp
import cl.rutbusiness.app.ui.impresora.ProveerImpresora
import cl.rutbusiness.app.ui.offline.ProveerOffline
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
            // Los dos enchufes entre la plataforma y la UI. De acá para abajo
            // nadie sabe que existen CameraX ni el Bluetooth: las pantallas
            // piden `CamaraDeCodigos` o `Impresora` por `CompositionLocal` y
            // reciben una interfaz. Es lo que deja la capa de UI compilable
            // para iOS sin tocarla.
            CompositionLocalProvider(LocalCamaraDeCodigos provides camara) {
                ProveerImpresora(container.impresora) {
                    ProveerOffline(container.offline) {
                        RutBusinessApp(sesion = container.sesion)
                    }
                }
            }
        }
    }
}
