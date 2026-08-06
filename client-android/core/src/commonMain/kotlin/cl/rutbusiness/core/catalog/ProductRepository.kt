package cl.rutbusiness.core.catalog

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.cuerpo
import cl.rutbusiness.core.net.llamar

/**
 * Productos del negocio, vía el `CatalogApi` generado desde el OpenAPI.
 *
 * No hay ni una URL escrita a mano acá: la ruta, el método y el header de
 * autenticación salen del spec. Lo único que agrega esta clase es el tipo de la
 * respuesta, porque el spec todavía declara el body como `object` opaco.
 */
class ProductRepository(private val api: ApiFactory) {

    suspend fun listar(): Resultado<List<ProductDto>> = llamar(api.baseUrl) {
        api.catalog().listProducts().cuerpo<List<ProductDto>>(api.baseUrl)
    }
}
