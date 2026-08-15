package cl.rutbusiness.app.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import cl.rutbusiness.app.BuildConfig
import cl.rutbusiness.app.R
import cl.rutbusiness.app.ui.entrada.CompartirTarjeta
import cl.rutbusiness.app.ui.entrada.LocalCompartirTarjeta
import cl.rutbusiness.app.ui.entrada.TarjetaRescate
import cl.rutbusiness.app.ui.entrada.pasosDelPrimerUso
import cl.rutbusiness.app.ui.impresora.FallaDeImpresion
import cl.rutbusiness.core.backup.claveDeDemostracion
import cl.rutbusiness.core.backup.htmlTarjetaImprimible
import cl.rutbusiness.core.backup.textoTarjetaImprimible
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * Cómo se llama el producto **para la dueña**: RutAgent.
 *
 * "RutBusiness" sigue siendo el nombre interno y no se va a ninguna parte: es el
 * package `cl.rutbusiness`, el `applicationId`, el estilo `Theme.RutBusiness` y
 * un montón de identificadores en el código. Nada de eso lo lee nadie desde un
 * teléfono. Lo que sí se lee —el ícono del lanzador, el cartel de arranque, la
 * presentación del primer uso, la hoja de rescate que se pega en el cuaderno—
 * dice RutAgent, y este archivo es lo que lo sostiene.
 *
 * Por qué una prueba y no confianza: un nombre de producto no vive en un solo
 * lugar. Se filtró en seis archivos de tres módulos, y ya pasó una vez que una
 * marca vieja ("Tu Farmacia") sobreviviera escondida en las boletas mucho
 * después de que el producto se llamara otra cosa. El que renombra encuentra
 * los tres lugares obvios; los que quedan aparecen en el mostrador.
 *
 * La forma de las afirmaciones es a propósito: **todo se compara contra
 * `app_name`**, nunca contra el literal "RutAgent" suelto. `app_name` es el
 * nombre que el teléfono le pone al ícono, así que es el único que la dueña no
 * puede dejar de ver. Si mañana el producto se llama distinto, se cambia ese
 * recurso y esta prueba dice exactamente qué otros textos quedaron atrás, en vez
 * de romperse siete veces con el mismo mensaje.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class NombreDelProductoTest {

    @get:Rule
    val compose = createComposeRule()

    private val app = RuntimeEnvironment.getApplication()

    /** El nombre visible, leído de donde lo lee Android. */
    private val nombreVisible: String = app.getString(R.string.app_name)

    // --- el ícono -------------------------------------------------------------

    /**
     * El `android:label` de la aplicación y el de la activity salen los dos de
     * `app_name`, así que alcanza con fijar el recurso… salvo que alguien
     * escriba un literal en el manifiesto y desacople una cosa de la otra. Por
     * eso se pregunta por la etiqueta **resuelta**, que es la que termina abajo
     * del ícono: es la respuesta a "¿qué app abrí?".
     */
    @Test
    fun `el lanzador dice RutAgent`() {
        assertEquals("RutAgent", nombreVisible)

        val pm = app.packageManager
        val etiqueta = pm.getApplicationLabel(app.applicationInfo).toString()
        assertEquals(
            "la etiqueta que muestra el lanzador no es la de app_name",
            nombreVisible,
            etiqueta,
        )
    }

    /**
     * El otro lado del trato: cambia el nombre, **no** la identidad.
     *
     * El `applicationId` es lo que Play y el teléfono usan para saber que un APK
     * nuevo es una actualización del que ya está instalado. Moverlo no renombra
     * la app: instala una segunda al lado, con la venta y el fiado de la dueña
     * atrapados en la primera, que nadie va a volver a abrir. Es el error caro
     * que este trabajo tenía que no cometer, así que queda escrito.
     */
    @Test
    fun `el id de aplicacion no cambio`() {
        assertEquals("cl.rutbusiness.app", BuildConfig.APPLICATION_ID)
        assertEquals(
            "el package instalado dejó de coincidir con el applicationId",
            "cl.rutbusiness.app",
            app.packageName,
        )
    }

    // --- lo primero que se ve -------------------------------------------------

    /**
     * El DoD, montado: sesión en [cl.rutbusiness.core.session.EstadoSesion.Cargando]
     * —que es como arranca [SessionRepository] antes de restaurar nada— y se lee
     * el cartel de verdad, el que sale de [RutBusinessApp].
     *
     * Construir el repositorio sólo abre `SharedPreferences`: no hay red acá.
     */
    @Test
    fun `al abrir la app se lee RutAgent`() {
        val sesion = SessionRepository(AlmacenamientoPlataforma(app))

        compose.setContent {
            Box(Modifier.fillMaxSize()) { RutBusinessApp(sesion = sesion) }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Abriendo $nombreVisible...").assertIsDisplayed()
    }

    /** La pantalla que presenta el producto es la que más obvio lo tiene que decir. */
    @Test
    fun `el primer uso presenta RutAgent`() {
        val primerPaso = pasosDelPrimerUso(googleDisponible = false).first()
        assertEquals("Esto es $nombreVisible", primerPaso.titulo)
    }

    // --- lo que sale de la app en papel o en una nota -------------------------

    /**
     * La hoja de rescate se imprime, se pega en el cuaderno y se mira meses
     * después, cuando el teléfono ya se rompió. Es el texto de la app con la
     * vida más larga, y el peor lugar para dejar un nombre viejo: quien la lea
     * tiene que reconocer en el papel la app que tiene que volver a instalar.
     *
     * El payload `rutbusiness-rescue:v1:` que va en el QR **no** se toca y no se
     * mira acá: es formato de cable, no texto. Renombrarlo dejaría afuera todas
     * las tarjetas ya impresas, que es justo a quien la tarjeta sirve.
     */
    @Test
    fun `la tarjeta de rescate se imprime como RutAgent`() {
        val clave = claveDeDemostracion()
        val texto = textoTarjetaImprimible(clave, tenantSlug = "puesto-rosa")
        val html = htmlTarjetaImprimible(clave, tenantSlug = "puesto-rosa")

        assertTrue(
            "la hoja para el cuaderno no se presenta como $nombreVisible",
            texto.startsWith("$nombreVisible - tarjeta de rescate"),
        )
        assertTrue(
            "el título de la hoja impresa no dice $nombreVisible",
            html.contains("<h1>$nombreVisible - tarjeta de rescate</h1>"),
        )
        assertTrue(
            "la advertencia de la hoja nombra otro producto",
            html.contains("$nombreVisible no puede recuperarla"),
        )
        assertTrue(
            "el pie de la hoja impresa nombra otro producto",
            html.contains("$nombreVisible · tarjeta de rescate"),
        )
    }

    /** Y el mismo nombre en el asunto de lo que la dueña se manda a una nota. */
    @Test
    fun `compartir la tarjeta manda el asunto con RutAgent`() {
        val asuntos = mutableListOf<String>()
        val compartir = object : CompartirTarjeta {
            override fun compartirTexto(asunto: String, texto: String) { asuntos += asunto }
            override fun imprimirHtml(html: String) = Unit
        }

        compose.setContent {
            CompositionLocalProvider(LocalCompartirTarjeta provides compartir) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    Box(Modifier.fillMaxSize()) {
                        TarjetaRescate(onListo = {}, tenantSlug = "puesto-rosa")
                    }
                }
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Compartir a una nota", substring = true)
            .performScrollTo()
            .performClick()
        compose.waitForIdle()

        assertEquals(listOf("$nombreVisible - tarjeta de rescate"), asuntos)
    }

    // --- el nombre como instrucción -------------------------------------------

    /**
     * Este es el que de verdad se rompe si el nombre queda a medias.
     *
     * Cuando Android ya negó el Bluetooth para siempre, la app manda a la dueña
     * a Ajustes › Aplicaciones a buscar la app en una lista — y el teléfono la
     * lista por su `app_name`. El texto no está *mencionando* la marca, está
     * diciendo qué renglón tocar. Con un nombre que ya no existe, la instrucción
     * es correcta en todo menos en lo único que importa, y la boleta no sale.
     */
    @Test
    fun `el camino de Ajustes nombra la app como la lista el telefono`() {
        val queHacer = FallaDeImpresion.FaltaPermiso().queHacer
        assertTrue(
            "el camino de Ajustes no nombra la app como aparece en el teléfono",
            queHacer.contains("Ajustes › Aplicaciones › $nombreVisible › Permisos"),
        )
    }

    // --- la red de arrastre ---------------------------------------------------

    /**
     * Barrido sobre el copy que esta rama tocó: ningún texto que la dueña pueda
     * leer nombra al producto viejo, ni a la marca histórica de la que ya se
     * escapó una vez.
     *
     * Va sobre los textos como **dato** —no sobre pantallas montadas— para que
     * agregar un párrafo nuevo quede cubierto sin que nadie se acuerde de venir
     * a extender una prueba de UI.
     */
    @Test
    fun `ningun texto visible nombra al producto viejo`() {
        val clave = claveDeDemostracion()
        val copy: List<String> = buildList {
            add(nombreVisible)
            pasosDelPrimerUso(googleDisponible = true).forEach { paso ->
                add(paso.titulo)
                add(paso.encabezado)
                addAll(paso.parrafos)
                addAll(paso.lista)
                paso.remate?.let { add(it) }
            }
            pasosDelPrimerUso(googleDisponible = false).forEach { paso ->
                addAll(paso.lista)
            }
            add(FallaDeImpresion.FaltaPermiso().queHacer)
            add(textoTarjetaImprimible(clave, tenantSlug = "puesto-rosa"))
            add(htmlTarjetaImprimible(clave, tenantSlug = "puesto-rosa"))
        }

        // "rutbusiness-rescue:v1:" es el payload del QR y tiene permiso de
        // quedarse: no es texto que alguien lea como nombre del producto, es el
        // formato que las tarjetas ya impresas necesitan que siga entendiéndose.
        val marcasViejas = listOf("RutBusiness", "Tu Farmacia", "TuFarmacia")
        copy.forEach { texto ->
            val limpio = texto.replace("rutbusiness-rescue:v1:", "")
            marcasViejas.forEach { vieja ->
                assertTrue(
                    "quedó \"$vieja\" en un texto que lee la dueña: ${limpio.take(120)}",
                    !limpio.contains(vieja, ignoreCase = true),
                )
            }
        }
    }
}
