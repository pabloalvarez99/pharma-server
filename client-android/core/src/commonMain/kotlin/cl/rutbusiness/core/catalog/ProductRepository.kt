package cl.rutbusiness.core.catalog

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.cuerpo
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.parameter

/**
 * Productos del negocio.
 *
 * El lookup por código de barras sale del `CatalogApi` generado, con ruta,
 * método y auth tomados del spec. La búsqueda por texto va a mano por una razón
 * concreta: `GET /api/v1/products` **sí** acepta `search`, `limit` y `offset`
 * (`ProductFilters` en `crates/domain/src/catalog/model.rs`), pero el
 * `#[utoipa::path]` del handler no los declara, así que el generador produce
 * un `listProducts()` sin parámetros. El día que se anoten, este método pasa a
 * ser una línea contra el cliente generado.
 */
class ProductRepository(private val api: ApiFactory) {

    /**
     * Busca por nombre.
     *
     * Filtra en el server, no en el teléfono: el aparato objetivo tiene 1-2 GB
     * y hay tenants con miles de SKU. [limite] acota lo que viaja por datos
     * móviles caros.
     */
    suspend fun buscar(
        texto: String,
        limite: Int = LIMITE_BUSQUEDA,
        desplazamiento: Int = 0,
    ): Resultado<List<ProductDto>> = llamar(api.baseUrl) {
        api.http.get("${api.baseUrl}/api/v1/products") {
            val limpio = texto.trim()
            if (limpio.isNotEmpty()) parameter("search", limpio)
            parameter("active", true)
            parameter("limit", limite)
            parameter("offset", desplazamiento)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Lectura directa por código de barras.
     *
     * Es el camino del escáner: un código no se "busca", se resuelve a un
     * producto o no existe. 404 llega como error y la pantalla lo muestra como
     * "ese código no está en tu catálogo", no como una lista vacía.
     */
    suspend fun porCodigoDeBarras(codigo: String): Resultado<ProductDto> = llamar(api.baseUrl) {
        api.catalog().getByBarcode(codigo.trim()).cuerpo<ProductDto>(api.baseUrl)
    }

    companion object {
        /**
         * Cuántos productos trae una búsqueda.
         *
         * 40 llena varias pantallas de scroll sin que la respuesta pese: el
         * `LazyColumn` sólo compone lo visible, pero el JSON igual viaja entero
         * por la red del negocio.
         */
        const val LIMITE_BUSQUEDA = 40
    }
}
