package cl.rutbusiness.app.camara

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import android.util.Log
import android.util.Size
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.AspectRatioStrategy
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import cl.rutbusiness.app.ui.scanner.CamaraDeCodigos
import cl.rutbusiness.app.ui.scanner.EstadoDelPermiso
import cl.rutbusiness.app.ui.scanner.FormatoDeCodigo
import cl.rutbusiness.app.ui.scanner.PermisoDeCamara
import cl.rutbusiness.ui.theme.RbTheme
import com.google.mlkit.vision.barcode.BarcodeScanning
import java.util.concurrent.Executors

/**
 * La cámara de verdad: CameraX para el video, ML Kit para decodificar.
 *
 * Este archivo y [AnalizadorDeCodigos] son **los únicos** de la app que saben
 * que existe una cámara. `ui/scanner` habla con la interfaz
 * [CamaraDeCodigos] y no importa nada de `android.*`; el enchufe se hace en
 * `MainActivity`, que es la frontera de plataforma.
 */
object CamaraDeCodigosCameraX : CamaraDeCodigos {

    private const val TAG = "RbCamara"

    /**
     * Resolución del análisis.
     *
     * 1280x720 no es un número de compromiso, es un piso medido: un EAN-13 a
     * treinta centímetros ocupa cerca de un tercio del ancho del cuadro, y a
     * 640x480 esas trece barras caen bajo los ~2 px por barra que el detector
     * necesita — el código se ve perfecto en pantalla y no lee nunca. Hacia
     * arriba no hay nada que ganar: pedir 4K para leer trece dígitos multiplica
     * por nueve los bytes que cruzan el bus de la cámara y el trabajo por frame,
     * en un aparato que ya va justo.
     */
    private val RESOLUCION = Size(1280, 720)

    /**
     * Un solo selector para la vista previa y para el análisis, a propósito.
     *
     * Si cada uno eligiera su tamaño por su cuenta podrían quedar con distinta
     * relación de aspecto, y entonces el recorte que ve la cajera no sería el
     * que lee el detector: apuntaría a un código que está en pantalla y el
     * escáner no lo vería. Mismo selector, mismo encuadre.
     */
    private val SELECTOR_DE_RESOLUCION = ResolutionSelector.Builder()
        .setAspectRatioStrategy(AspectRatioStrategy.RATIO_16_9_FALLBACK_AUTO_STRATEGY)
        .setResolutionStrategy(
            ResolutionStrategy(
                RESOLUCION,
                ResolutionStrategy.FALLBACK_RULE_CLOSEST_LOWER_THEN_HIGHER,
            ),
        )
        .build()

    // --- permiso ------------------------------------------------------------

    @Composable
    override fun recordarPermiso(): PermisoDeCamara {
        val contexto = LocalContext.current
        val actividad = remember(contexto) { contexto.actividad() }
        val duenio = LocalLifecycleOwner.current

        var estado by rememberSaveable {
            mutableStateOf(
                if (contexto.tienePermisoDeCamara()) {
                    EstadoDelPermiso.Concedido
                } else {
                    EstadoDelPermiso.SinPedir
                },
            )
        }

        val lanzador = rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { concedido ->
            estado = when {
                concedido -> EstadoDelPermiso.Concedido
                // Android deja de mostrar el diálogo después del segundo "no", y
                // la única señal de que eso pasó es que el sistema ya no pide
                // que expliquemos por qué. Distinguirlo importa: con `Negado`
                // volver a tocar "Permitir" sirve, con `NegadoParaSiempre` ese
                // botón no haría absolutamente nada y hay que mandar a Ajustes.
                actividad != null && actividad.deberiaExplicarCamara() -> EstadoDelPermiso.Negado
                else -> EstadoDelPermiso.NegadoParaSiempre
            }
        }

        // El permiso se puede haber concedido en Ajustes mientras la app estaba
        // atrás. Sin esto, la cajera vuelve del sistema con el permiso dado y la
        // pantalla le sigue diciendo que no lo tiene.
        DisposableEffect(duenio, contexto) {
            val observador = LifecycleEventObserver { _, evento ->
                if (evento == Lifecycle.Event.ON_RESUME && contexto.tienePermisoDeCamara()) {
                    estado = EstadoDelPermiso.Concedido
                }
            }
            duenio.lifecycle.addObserver(observador)
            onDispose { duenio.lifecycle.removeObserver(observador) }
        }

        return remember(estado, lanzador, contexto) {
            object : PermisoDeCamara {
                override val estado = estado

                override fun pedir() {
                    lanzador.launch(Manifest.permission.CAMERA)
                }

                override fun abrirAjustes() {
                    val intent = Intent(
                        Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                        Uri.fromParts("package", contexto.packageName, null),
                    ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    // Un fabricante puede no tener esa pantalla. Que no abra es
                    // feo; que la app se caiga desde una pantalla de error sería
                    // peor.
                    runCatching { contexto.startActivity(intent) }
                        .onFailure { Log.w(TAG, "no se pudo abrir Ajustes", it) }
                }
            }
        }
    }

    // --- visor --------------------------------------------------------------

    @Composable
    override fun Visor(
        modifier: Modifier,
        formatos: Set<FormatoDeCodigo>,
        onCodigo: (String) -> Unit,
    ) {
        val contexto = LocalContext.current
        val duenio = LocalLifecycleOwner.current
        // La lambda cambia en cada recomposición del padre; el analizador se
        // queda con ésta y siempre llama a la última, sin rearmar la cámara.
        val alLeer by rememberUpdatedState(onCodigo)

        var falla by remember { mutableStateOf<String?>(null) }
        val vista = remember(contexto) {
            PreviewView(contexto).apply {
                // FIT_CENTER y no FILL: con recorte, la cajera apunta a lo que
                // ve y el detector lee otra cosa. Prefiere barras negras a un
                // encuadre que miente.
                scaleType = PreviewView.ScaleType.FIT_CENTER
            }
        }

        DisposableEffect(duenio, formatos, vista) {
            val hilo = Executors.newSingleThreadExecutor()
            val escaner = BarcodeScanning.getClient(opcionesDe(formatos))
            val futuro = ProcessCameraProvider.getInstance(contexto)

            var proveedor: ProcessCameraProvider? = null
            var analisis: ImageAnalysis? = null
            var vivo = true

            futuro.addListener({
                // Salir de la pantalla mientras la cámara todavía se abría deja
                // este callback en vuelo. Sin la bandera, encendería una cámara
                // que ya nadie va a apagar.
                if (!vivo) return@addListener

                val disponible = runCatching { futuro.get() }.getOrElse { e ->
                    Log.e(TAG, "no se pudo abrir la cámara", e)
                    falla = "No pudimos abrir la cámara de este teléfono."
                    return@addListener
                }
                proveedor = disponible

                val previa = Preview.Builder()
                    .setResolutionSelector(SELECTOR_DE_RESOLUCION)
                    .build()
                    .also { it.surfaceProvider = vista.surfaceProvider }

                val lector = ImageAnalysis.Builder()
                    .setResolutionSelector(SELECTOR_DE_RESOLUCION)
                    // Descartar los frames atrasados en vez de encolarlos. En un
                    // aparato lento el detector no alcanza a mirar todos, y una
                    // cola sólo agrega retardo entre lo que la cajera apunta y
                    // lo que el escáner está analizando.
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                    .also {
                        // Un hilo propio y uno solo: el análisis no puede correr
                        // en el hilo principal (congelaría la UI) y un pool por
                        // frame sería recolección de basura constante.
                        it.setAnalyzer(hilo, AnalizadorDeCodigos(escaner) { codigo -> alLeer(codigo) })
                    }
                analisis = lector

                runCatching {
                    disponible.unbindAll()
                    disponible.bindToLifecycle(
                        duenio,
                        CameraSelector.DEFAULT_BACK_CAMERA,
                        previa,
                        lector,
                    )
                }.onFailure { e ->
                    Log.e(TAG, "no se pudo enlazar la cámara", e)
                    falla = "Este teléfono no nos dejó usar la cámara de atrás."
                }
            }, ContextCompat.getMainExecutor(contexto))

            onDispose {
                // Salir de la pantalla suelta TODO. Una sesión de cámara viva es
                // un buffer de video por frame y el sensor encendido: en un
                // aparato de 1-2 GB eso se nota en la memoria y en la batería
                // del turno completo.
                vivo = false
                analisis?.clearAnalyzer()
                proveedor?.unbindAll()
                escaner.close()
                hilo.shutdown()
            }
        }

        Box(modifier = modifier.background(Color.Black), contentAlignment = Alignment.Center) {
            AndroidView(factory = { vista }, modifier = Modifier.fillMaxSize())
            falla?.let { mensaje ->
                Text(
                    text = "$mensaje Puedes escribir el código a mano.",
                    style = RbTheme.typography.body,
                    color = RbTheme.colors.textPrimary,
                    textAlign = TextAlign.Center,
                    modifier = Modifier
                        .background(RbTheme.colors.surface)
                        .padding(RbTheme.dimens.space3),
                )
            }
        }
    }
}

private fun Context.tienePermisoDeCamara(): Boolean =
    ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) ==
        PackageManager.PERMISSION_GRANTED

private fun Activity.deberiaExplicarCamara(): Boolean =
    androidx.core.app.ActivityCompat.shouldShowRequestPermissionRationale(
        this,
        Manifest.permission.CAMERA,
    )

/** El `Activity` detrás de un `Context` de Compose, que suele venir envuelto. */
private tailrec fun Context.actividad(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.actividad()
    else -> null
}
