package cl.rutbusiness.app.voz

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import cl.rutbusiness.app.ui.assist.DictadoDeVoz
import cl.rutbusiness.app.ui.assist.SesionDeDictado

/**
 * El dictado de verdad: el `SpeechRecognizer` del sistema.
 *
 * Este archivo es **el único** de la app que sabe que existe un reconocedor de
 * voz; `ui/assist` habla con la interfaz [DictadoDeVoz] y no importa nada de
 * `android.*`. El enchufe se hace en `MainActivity`, que es la frontera de
 * plataforma.
 *
 * Nada de Google Speech, ML Kit ni un modelo propio: el motor del aparato
 * alcanza, no agrega megas al APK -que en el teléfono objetivo se cuentan- y no
 * manda el audio a ningún server nuestro.
 */
object DictadoDeVozAndroid : DictadoDeVoz {

    private const val TAG = "RbVoz"

    /**
     * Los idiomas que se prueban, en ese orden.
     *
     * `es-CL` primero porque es como habla quien va a usar esto: "luca", "kilo
     * de tomates", "fiado a la señora Ana". `es-ES` después, porque un motor sin
     * el pack chileno casi siempre tiene el genérico. `null` al final significa
     * "no le digas nada y que use el idioma del teléfono": es mejor entender a
     * medias que no escuchar.
     *
     * No se elige adivinando: el motor avisa con
     * `ERROR_LANGUAGE_NOT_SUPPORTED` / `ERROR_LANGUAGE_UNAVAILABLE` y ahí se
     * baja un escalón. Los aparatos viejos nunca mandan esos códigos -llegaron
     * en Android 13- y simplemente se quedan en el primero, que es lo que hacen
     * hoy sin esta lista.
     */
    private val IDIOMAS: List<String?> = listOf("es-CL", "es-ES", null)

    @Composable
    override fun recordarSesion(onTexto: (String) -> Unit): SesionDeDictado {
        val contexto = LocalContext.current
        // La lambda cambia en cada recomposición del padre; el motor se queda
        // con ésta y siempre llama a la última.
        val alReconocer by rememberUpdatedState(onTexto)

        val motor = remember(contexto) { MotorDeDictado(contexto) }

        val lanzador = rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { concedido ->
            // Concedido, arranca sola: la dueña ya dijo que quería hablar
            // cuando tocó el botón, hacerla tocarlo de nuevo es cobrarle el
            // trámite dos veces. Negado, se lo dice en una línea y sigue el
            // teclado; no hay diálogo, no hay callejón.
            if (concedido) motor.empezar { texto -> alReconocer(texto) } else motor.sinPermiso()
        }

        // Salir de la pantalla suelta el reconocedor. Uno vivo mantiene abierto
        // el servicio de reconocimiento y, mientras escucha, el micrófono: en un
        // teléfono de 1-2 GB eso se paga en RAM y en batería del turno entero.
        DisposableEffect(motor) {
            onDispose { motor.soltar() }
        }

        return remember(motor, lanzador, contexto) {
            object : SesionDeDictado {
                override val escuchando: Boolean get() = motor.escuchando

                override val procesando: Boolean get() = motor.procesando

                override val aviso: String? get() = motor.aviso

                override fun alternar() {
                    when (
                        accionAlTocarVoz(
                            escuchando = motor.escuchando,
                            procesando = motor.procesando,
                            tienePermiso = contexto.tienePermisoDeMicrofono(),
                        )
                    ) {
                        // Segundo toque: "ya dije". Corta y manda lo que
                        // alcanzó a entender, en vez de tirarlo -que es lo que
                        // haría `cancel()`- porque quien lo aprieta acaba de
                        // terminar la frase, no se arrepintió.
                        AccionAlTocarVoz.CORTAR -> motor.cortar()

                        // Tercer toque, en pleno procesando: acá vivía la
                        // carrera de la ola 23. No hacer nada es la salida
                        // segura -el reconocedor ya va a contestar solo, por
                        // `onResults()` o por `onError()`.
                        AccionAlTocarVoz.IGNORAR -> Unit

                        AccionAlTocarVoz.EMPEZAR ->
                            motor.empezar { texto -> alReconocer(texto) }

                        // El permiso se pide **acá**, cuando toca el
                        // micrófono, y nunca al abrir la app: en el manifiesto
                        // va sin `required`, así que un teléfono sin micrófono
                        // igual instala y escribe.
                        AccionAlTocarVoz.PEDIR_PERMISO ->
                            lanzador.launch(Manifest.permission.RECORD_AUDIO)
                    }
                }

                override fun olvidarAviso() = motor.olvidarAviso()
            }
        }
    }

    /**
     * El estado del dictado y el reconocedor que lo produce.
     *
     * Vive en una clase y no en `remember`s sueltos para que el objeto que ve la
     * UI tenga identidad estable y cada lectura sea fresca: la pantalla se
     * redibuja porque [escuchando] y [aviso] son estado de Compose, no porque
     * alguien se acuerde de avisar.
     */
    private class MotorDeDictado(contexto: Context) {

        private val app = contexto.applicationContext

        var escuchando by mutableStateOf(false)
            private set

        /**
         * True entre que se corta el dictado y que el reconocedor contesta.
         *
         * Ver [SesionDeDictado.procesando]: es el estado que faltaba para que
         * un tercer toque en ese hueco no arranque una sesión nueva.
         */
        var procesando by mutableStateOf(false)
            private set

        var aviso by mutableStateOf<String?>(null)
            private set

        private var reconocedor: SpeechRecognizer? = null
        private var idioma = 0
        private var alTexto: ((String) -> Unit)? = null

        fun empezar(alTexto: (String) -> Unit) {
            // El guardia cubre los dos tramos en que el reconocedor ya está
            // ocupado: escuchando, o todavía procesando lo último dicho.
            if (escuchando || procesando) return
            this.alTexto = alTexto
            aviso = null
            arrancar()
        }

        /** Corta y espera el resultado de lo ya dicho. */
        fun cortar() {
            if (!escuchando) return
            escuchando = false
            procesando = true
            runCatching { reconocedor?.stopListening() }
                .onFailure { e ->
                    Log.w(TAG, "no se pudo cortar el dictado", e)
                    // Si `stopListening()` ni siquiera salió, nunca va a
                    // llegar `onResults()` ni `onError()` que baje
                    // `procesando`: quedarse ahí sería el mismo tipo de
                    // cuelgue que se está arreglando acá, sólo que
                    // permanente en vez de una ventana de milisegundos.
                    procesando = false
                    aviso = NO_DISPONIBLE
                }
        }

        fun sinPermiso() {
            escuchando = false
            aviso = SIN_PERMISO
        }

        fun olvidarAviso() {
            if (aviso != null) aviso = null
        }

        fun soltar() {
            alTexto = null
            escuchando = false
            procesando = false
            runCatching { reconocedor?.destroy() }
                .onFailure { Log.w(TAG, "no se pudo soltar el reconocedor", it) }
            reconocedor = null
        }

        private fun arrancar() {
            val motor = reconocedor ?: crear() ?: return
            escuchando = true
            runCatching { motor.startListening(intentDeReconocimiento(IDIOMAS[idioma])) }
                .onFailure { e ->
                    // Un motor de fabricante puede rechazar la petición. Es feo;
                    // que la app se caiga en la mano de la dueña con gente
                    // esperando sería peor.
                    Log.w(TAG, "no arrancó el dictado", e)
                    escuchando = false
                    aviso = NO_DISPONIBLE
                }
        }

        private fun crear(): SpeechRecognizer? {
            val motor = runCatching { SpeechRecognizer.createSpeechRecognizer(app) }
                .onFailure { Log.w(TAG, "no se pudo crear el reconocedor", it) }
                .getOrNull()
            if (motor == null) {
                aviso = NO_DISPONIBLE
                return null
            }
            motor.setRecognitionListener(escucha)
            reconocedor = motor
            return motor
        }

        private fun intentDeReconocimiento(idioma: String?): Intent =
            Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
                putExtra(
                    RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                    RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
                )
                // Una sola hipótesis: el texto va a un campo que la dueña lee
                // antes de que se guarde nada, y una lista de alternativas sería
                // una decisión más en el mostrador.
                putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
                // Los parciales no entran al campo: un texto que se corrige solo
                // mientras se lee es imposible de revisar.
                putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, false)
                // Varios motores de fabricante rechazan la petición sin esto.
                putExtra(RecognizerIntent.EXTRA_CALLING_PACKAGE, app.packageName)
                if (idioma != null) {
                    putExtra(RecognizerIntent.EXTRA_LANGUAGE, idioma)
                    putExtra(RecognizerIntent.EXTRA_LANGUAGE_PREFERENCE, idioma)
                }
                // A propósito **no** se pide `EXTRA_PREFER_OFFLINE`: en un
                // teléfono sin el pack de voz descargado, pedirlo hace fallar un
                // dictado que con datos habría funcionado. Quien decide es el
                // motor, que sabe qué tiene instalado.
            }

        private val escucha = object : RecognitionListener {

            override fun onResults(results: Bundle?) {
                escuchando = false
                procesando = false
                val texto = results
                    ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                    ?.firstOrNull()
                    ?.trim()
                    .orEmpty()

                if (texto.isEmpty()) {
                    aviso = NO_TE_ESCUCHE
                    return
                }
                alTexto?.invoke(texto)
            }

            override fun onError(error: Int) {
                escuchando = false
                procesando = false
                when (error) {
                    // Los dos códigos de idioma llegaron en Android 13. Son
                    // constantes: en un Android 6 el `when` simplemente nunca
                    // entra por acá.
                    SpeechRecognizer.ERROR_LANGUAGE_NOT_SUPPORTED,
                    SpeechRecognizer.ERROR_LANGUAGE_UNAVAILABLE,
                    -> bajarUnIdioma()

                    // Silencio útil: no entendió, o no se dijo nada. Se ofrece
                    // el otro camino en una línea, sin alarma.
                    SpeechRecognizer.ERROR_NO_MATCH,
                    SpeechRecognizer.ERROR_SPEECH_TIMEOUT,
                    -> aviso = NO_TE_ESCUCHE

                    SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> aviso = SIN_PERMISO

                    // Casi todos los motores necesitan datos si no tienen el
                    // pack de voz bajado. En feria eso pasa seguido y no es una
                    // falla de la app: es el día normal.
                    SpeechRecognizer.ERROR_NETWORK,
                    SpeechRecognizer.ERROR_NETWORK_TIMEOUT,
                    -> aviso = SIN_SENAL

                    // `cancel()`/`destroy()` en vuelo llegan como ERROR_CLIENT.
                    // Eso lo pidió la dueña -o la pantalla se fue-, así que no
                    // hay nada que contarle.
                    SpeechRecognizer.ERROR_CLIENT -> aviso = null

                    else -> aviso = NO_DISPONIBLE
                }
            }

            /** Se acabó el audio; el resultado llega igual por [onResults]. */
            override fun onEndOfSpeech() = Unit

            override fun onReadyForSpeech(params: Bundle?) = Unit
            override fun onBeginningOfSpeech() = Unit
            override fun onRmsChanged(rmsdB: Float) = Unit
            override fun onBufferReceived(buffer: ByteArray?) = Unit
            override fun onPartialResults(partialResults: Bundle?) = Unit
            override fun onEvent(eventType: Int, params: Bundle?) = Unit
        }

        private fun bajarUnIdioma() {
            if (idioma >= IDIOMAS.lastIndex) {
                aviso = NO_DISPONIBLE
                return
            }
            idioma += 1
            // Reintento inmediato y **una sola vez por escalón**: la lista tiene
            // tres, así que esto no puede girar para siempre.
            arrancar()
        }
    }

    // --- lo que se le dice a la dueña ---------------------------------------
    //
    // Todas terminan en cómo seguir. Ninguna dice "error".

    private const val NO_TE_ESCUCHE =
        "No te escuché. Prueba de nuevo o escríbelo."

    private const val SIN_PERMISO =
        "Para dictar tienes que dejarme usar el micrófono. Puedes escribir igual."

    private const val SIN_SENAL =
        "Sin internet no puedo escucharte. Escríbelo y sigue igual."

    private const val NO_DISPONIBLE =
        "El dictado no está disponible en este teléfono. Puedes escribir igual."
}

/**
 * ¿Hay algo en este aparato que sepa escuchar?
 *
 * Se pregunta una vez, en `MainActivity`, y de ahí sale si la pantalla del
 * agente dibuja o no el control de voz. Un botón que no puede funcionar es una
 * promesa rota; no dibujarlo deja el teclado, que es el camino que siempre
 * estuvo.
 */
fun Context.hayDictadoDeVoz(): Boolean =
    packageManager.hasSystemFeature(PackageManager.FEATURE_MICROPHONE) &&
        runCatching { SpeechRecognizer.isRecognitionAvailable(this) }.getOrDefault(false)

private fun Context.tienePermisoDeMicrofono(): Boolean =
    ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) ==
        PackageManager.PERMISSION_GRANTED

/** Lo que hay que hacer al tocar el botón de voz. */
internal enum class AccionAlTocarVoz {
    CORTAR,
    IGNORAR,
    EMPEZAR,
    PEDIR_PERMISO,
}

/**
 * La decisión de [SesionDeDictado.alternar], separada del `SpeechRecognizer`
 * y de Compose a propósito: ninguno de los dos corre en una prueba JVM pura,
 * y acá es donde vivía la carrera que encontró el asiento de la ola 23 -que
 * un tercer toque durante [procesando] arrancara una sesión nueva encima de
 * la que el reconocedor todavía no había terminado de contestar.
 */
internal fun accionAlTocarVoz(
    escuchando: Boolean,
    procesando: Boolean,
    tienePermiso: Boolean,
): AccionAlTocarVoz = when {
    // Segundo toque: corta, pase lo que pase con el resto -no puede haber
    // procesando en vuelo si todavía está escuchando.
    escuchando -> AccionAlTocarVoz.CORTAR

    // Tercer toque, en pleno procesando: el reconocedor sigue vivo con la
    // frase anterior. Arrancar acá es la carrera; no hacer nada es la
    // salida segura, porque `onResults()`/`onError()` van a llegar solos.
    procesando -> AccionAlTocarVoz.IGNORAR

    tienePermiso -> AccionAlTocarVoz.EMPEZAR

    else -> AccionAlTocarVoz.PEDIR_PERMISO
}
