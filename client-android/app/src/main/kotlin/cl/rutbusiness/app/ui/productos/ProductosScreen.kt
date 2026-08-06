package cl.rutbusiness.app.ui.productos

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import cl.rutbusiness.app.ui.common.PantallaCargando
import cl.rutbusiness.app.ui.common.PantallaError
import cl.rutbusiness.app.ui.common.PantallaVacia
import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.format.formatearCLP
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository

@Composable
fun ProductosRoute(sesion: SessionRepository, estado: EstadoSesion.Activa) {
    val vm: ProductosViewModel = viewModel(
        key = "productos:${estado.baseUrl}",
        factory = viewModelFactory {
            initializer { ProductosViewModel(sesion) }
        },
    )
    ProductosScreen(vm, estado.baseUrl)
}

/**
 * Primera pantalla con datos reales del server: el catálogo del negocio.
 *
 * `LazyColumn` y no `Column` con scroll: con 1-2 GB de RAM no se cargan listas
 * enteras en memoria (piso de hardware, regla 5). Solo se componen las filas
 * visibles, tenga el negocio 10 productos o 10.000.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProductosScreen(vm: ProductosViewModel, baseUrl: String) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Productos") },
                actions = {
                    TextButton(onClick = vm::salir) { Text("Salir") }
                },
            )
        },
    ) { padding ->
        Column(modifier = Modifier.padding(padding).fillMaxSize()) {
            Text(
                text = baseUrl,
                modifier = Modifier.padding(horizontal = 16.dp),
                style = MaterialTheme.typography.bodySmall,
            )

            when (val estado = vm.estado) {
                EstadoProductos.Cargando -> PantallaCargando("Buscando tus productos…")

                is EstadoProductos.Error -> PantallaError(
                    mensaje = estado.mensaje,
                    onAccion = vm::cargar,
                )

                is EstadoProductos.Listo -> if (estado.productos.isEmpty()) {
                    PantallaVacia(
                        titulo = "Todavía no hay productos",
                        detalle = "Cuando cargues productos en el sistema del negocio, van a aparecer acá.",
                    )
                } else {
                    LazyColumn(modifier = Modifier.fillMaxSize()) {
                        items(
                            items = estado.productos,
                            key = { it.id },
                        ) { producto ->
                            FilaProducto(producto)
                            HorizontalDivider()
                        }
                    }
                }
            }
        }
    }
}

/**
 * TODO(design-system): reemplazar por la fila de lista del design system.
 */
@Composable
private fun FilaProducto(producto: ProductDto) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = producto.name,
                style = MaterialTheme.typography.bodyLarge,
            )
            Text(
                text = if (producto.stock > 0) "${producto.stock} en stock" else "Sin stock",
                style = MaterialTheme.typography.bodySmall,
            )
        }
        Text(
            text = formatearCLP(producto.price),
            style = MaterialTheme.typography.titleMedium,
        )
    }
}
