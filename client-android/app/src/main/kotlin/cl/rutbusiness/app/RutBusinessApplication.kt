package cl.rutbusiness.app

import android.app.Application

/**
 * Punto donde vive el grafo de dependencias de la app.
 *
 * A propósito no hay framework de DI: el grafo es un puñado de objetos creados
 * a mano. En un aparato de 1-2 GB de RAM, cada milisegundo y cada clase que se
 * carga en el arranque se paga; un contenedor manual arranca en cero.
 */
class RutBusinessApplication : Application()
