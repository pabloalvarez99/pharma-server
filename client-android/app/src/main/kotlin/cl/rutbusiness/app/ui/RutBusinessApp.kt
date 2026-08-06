package cl.rutbusiness.app.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier

/**
 * Raíz de la UI. Kotlin común: ni un solo `import android.*` acá abajo.
 *
 * TODO(design-system): envolver en el tema real cuando aterrice `ui/theme/`.
 * Por ahora [MaterialTheme] con los defaults de Material 3, feo pero honesto.
 */
@Composable
fun RutBusinessApp() {
    MaterialTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = "Hola, RutBusiness",
                    style = MaterialTheme.typography.headlineMedium,
                )
            }
        }
    }
}
