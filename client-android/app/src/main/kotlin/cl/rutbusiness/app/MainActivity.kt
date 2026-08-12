package cl.rutbusiness.app

import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import cl.rutbusiness.app.camara.CamaraDeCodigosCameraX
import cl.rutbusiness.app.ui.RutBusinessApp
import cl.rutbusiness.app.entrada.CompartirTarjetaAndroid
import cl.rutbusiness.app.ui.assist.LocalDictadoDeVoz
import cl.rutbusiness.app.ui.entrada.LocalCompartirTarjeta
import cl.rutbusiness.app.ui.entrada.ProveerEntrada
import cl.rutbusiness.app.ui.impresora.ProveerImpresora
import cl.rutbusiness.app.ui.offline.ProveerOffline
import cl.rutbusiness.app.ui.rubro.ProveerRubro
import cl.rutbusiness.app.ui.scanner.LocalCamaraDeCodigos
import cl.rutbusiness.app.voz.DictadoDeVozAndroid
import cl.rutbusiness.app.voz.hayDictadoDeVoz

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

        // Lo mismo con el micrófono: si este teléfono no tiene con qué
        // escuchar -sin micrófono, o sin ningún motor de reconocimiento
        // instalado- la pantalla del agente no dibuja el control de voz y se
        // sigue escribiendo. Preguntarlo acá, una vez, evita que la pantalla
        // tenga que saber de `PackageManager` ni de `SpeechRecognizer`.
        val dictado = if (hayDictadoDeVoz()) DictadoDeVozAndroid else null

        // Fuera de `setContent` a propósito: adentro se recompone, y armar los
        // servicios de entrada en cada recomposición crearía un
        // `IdentidadGoogleAndroid` nuevo por frame. Acá se arma una vez, con el
        // contexto de esta Activity — que es el que el selector de cuentas de
        // Google necesita para poder montarse (ver `AppContainer.entradaPara`).
        val entrada = container.entradaPara(this)

        setContent {
            // Los enchufes entre la plataforma y la UI. De acá para abajo nadie
            // sabe que existen CameraX, el `SpeechRecognizer`, el Bluetooth ni
            // el `ConnectivityManager`: las pantallas piden `CamaraDeCodigos`,
            // `DictadoDeVoz`, `Impresora` o `RedDelTelefono` por
            // `CompositionLocal` y reciben una interfaz. Es lo que deja la capa
            // de UI compilable para iOS sin tocarla.
            CompositionLocalProvider(
                LocalCamaraDeCodigos provides camara,
                LocalDictadoDeVoz provides dictado,
                LocalCompartirTarjeta provides CompartirTarjetaAndroid(this),
            ) {
                ProveerImpresora(container.impresora) {
                    ProveerOffline(container.offline) {
                        ProveerEntrada(entrada) {
                            ProveerRubro(container.rubro) {
                                RutBusinessApp(sesion = container.sesion)
                            }
                        }
                    }
                }
            }
        }
    }
}
