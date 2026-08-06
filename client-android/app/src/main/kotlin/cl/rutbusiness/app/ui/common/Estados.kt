package cl.rutbusiness.app.ui.common

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

/**
 * Estados de carga, vacío y error.
 *
 * Existen para cumplir una regla sola: **nunca una pantalla en blanco**. Si no
 * hay datos todavía, si no hay datos nunca, o si algo falló, siempre hay algo
 * escrito que dice qué está pasando y qué hacer.
 *
 * TODO(design-system): estos tres son provisorios y feos a propósito. Cuando
 * `ui/components/` aterrice, se reemplazan por los del design system.
 */

@Composable
fun PantallaCargando(mensaje: String) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        CircularProgressIndicator()
        Text(
            text = mensaje,
            modifier = Modifier.padding(top = 16.dp),
            style = MaterialTheme.typography.bodyLarge,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
fun PantallaVacia(
    titulo: String,
    detalle: String,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(text = titulo, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
        Text(
            text = detalle,
            modifier = Modifier.padding(top = 8.dp),
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
fun PantallaError(
    mensaje: String,
    textoAccion: String = "Reintentar",
    onAccion: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = mensaje,
            style = MaterialTheme.typography.bodyLarge,
            textAlign = TextAlign.Center,
        )
        Button(
            onClick = onAccion,
            modifier = Modifier
                .padding(top = 20.dp)
                .fillMaxWidth()
                // Piso de hardware, regla 4: el pulso del usuario no es el de un
                // cajero de 25 años. 56 dp, no los 48 dp de Material.
                .sizeIn(minHeight = 56.dp),
        ) {
            Text(textoAccion)
        }
    }
}
