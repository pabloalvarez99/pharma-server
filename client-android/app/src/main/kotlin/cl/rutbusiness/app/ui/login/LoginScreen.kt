package cl.rutbusiness.app.ui.login

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.components.RbButton
import cl.rutbusiness.ui.components.RbTextField
import cl.rutbusiness.ui.theme.RbTheme
import cl.rutbusiness.ui.theme.rbHeading

@Composable
fun LoginRoute(sesion: SessionRepository, estado: EstadoSesion.SinSesion) {
    val vm: LoginViewModel = viewModel(
        key = "login",
        factory = viewModelFactory { initializer { LoginViewModel(sesion, estado) } },
    )
    LoginScreen(vm)
}

/**
 * Pantalla de entrada.
 *
 * La dirección del server es un campo de primera clase, no una preferencia
 * escondida: en este producto **lo normal** es que el teléfono sea un cliente
 * liviano contra un server que vive en otra parte -- el PC del negocio o la
 * nube. Ese modo no es una versión degradada de nada, y el copy lo dice para
 * que nadie crea que le falta algo por instalar.
 */
@Composable
private fun LoginScreen(vm: LoginViewModel) {
    val dimens = RbTheme.dimens

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(dimens.space4),
        verticalArrangement = Arrangement.spacedBy(dimens.space3),
    ) {
        Text(
            text = "RutBusiness",
            style = RbTheme.typography.title,
            color = RbTheme.colors.textPrimary,
            modifier = Modifier.rbHeading(),
        )
        Text(
            text = "Entra al servidor de tu negocio.",
            style = RbTheme.typography.body,
            color = RbTheme.colors.textSecondary,
        )

        RbTextField(
            value = vm.url,
            onValueChange = vm::cambiarUrl,
            label = "Dirección del servidor",
            supportingText = "En el emulador: http://10.0.2.2:8080 · En un teléfono: la IP del PC " +
                "en la red, por ejemplo http://192.168.1.10:8080",
            keyboardType = KeyboardType.Uri,
            enabled = !vm.enviando,
        )
        Text(
            text = "El servidor puede estar en el PC del negocio, en la misma red wifi, o en " +
                "internet. Tu teléfono solo necesita la dirección: no guarda la base de datos ni " +
                "le hace falta. Así es como funciona normalmente.",
            style = RbTheme.typography.support,
            color = RbTheme.colors.textSecondary,
        )

        RbTextField(
            value = vm.sucursal,
            onValueChange = vm::cambiarSucursal,
            label = "Sucursal",
            supportingText = "El nombre corto que te dieron al crear el negocio.",
            enabled = !vm.enviando,
        )
        RbTextField(
            value = vm.email,
            onValueChange = vm::cambiarEmail,
            label = "Correo",
            keyboardType = KeyboardType.Email,
            enabled = !vm.enviando,
        )
        RbTextField(
            value = vm.password,
            onValueChange = vm::cambiarPassword,
            label = "Contraseña",
            keyboardType = KeyboardType.Password,
            visualTransformation = PasswordVisualTransformation(),
            errorMessage = vm.error,
            enabled = !vm.enviando,
        )

        RbButton(
            label = if (vm.enviando) "Entrando..." else "Entrar",
            onClick = vm::entrar,
            enabled = vm.puedeEntrar,
            fillWidth = true,
        )
    }
}
