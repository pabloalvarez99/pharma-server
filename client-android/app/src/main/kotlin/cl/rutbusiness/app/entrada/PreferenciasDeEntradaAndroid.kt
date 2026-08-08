package cl.rutbusiness.app.entrada

import android.content.Context
import cl.rutbusiness.app.ui.entrada.PreferenciasDeEntrada

/**
 * Dónde queda anotado que esta persona ya entró alguna vez.
 *
 * `SharedPreferences` y no DataStore por lo mismo que las de la impresora: es
 * un booleano que hay que poder leer **sin suspender**, porque de él depende
 * cuál es la primera pantalla que se compone. Leerlo un frame después haría que
 * la app abriera el login y después saltara a la bienvenida, que es justo el
 * parpadeo que este proyecto evita en el arranque en frío.
 *
 * Acá no hay ningún secreto: un `true`.
 */
class PreferenciasDeEntradaAndroid(context: Context) : PreferenciasDeEntrada {

    private val prefs = context.applicationContext
        .getSharedPreferences("rutbusiness_entrada", Context.MODE_PRIVATE)

    override fun yaEntroAlgunaVez(): Boolean = prefs.getBoolean(ENTRO, false)

    override fun marcarQueEntro() {
        prefs.edit().putBoolean(ENTRO, true).apply()
    }

    private companion object {
        const val ENTRO = "ya_entro"
    }
}
