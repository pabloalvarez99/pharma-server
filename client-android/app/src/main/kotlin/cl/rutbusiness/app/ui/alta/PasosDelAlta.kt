package cl.rutbusiness.app.ui.alta

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import cl.rutbusiness.ui.components.RbCard
import cl.rutbusiness.ui.components.RbChip
import cl.rutbusiness.ui.components.RbChipTone
import cl.rutbusiness.ui.components.RbListRow
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme

/**
 * Las cuatro preguntas del alta, una por archivo de composable y una por
 * pantalla.
 *
 * **Por qué una pregunta por pantalla y no un formulario largo.** El piso de
 * hardware es una persona mayor con un teléfono de 720x1280. Al 200% de escala
 * de letra, el formulario completo —nombre, ocho rubros, correo, clave, y a
 * veces la dirección— mide más de tres pantallas: quien lo llena tiene que
 * acordarse de lo que escribió arriba mientras el teclado le tapa la mitad de
 * abajo. Preguntando de a una, cada pantalla entra sin scrollear incluso al
 * 200%, y el botón de avanzar está siempre en el mismo lugar.
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
    RbCard(title = "El computador del negocio") {
        Text(
            text = "Tus ventas no se guardan en el teléfono: se guardan en un computador tuyo. " +
                "Puede estar en el local o arrendado en internet.",
            style = RbTheme.typography.body,
            color = RbTheme.colors.textPrimary,
        )
        RbTextField(
            value = url,
            onValueChange = onUrl,
            label = "Dirección del computador",
            placeholder = "192.168.1.10:8080",
            supportingText = ayuda,
            errorMessage = error,
            keyboardType = KeyboardType.Uri,
            enabled = habilitado,
        )
        Text(
            text = "¿No la tienes? Pídesela a quien instaló el sistema en ese computador.",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )
    }
}

/** Cómo se llama el negocio. */
@Composable
internal fun PasoNegocio(
    nombre: String,
    onNombre: (String) -> Unit,
    habilitado: Boolean,
    onListo: () -> Unit,
) {
    RbCard(title = "El nombre de tu negocio") {
        RbTextField(
            value = nombre,
            onValueChange = onNombre,
            label = "Nombre del negocio",
            placeholder = "Almacén Doña Rosa",
            supportingText = "El que le dice la gente del barrio. Después se puede cambiar.",
            enabled = habilitado,
            imeAction = ImeAction.Next,
            onImeAction = onListo,
        )
    }
}

/**
 * A qué se dedica.
 *
 * Las ocho opciones de [RUBROS], en filas de lista y no en una grilla de
 * tarjetas: una grilla de dos columnas al 200% de escala parte las etiquetas
 * largas ("Minimarket / Almacén", "Restaurant / Comida") en tres renglones de
 * dos palabras, y una fila entera por opción es además un objetivo táctil que
 * no hay que apuntar.
 *
 * Ninguna viene elegida. Un rubro elegido de verdad prende y apaga módulos de
 * la app; uno puesto por defecto sólo se ve elegido.
 */
@Composable
internal fun PasoRubro(
    elegido: Rubro?,
    onElegir: (Rubro) -> Unit,
    habilitado: Boolean,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(RbTheme.dimens.space1),
    ) {
        Text(
            text = "Elige el que más se parezca. Con esto la app se acomoda a tu negocio: qué " +
                "campos te pide y qué te muestra.",
            style = RbTheme.typography.body,
            color = RbTheme.colors.textPrimary,
        )

        RUBROS.forEach { rubro ->
            val esElegido = rubro.clave == elegido?.clave
            RbListRow(
                title = rubro.etiqueta,
                subtitle = rubro.frase,
                trailing = if (esElegido) {
                    { RbChip(label = "Elegido", tone = RbChipTone.Brand) }
                } else {
                    null
                },
                onClick = if (habilitado) ({ onElegir(rubro) }) else null,
            )
        }
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
) {
    RbCard(title = "Con esto vas a entrar") {
        RbTextField(
            value = email,
            onValueChange = onEmail,
            label = "Tu correo",
            placeholder = "dueno@minegocio.cl",
            supportingText = "Es tu nombre de usuario para entrar.",
            errorMessage = errorDeCorreo,
            keyboardType = KeyboardType.Email,
            enabled = habilitado,
            imeAction = ImeAction.Next,
        )
        RbTextField(
            value = clave,
            onValueChange = onClave,
            label = "Tu clave",
            supportingText = "Al menos $LARGO_MINIMO_DE_CLAVE letras o números. Anótala: nadie " +
                "puede recuperártela por ti.",
            errorMessage = errorDeClave,
            keyboardType = KeyboardType.Password,
            visualTransformation = PasswordVisualTransformation(),
            enabled = habilitado,
            imeAction = ImeAction.Go,
            onImeAction = onListo,
        )
    }
}
