package cl.rutbusiness.app.ui.alta

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

/**
 * Las cuatro preguntas del alta, una por composable y una por pantalla.
 *
 * **Cartel, no wizard.** Cada paso es una sola pregunta con aire: la dueña no
 * ve un formulario de cinco campos ni una grilla de “pasos”. El piso de
 * hardware es una persona mayor con un teléfono de 720x1280; al 200% de escala
 * de letra, el formulario completo mide más de tres pantallas. Preguntando de
 * a una, cada pantalla entra sin scrollear incluso al 200%, y el botón de
 * avanzar está siempre en el mismo lugar (panel anclado en [PantallaDeAlta]).
 *
 * Ninguna de estas funciones sabe nada del server ni del ViewModel: reciben el
 * valor y devuelven el cambio. Es lo que las hace medibles al 100% y al 200%
 * sin levantar nada.
 */

/**
 * Dónde se va a guardar.
 *
 * **Sólo aparece cuando el APK no trae una dirección de nube compilada.** Ver
 * [AltaViewModel.pasos]: el camino de la nube no pregunta esto nunca, que es el
 * punto entero de que exista.
 */
@Composable
internal fun PasoDonde(
    url: String,
    onUrl: (String) -> Unit,
    ayuda: String,
    error: String?,
    habilitado: Boolean,
) {
    val copy = copyPasoDonde()
    CartelDelPaso(titulo = copy.titulo) {
        Text(
            text = copy.cuerpo,
            style = RbTheme.typography.body,
            color = RbTheme.colors.textPrimary,
        )
        RbTextField(
            value = url,
            onValueChange = onUrl,
            label = copy.labelDireccion,
            placeholder = copy.placeholder,
            supportingText = ayuda,
            errorMessage = error,
            keyboardType = KeyboardType.Uri,
            enabled = habilitado,
        )
        Text(
            text = copy.ayudaSinDireccion,
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )
    }
}

/** Cómo se llama el negocio / puesto. */
@Composable
internal fun PasoNegocio(
    nombre: String,
    onNombre: (String) -> Unit,
    habilitado: Boolean,
    onListo: () -> Unit,
    esFeria: Boolean = false,
) {
    val copy = copyPasoNegocio(esFeria)
    CartelDelPaso(titulo = copy.titulo) {
        RbTextField(
            value = nombre,
            onValueChange = onNombre,
            label = copy.label,
            placeholder = copy.placeholder,
            supportingText = copy.ayuda,
            enabled = habilitado,
            imeAction = ImeAction.Next,
            onImeAction = onListo,
        )
    }
}

/**
 * A qué se dedica.
 *
 * Los nueve [RUBROS] se pintan como carteles apilados ([CartelesDeRubro]), no
 * como filas de lista ni grilla de dos columnas: al 200% una grilla parte las
 * etiquetas largas, y un combo se siente a menú de sistema. Un cartel entero
 * por opción es un objetivo táctil de 56dp que no hay que apuntar.
 *
 * Ninguna viene elegida. Feria va primero y se anuncia; un rubro elegido de
 * verdad prende y apaga módulos de la app, uno puesto por descarte no.
 */
@Composable
internal fun PasoRubro(
    elegido: Rubro?,
    onElegir: (Rubro) -> Unit,
    habilitado: Boolean,
) {
    val dimens = RbTheme.dimens

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Text(
            text = AYUDA_DE_RUBROS,
            style = RbTheme.typography.body,
            color = RbTheme.colors.textPrimary,
        )
        CartelesDeRubro(
            elegido = elegido,
            onElegir = onElegir,
            habilitado = habilitado,
        )
    }
}

/** El correo y la clave de la dueña. */
@Composable
internal fun PasoCuenta(
    email: String,
    onEmail: (String) -> Unit,
    errorDeCorreo: String?,
    clave: String,
    onClave: (String) -> Unit,
    errorDeClave: String?,
    habilitado: Boolean,
    onListo: () -> Unit,
    esFeria: Boolean = false,
) {
    val copy = copyPasoCuenta(esFeria)
    CartelDelPaso(titulo = copy.titulo) {
        RbTextField(
            value = email,
            onValueChange = onEmail,
            label = copy.labelCorreo,
            placeholder = copy.placeholderCorreo,
            supportingText = copy.ayudaCorreo,
            errorMessage = errorDeCorreo,
            keyboardType = KeyboardType.Email,
            enabled = habilitado,
            imeAction = ImeAction.Next,
        )
        RbTextField(
            value = clave,
            onValueChange = onClave,
            label = copy.labelClave,
            supportingText = copy.ayudaClave,
            errorMessage = errorDeClave,
            keyboardType = KeyboardType.Password,
            visualTransformation = PasswordVisualTransformation(),
            enabled = habilitado,
            imeAction = ImeAction.Go,
            onImeAction = onListo,
        )
    }
}

/**
 * Marco del cartel de un paso: superficie elevada, borde fuerte, padding
 * generoso. Misma vara que el camino primario de la puerta — se lee de lejos,
 * no como un recuadro de formulario.
 */
@Composable
private fun CartelDelPaso(
    titulo: String,
    content: @Composable () -> Unit,
) {
    val colors = RbTheme.colors
    val dimens = RbTheme.dimens
    val shape = RbTheme.shapes.card

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(colors.surfaceRaised)
            .border(dimens.focusRing, colors.outlineStrong, shape)
            .padding(horizontal = dimens.space3, vertical = dimens.space4),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Text(
            text = titulo,
            style = RbTheme.typography.heading,
            color = colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        content()
    }
}
