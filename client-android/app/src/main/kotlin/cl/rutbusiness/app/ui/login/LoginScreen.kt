package cl.rutbusiness.app.ui.login

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository

@Composable
fun LoginRoute(sesion: SessionRepository, estado: EstadoSesion.SinSesion) {
    val vm: LoginViewModel = viewModel(
        key = "login",
        factory = viewModelFactory {
            initializer { LoginViewModel(sesion, estado) }
        },
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
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .imePadding()
            .padding(24.dp),
    ) {
        Text(
            text = "RutBusiness",
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text = "Entra al servidor de tu negocio.",
            modifier = Modifier.padding(top = 4.dp),
            style = MaterialTheme.typography.bodyLarge,
        )

        Campo(
            valor = vm.url,
            onValorCambia = vm::cambiarUrl,
            etiqueta = "Dirección del servidor",
            apoyo = "En el emulador: http://10.0.2.2:8080 · En un teléfono: la IP del PC en la red, por ejemplo http://192.168.1.10:8080",
            tipoTeclado = KeyboardType.Uri,
            habilitado = !vm.enviando,
        )
        Text(
            text = "El servidor puede estar en el PC del negocio, en la misma red wifi, o en internet. " +
                "Tu teléfono solo necesita la dirección: no guarda la base de datos ni le hace falta. " +
                "Así es como funciona normalmente.",
            modifier = Modifier.padding(top = 4.dp),
            style = MaterialTheme.typography.bodySmall,
        )

        Campo(
            valor = vm.sucursal,
            onValorCambia = vm::cambiarSucursal,
            etiqueta = "Sucursal",
            apoyo = "El nombre corto que te dieron al crear el negocio.",
            habilitado = !vm.enviando,
        )
        Campo(
            valor = vm.email,
            onValorCambia = vm::cambiarEmail,
            etiqueta = "Correo",
            tipoTeclado = KeyboardType.Email,
            habilitado = !vm.enviando,
        )
        Campo(
            valor = vm.password,
            onValorCambia = vm::cambiarPassword,
            etiqueta = "Contraseña",
            tipoTeclado = KeyboardType.Password,
            esContrasena = true,
            ultimoCampo = true,
            habilitado = !vm.enviando,
        )

        vm.error?.let { mensaje ->
            Text(
                text = mensaje,
                modifier = Modifier.padding(top = 16.dp),
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
            )
        }

        Button(
            onClick = vm::entrar,
            enabled = vm.puedeEntrar,
            modifier = Modifier
                .padding(top = 24.dp)
                .fillMaxWidth()
                // Piso de hardware, regla 4: 56 dp, no los 48 dp de Material.
                .sizeIn(minHeight = 56.dp),
        ) {
            if (vm.enviando) {
                CircularProgressIndicator(modifier = Modifier.sizeIn(maxHeight = 24.dp))
            } else {
                Text("Entrar")
            }
        }
    }
}

/**
 * TODO(design-system): reemplazar por el campo de texto del design system.
 * Provisorio: `OutlinedTextField` pelado.
 */
@Composable
private fun Campo(
    valor: String,
    onValorCambia: (String) -> Unit,
    etiqueta: String,
    modifier: Modifier = Modifier,
    apoyo: String? = null,
    tipoTeclado: KeyboardType = KeyboardType.Text,
    esContrasena: Boolean = false,
    ultimoCampo: Boolean = false,
    habilitado: Boolean = true,
) {
    OutlinedTextField(
        value = valor,
        onValueChange = onValorCambia,
        label = { Text(etiqueta) },
        supportingText = apoyo?.let { { Text(it) } },
        singleLine = true,
        enabled = habilitado,
        visualTransformation = if (esContrasena) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = KeyboardOptions(
            keyboardType = tipoTeclado,
            imeAction = if (ultimoCampo) ImeAction.Done else ImeAction.Next,
        ),
        modifier = modifier
            .fillMaxWidth()
            .padding(top = 16.dp),
    )
}
