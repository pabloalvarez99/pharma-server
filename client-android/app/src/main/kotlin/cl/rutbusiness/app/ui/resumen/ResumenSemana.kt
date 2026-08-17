package cl.rutbusiness.app.ui.resumen

import cl.rutbusiness.core.money.Dinero
import kotlinx.serialization.Serializable
import kotlin.math.pow

/**
 * Un día del bloque semanal, ya con su plata adentro.
 *
 * `revenue` viaja como el mismo texto decimal que mandó el server — nunca se
 * reescribe acá. Un día sin fila en `sales-daily` se completa con `"0"`: no
 * hay fila para un día sin ventas, no un hueco en el gráfico (ver
 * [armarSemana]).
 *
 * Público, a diferencia del resto de este archivo: viaja adentro de
 * [ResumenGuardado], que es el snapshot offline y es público. Las funciones
 * que lo arman siguen siendo `internal` — lo que sale del módulo es el dato,
 * nunca la manera de calcularlo.
 */
@Serializable
data class DiaDeLaSemana(
    val fecha: String,
    val diaDeLaSemana: Int,
    val revenue: String,
    val esHoy: Boolean,
)

/**
 * Arma los 7 días del bloque semanal.
 *
 * Los 6 días pasados salen de `sales-daily` (`filas`), emparejados por
 * **fecha** — nunca por posición, porque un día sin ventas no manda fila y
 * correr el índice metería el total equivocado en el día equivocado. El día
 * de hoy no se busca en `filas` (nunca se pidió: ver [DiaUtc.rangoDeSemana])
 * y usa directamente [revenueHoy], el mismo texto que ya mostró la cifra
 * grande — así los dos "hoy" de la pantalla nunca pueden mostrar números
 * distintos.
 *
 * @param diasEsqueleto los 7 días de [DiaUtc.diasDeLaSemana], del más viejo
 *   al de hoy.
 * @param filas lo que contestó `sales-daily` para el rango de
 *   [DiaUtc.rangoDeSemana]. Puede faltar cualquier día, y nunca incluye hoy.
 * @param revenueHoy el texto de `ventasHoy.revenue` del agregado.
 */
internal fun armarSemana(
    diasEsqueleto: List<DiaEnLaSemana>,
    filas: List<VentaDiariaDto>,
    revenueHoy: String,
): List<DiaDeLaSemana> {
    val porFecha = filas.associateBy { it.date }
    return diasEsqueleto.map { dia ->
        DiaDeLaSemana(
            fecha = dia.fecha,
            diaDeLaSemana = dia.diaDeLaSemana,
            revenue = if (dia.esHoy) revenueHoy else porFecha[dia.fecha]?.revenue ?: "0",
            esHoy = dia.esHoy,
        )
    }
}

/**
 * El día con más plata de la semana, comparando — nunca sumando.
 *
 * La regla de plata prohíbe armar un total de la semana en el teléfono, así
 * que no hay "vendiste $X esta semana" posible acá. Lo único que se puede
 * decir sin inventar un monto es una comparación entre montos que el server
 * ya mandó completos, y eso es exactamente lo que hace [Dinero.compareTo]:
 * no sale un peso nuevo de acá.
 *
 * Un día en $0 no cuenta como "mejor día" — si nadie vendió nada en toda la
 * semana no hay mejor día que anunciar, y decir "el mejor día fue el lunes,
 * con $0" leería como un dato cuando es la ausencia de uno.
 */
internal fun mejorDiaDeLaSemana(dias: List<DiaDeLaSemana>): DiaDeLaSemana? =
    dias
        .mapNotNull { dia -> Dinero.deTextoDeServidor(dia.revenue)?.let { dia to it } }
        .filter { (_, monto) -> monto > Dinero.CERO }
        .maxByOrNull { (_, monto) -> monto }
        ?.first

/**
 * Qué tan alta dibujar la barra de un día, en una escala sin unidad.
 *
 * Ésta es la única conversión de un monto a `Float` que se permite en toda
 * la pantalla: normalizar contra el máximo de la semana es geometría de un
 * dibujo, no plata — nada de esto se muestra como cifra, y la cifra que
 * *sí* se muestra sigue saliendo siempre del texto del server intacto (ver
 * [cl.rutbusiness.ui.components.RbAmount]).
 *
 * Un monto que no parsea (texto vacío, corrupto) no puede tumbar el gráfico
 * entero: se dibuja como un día en cero, igual que un día sin ventas.
 */
internal fun alturaDeBarra(revenue: String): Float {
    val dinero = Dinero.deTextoDeServidor(revenue) ?: return 0f
    if (dinero.unidades <= 0L) return 0f
    return (dinero.unidades / 10.0.pow(dinero.escala)).toFloat()
}
