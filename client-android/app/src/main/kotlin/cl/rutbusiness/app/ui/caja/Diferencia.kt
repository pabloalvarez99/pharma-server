package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda

/** Cómo terminó el arqueo. */
enum class Cuadre {
    /** La plata contada es exactamente la anotada. */
    Justo,

    /** Hay menos plata en el cajón que la anotada. */
    Falta,

    /** Hay más plata en el cajón que la anotada. */
    Sobra,

    /** El server no mandó diferencia. Nunca se inventa una. */
    Desconocido,
}

/**
 * La diferencia del cierre, leída — no calculada — del texto que mandó el
 * server.
 *
 * @param cuadre de qué lado quedó, sacado del **signo** del decimal del server.
 * @param magnitud el mismo texto del server sin el signo menos, para poder
 *   escribir "Faltan $2.500" en vez de "Faltan -$2.500". Es un recorte de un
 *   carácter sobre la cadena original: no hay resta, ni valor absoluto
 *   aritmético, ni cambio de escala, así que los dígitos que se muestran son
 *   byte por byte los que grabó el cierre. `null` si el server no mandó nada.
 */
data class LecturaDeDiferencia(
    val cuadre: Cuadre,
    val magnitud: String?,
)

/**
 * Traduce `discrepancia` a algo que se pueda escribir en pantalla.
 *
 * El server la define como `contado − esperado`
 * (`domain::invariants::discrepancy`): negativa = falta plata, positiva =
 * sobra. Acá sólo se mira el signo, que es lo mismo que hace
 * [cl.rutbusiness.core.money.Dinero.compareTo] y por el mismo motivo: ordenar
 * no produce un monto nuevo.
 *
 * Un texto que no se entiende cae en [Cuadre.Desconocido] y **no** en "cuadró".
 * Decirle a la dueña que la caja cuadró cuando en realidad no se pudo leer la
 * diferencia es la peor de las respuestas posibles: es la única que hace que
 * deje de contar.
 */
fun leerDiferencia(discrepanciaDelServidor: String?): LecturaDeDiferencia {
    val texto = discrepanciaDelServidor?.trim()
    if (texto.isNullOrEmpty()) return LecturaDeDiferencia(Cuadre.Desconocido, null)

    val dinero = Dinero.deTextoDeServidor(texto)
        ?: return LecturaDeDiferencia(Cuadre.Desconocido, null)

    val sinSigno = texto.removePrefix("-").removePrefix("+")
    return when {
        dinero.unidades == 0L -> LecturaDeDiferencia(Cuadre.Justo, sinSigno)
        dinero.unidades < 0L -> LecturaDeDiferencia(Cuadre.Falta, sinSigno)
        else -> LecturaDeDiferencia(Cuadre.Sobra, sinSigno)
    }
}

/**
 * Lo que la dueña lee cuando la caja cierra con diferencia.
 *
 * @param titular la frase corta y grande: "Faltan $2.500".
 * @param explicacion los dos números que la producen, sin interpretarlos.
 * @param calma por qué esto pasa, escrito de forma que no acuse a nadie.
 */
data class CopyDeDiferencia(
    val titular: String,
    val explicacion: String,
    val calma: String,
)

/**
 * El texto exacto del cierre.
 *
 * Es la pantalla más delicada del día. Si se siente como una auditoría, la dueña
 * deja de cerrar caja — y una caja que no se cierra no sirve para nada. Tres
 * reglas de redacción, y las tres tienen consecuencia:
 *
 * - **Nadie es el sujeto de la falta.** Se dice "Faltan $2.500", nunca "te
 *   faltan" ni "hay un faltante de". La plata falta; la persona no falló.
 * - **Se nombra la causa probable, que casi nunca es robo.** Un vuelto mal dado
 *   y una compra chica sin anotar explican la enorme mayoría de las diferencias
 *   de un almacén, y decirlo saca de la mesa lo que la dueña está pensando.
 * - **No hay signos de admiración, ni colores de alarma, ni la palabra
 *   "error".** El chip que acompaña usa el tono neutro justamente para eso.
 *
 * Función pura para que el texto se pueda probar palabra por palabra sin montar
 * la pantalla: `DiferenciaTest` fija las frases, así que cambiarlas de gusto
 * rompe el build y obliga a pensarlo de nuevo.
 */
fun copyDeDiferencia(
    moneda: Moneda,
    contadoDelServidor: String?,
    esperadoDelServidor: String?,
    discrepanciaDelServidor: String?,
): CopyDeDiferencia {
    val lectura = leerDiferencia(discrepanciaDelServidor)
    val contado = contadoDelServidor?.let { moneda.formatear(it) }
    val esperado = esperadoDelServidor?.let { moneda.formatear(it) }
    val diferencia = lectura.magnitud?.let { moneda.formatear(it) }

    val comparacion = if (contado != null && esperado != null) {
        "Contaste $contado y el sistema tenía anotados $esperado."
    } else {
        "El cierre quedó guardado con lo que contaste."
    }

    return when (lectura.cuadre) {
        Cuadre.Justo -> CopyDeDiferencia(
            titular = "La caja cuadró",
            explicacion = if (contado != null) {
                "Contaste $contado y es justo lo que el sistema tenía anotado."
            } else {
                comparacion
            },
            calma = "Listo por hoy.",
        )

        Cuadre.Falta -> CopyDeDiferencia(
            titular = "Faltan ${diferencia ?: "algo"}",
            explicacion = comparacion,
            calma = "Casi siempre es un vuelto de más o una compra chica que no se alcanzó a " +
                "anotar. Queda guardado así y mañana empiezas de nuevo.",
        )

        Cuadre.Sobra -> CopyDeDiferencia(
            titular = "Sobran ${diferencia ?: "algo"}",
            explicacion = comparacion,
            calma = "Suele ser una venta que se cobró en efectivo y quedó anotada de otra " +
                "forma. Queda guardado así y mañana empiezas de nuevo.",
        )

        Cuadre.Desconocido -> CopyDeDiferencia(
            titular = "Caja cerrada",
            explicacion = if (contado != null) {
                "La caja quedó cerrada con los $contado que contaste."
            } else {
                comparacion
            },
            // Se dice que falta el dato en vez de mostrar un cero: "cuadró"
            // cuando en realidad no se pudo comparar es la única respuesta que
            // hace que la dueña deje de contar.
            calma = "No pudimos traer la comparación con lo anotado. El cierre igual quedó " +
                "guardado: vuelve a abrir la caja más tarde para ver cómo quedó.",
        )
    }
}
