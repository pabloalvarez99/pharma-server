package cl.rutbusiness.app.ui.entrada

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.compositionLocalOf
import cl.rutbusiness.core.session.IdentidadGoogle
import cl.rutbusiness.core.session.IdentidadGoogleNoCableada

/**
 * Si el teléfono está conectado a algo.
 *
 * No pregunta si hay **internet**: pregunta si hay red. La diferencia importa y
 * es el caso normal de este producto, no el borde. El computador del negocio
 * suele estar en el mismo wifi del local, un wifi que muchas veces no sale a
 * internet — y decirle a la dueña "no tienes internet" cuando su sistema está a
 * tres metros y andando la manda a llamar a la compañía de teléfono por nada.
 */
interface RedDelTelefono {
    /** `true` si hay wifi o datos prendidos, sin prometer que salgan a la calle. */
    fun hayRed(): Boolean
}

/**
 * Lo que este teléfono ya sabe de la primera vez.
 *
 * Banderas de onboarding. [marcarQueEntro] se prende con el primer login que
 * funciona, no cuando la dueña termina de leer: quien saltó la explicación y
 * entró bien tampoco tiene que volver a verla.
 *
 * [rubroElegido] y la tarjeta de rescate (ADR-0022) viven acá porque se eligen
 * **antes** de que haya pack de server, y se aplican al entrar.
 */
interface PreferenciasDeEntrada {
    fun yaEntroAlgunaVez(): Boolean
    fun marcarQueEntro()

    /** Vertical elegido en el alta (`feria`, `farmacia`, …) o null. */
    fun rubroElegido(): String?

    fun guardarRubroElegido(rubro: String)

    /**
     * `true` si aún no se mostró la tarjeta de rescate y el rubro es informal
     * (feria). Se apaga al confirmar "ya la anoté".
     */
    fun debeMostrarTarjetaRescate(): Boolean

    fun marcarTarjetaRescateVista()
}

/**
 * Lo que la plataforma le presta a la pantalla de entrada.
 *
 * Va por `CompositionLocal` y no por parámetro por lo mismo que
 * [cl.rutbusiness.app.ui.impresora.ServiciosDeImpresora]: son cosas del
 * teléfono que necesita **una** pantalla, y hacerlas viajar por la raíz de la
 * app obligaría a que todo el resto las conozca.
 *
 * [identidadGoogle] es stub por default (ADR-0022): sin client id no hay OAuth
 * real; la UI muestra copy feria y el camino correo/clave sigue siendo el
 * feliz path offline-first.
 */
class ServiciosDeEntrada(
    val red: RedDelTelefono,
    val preferencias: PreferenciasDeEntrada,
    val identidadGoogle: IdentidadGoogle = IdentidadGoogleNoCableada,
    /**
     * La dirección a la que apunta "Crear mi negocio", o `null` cuando este APK
     * no trae ninguna.
     *
     * Viene de `BuildConfig.URL_NUBE`, o sea de cómo se compiló el APK, y no de
     * una constante en el código (ver `app/build.gradle.kts`). Entra por acá y
     * no leyendo `BuildConfig` desde la pantalla por la misma razón que la red
     * y las preferencias: debajo de [cl.rutbusiness.app.ui.RutBusinessApp] no
     * hay nada de Android ni de este módulo de aplicación, y eso es lo que deja
     * la capa de UI compilable para iOS sin tocarla.
     *
     * No es un secreto y no puede serlo: es una dirección pública, la misma que
     * cualquiera ve mirando el tráfico de la app. Ninguna clave viaja en el APK
     * — ver el encabezado de `ui/alta/AltaApi.kt`.
     */
    val nube: String? = null,
)

/**
 * `null` cuando nadie lo proveyó — una prueba de otra pantalla, por ejemplo.
 *
 * Sin esto la pantalla de entrada sigue funcionando: se salta la explicación
 * del primer uso y no puede descartar el caso de "sin red" antes de intentar,
 * que es exactamente lo que hacía la versión anterior. Degradarse así es
 * preferible a que la app no arranque en un contexto de prueba.
 */
val LocalEntrada: ProvidableCompositionLocal<ServiciosDeEntrada?> =
    compositionLocalOf { null }

@Composable
fun ProveerEntrada(
    servicios: ServiciosDeEntrada,
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(LocalEntrada provides servicios, content = content)
}
