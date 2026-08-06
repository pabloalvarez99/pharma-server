package cl.rutbusiness.app

import android.app.Application

class RutBusinessApplication : Application() {

    /** Ver [AppContainer]: el grafo de dependencias, armado a mano. */
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
    }
}
