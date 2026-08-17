package cl.rutbusiness.app.ui.nav

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.catalogo.abrirCatalogo
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.pos.Carrito
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Que cambiar de pestaña no borre lo que se estaba haciendo.
 *
 * Es la prueba que justifica no haber metido `navigation-compose`: un `NavHost`
 * mal configurado limpia el `ViewModelStore` de la entrada al salir de ella, y
 * el síntoma sería un carrito a medias que desaparece porque la dueña fue a
 * preguntarle algo al agente. En un punto de venta ése es el peor bug posible —
 * la cajera vuelve, ve el carrito vacío y tiene que re-escanear todo con el
 * cliente esperando.
 *
 * El carrito de acá es el [Carrito] de verdad, dentro de un `ViewModel` de
 * verdad pedido con `viewModel()`, dentro del [ContenedorDeDestinos] de verdad.
 * Lo único simulado son las pantallas, porque las de producción necesitan un
 * server contestando y eso no es lo que está bajo prueba.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class NavegacionTest {

    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    /** Un producto con la forma exacta que devuelve el server. */
    private fun producto(id: String, nombre: String) = ProductDto(
        active = true,
        createdAt = "2026-08-06T00:00:00Z",
        id = id,
        name = nombre,
        physicalStock = true,
        prescriptionType = "none",
        price = "1490",
        slug = nombre.lowercase(),
        stock = 10L,
        updatedAt = "2026-08-06T00:00:00Z",
    )

    /** El carrito vive donde vive en producción: en un `ViewModel`. */
    private class CarritoFalsoViewModel : ViewModel() {
        var carrito by mutableStateOf(Carrito())
            private set

        fun agregar(p: ProductDto) {
            carrito = carrito.agregar(p)
        }
    }

    /**
     * @param sesion sólo la necesitan las pruebas del catálogo: sin sesión
     *   `LocalAbrirCatalogo` queda en `null` (ver `AbrirCatalogo.kt`) y el
     *   botón de [CobrarDeMentira] no tiene qué invocar.
     */
    private fun montar(sesion: SessionRepository? = null) {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                ContenedorDeDestinos(sesion = sesion) { destino ->
                    when (destino) {
                        Destino.Agente -> PantallaDeMentira("Estoy en el agente")
                        Destino.Cobrar -> CobrarDeMentira()
                        Destino.Resumen -> PantallaDeMentira("Estoy en el resumen")
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    /** Sesión sin login: alcanza para que `abrirCatalogo()` deje de ser `null`. */
    private fun sesionDeMentira(): SessionRepository {
        val app = RuntimeEnvironment.getApplication()
        return SessionRepository(AlmacenamientoPlataforma(app))
    }

    @Composable
    private fun PantallaDeMentira(texto: String) {
        Text(text = texto)
    }

    /**
     * Imita lo que importa de Cobrar: un carrito en el `ViewModel` y un dato de
     * pantalla en `rememberSaveable`. Las dos cosas se pierden por motivos
     * distintos si el contenedor está mal, así que las dos se miran.
     */
    @Composable
    private fun CobrarDeMentira() {
        val vm: CarritoFalsoViewModel = viewModel(
            key = "cobrar-de-mentira",
            factory = viewModelFactory { initializer { CarritoFalsoViewModel() } },
        )
        var notaDelMostrador by rememberSaveable { mutableStateOf("") }
        // Lo mismo que hace Cobrar de verdad para abrir "Lo que vendo": leer
        // el callback del `CompositionLocal` y listo. Con `sesion == null` en
        // `montar()` esto es `null` y el botón no hace nada -el mismo gateo
        // que la pantalla real-, así que las pruebas que no piden catálogo no
        // se ven afectadas.
        val abrirCatalogo = abrirCatalogo()

        Column {
            Text("En el carrito: ${vm.carrito.items.size}")
            Text("Nota: «$notaDelMostrador»")
            RbButton(
                label = "Agregar pan",
                onClick = { vm.agregar(producto("product:pan", "Pan")) },
            )
            RbButton(
                label = "Agregar leche",
                onClick = { vm.agregar(producto("product:leche", "Leche")) },
            )
            RbButton(
                label = "Anotar",
                onClick = { notaDelMostrador = "paga con 20 lucas" },
            )
            RbButton(
                label = "Abrir catálogo",
                onClick = { abrirCatalogo?.invoke() },
            )
        }
    }

    private fun irA(destino: Destino) {
        compose.onNodeWithText(destino.etiqueta).performClick()
        compose.waitForIdle()
    }

    @Test
    fun `arranca en el agente`() {
        montar()
        compose.onNodeWithText("Estoy en el agente").assertIsDisplayed()
    }

    @Test
    fun `las tres pestanas llevan a su pantalla`() {
        montar()

        irA(Destino.Cobrar)
        compose.onNodeWithText("En el carrito: 0").assertIsDisplayed()

        irA(Destino.Resumen)
        compose.onNodeWithText("Estoy en el resumen").assertIsDisplayed()

        irA(Destino.Agente)
        compose.onNodeWithText("Estoy en el agente").assertIsDisplayed()
    }

    /** El caso que da nombre a todo esto. */
    @Test
    fun `un carrito a medias sobrevive ir al agente y volver`() {
        montar()

        irA(Destino.Cobrar)
        compose.onNodeWithText("Agregar pan").performClick()
        compose.onNodeWithText("Agregar leche").performClick()
        compose.onNodeWithText("Anotar").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("En el carrito: 2").assertIsDisplayed()

        irA(Destino.Agente)
        compose.onNodeWithText("Estoy en el agente").assertIsDisplayed()

        irA(Destino.Cobrar)
        compose.onNodeWithText("En el carrito: 2").assertIsDisplayed()
        compose.onNodeWithText("Nota: «paga con 20 lucas»").assertIsDisplayed()
    }

    /** Y también sobrevive pasar por la tercera pestaña, no sólo por el agente. */
    @Test
    fun `el carrito sobrevive dar la vuelta completa`() {
        montar()

        irA(Destino.Cobrar)
        compose.onNodeWithText("Agregar pan").performClick()
        compose.waitForIdle()

        irA(Destino.Resumen)
        irA(Destino.Agente)
        irA(Destino.Cobrar)

        compose.onNodeWithText("En el carrito: 1").assertIsDisplayed()
    }

    /**
     * Atrás desde una pestaña secundaria vuelve al agente, no cierra la app.
     *
     * Y desde el agente **no** se atrapa: ahí atrás significa salir, y quedarse
     * con él dejaría a la dueña sin forma de cerrar la app.
     */
    @Test
    fun `atras vuelve al agente y desde el agente no se atrapa`() {
        montar()
        val despachador = compose.activity.onBackPressedDispatcher

        irA(Destino.Resumen)
        compose.onNodeWithText("Estoy en el resumen").assertIsDisplayed()

        compose.runOnUiThread { despachador.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithText("Estoy en el agente").assertIsDisplayed()

        assertEquals(
            "desde el agente, atrás tiene que quedar en manos del sistema",
            false,
            despachador.hasEnabledCallbacks(),
        )
    }

    /** Atrás tampoco puede vaciar el carrito: sólo cambia de pestaña. */
    @Test
    fun `volver con atras no borra el carrito`() {
        montar()

        irA(Destino.Cobrar)
        compose.onNodeWithText("Agregar pan").performClick()
        compose.waitForIdle()

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNodeWithText("Estoy en el agente").assertIsDisplayed()

        irA(Destino.Cobrar)
        compose.onNodeWithText("En el carrito: 1").assertIsDisplayed()
    }

    /**
     * El bug real: con el catálogo abierto encima de Cobrar, tocar otra
     * pestaña resaltaba la pestaña nueva pero dejaba el catálogo en pantalla
     * -recién Atrás mostraba lo que la barra ya decía-. La causa no era que
     * `viendoCatalogo` sobreviviera al cambio de pestaña -eso es
     * correcto, es lo mismo que hace `viendoCola`- sino que
     * `BarraDeNavegacion.onElegir` sólo cambiaba `destino` y nunca apagaba la
     * bandera, así que el `if` en cascada de `ContenedorDeDestinos` seguía
     * entrando a la rama del catálogo. Ver el `onElegir` de
     * `ContenedorDeDestinos`.
     */
    @Test
    fun `tocar una pestana cierra el catalogo y muestra el destino nuevo de una`() {
        montar(sesionDeMentira())

        irA(Destino.Cobrar)
        compose.onNodeWithText("Agregar pan").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Abrir catálogo").performClick()
        compose.waitForIdle()

        // El catálogo tapa a Cobrar: su topbar ("Inventario", pack por
        // defecto de la prueba) es lo único que se ve.
        compose.onNodeWithText("Inventario").assertIsDisplayed()
        compose.onNodeWithText("En el carrito: 1").assertDoesNotExist()

        irA(Destino.Resumen)

        compose.onNodeWithText("Inventario").assertDoesNotExist()
        compose.onNodeWithText("Estoy en el resumen").assertIsDisplayed()
    }

    /**
     * Decisión de producto para el caso completo del encargo: el catálogo se
     * cierra, no se pausa. Se abre **desde** Vender para cargar algo que
     * falta -"esto no lo tengo cargado", ver `LocalAbrirCatalogo`- y se
     * vuelve a cobrar; no es un lugar donde uno se queda. Tocar otra pestaña
     * es la dueña yéndose a hacer otra cosa a propósito, así que si vuelve a
     * Cobrar más tarde por la barra encuentra Cobrar, no el catálogo de
     * vuelta -que sería la misma mentira de estado que este arreglo saca,
     * sólo que retrasada-. El carrito, en cambio, sí sobrevive: es el estado
     * de la pestaña, no el del catálogo.
     */
    @Test
    fun `el catalogo no reaparece solo al volver a la pestana`() {
        montar(sesionDeMentira())

        irA(Destino.Cobrar)
        compose.onNodeWithText("Agregar pan").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Abrir catálogo").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Inventario").assertIsDisplayed()

        irA(Destino.Resumen)
        irA(Destino.Cobrar)

        compose.onNodeWithText("Inventario").assertDoesNotExist()
        compose.onNodeWithText("En el carrito: 1").assertIsDisplayed()
    }
}
