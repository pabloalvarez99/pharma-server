package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.rubro.RubroPack

/**
 * Un grupo de la lista "qué le puedo preguntar" (paso 3 del encargo de ola
 * 30). [titulo] describe lo que la dueña está tratando de hacer, nunca un
 * módulo del sistema — por eso no hay un grupo "ventas" ni "inventario".
 */
internal data class GrupoAyuda(val titulo: String, val frases: List<String>)

/** Copy del botón que abre/cierra la lista completa, con rama de feria. */
internal data class CopyAyuda(
    val titulo: String,
    val verTodo: String,
    val cerrar: String,
    val cuandoNoEntiendeTitulo: String,
    val cuandoNoEntiendeVerTodo: String,
)

/**
 * La lista completa de lo que el agente entiende, agrupada por lo que la
 * dueña está tratando de hacer.
 *
 * Cada frase de acá está verificada contra el parser real de
 * `crates/assist` (`intent.rs` para lectura, `actions.rs` para escritura):
 * son los ejemplos que los propios mensajes de ayuda del server («¿de cuánto
 * es el gasto? Por ejemplo…») usan, o frases de sus tests. Ninguna es
 * inventada — si no parsea, no está acá.
 *
 * `Intent::Ayuda` del server contesta esto mismo en un párrafo; un párrafo no
 * se puede tocar ni escanear con la vista, así que esta lista es la versión
 * tocable de la misma respuesta.
 */
internal fun gruposDeAyuda(pack: RubroPack): List<GrupoAyuda> {
    val feria = pack.features.agentHome || pack.rubro == "feria"

    val saberComoVoy = GrupoAyuda(
        titulo = "Saber cómo voy",
        frases = listOf(
            "¿Cuánto vendí hoy?",
            "Ventas de ayer",
            "Ventas del mes",
            "Ventas del mes pasado",
            "Ventas de la semana",
            "Ventas de hoy vs ayer",
            "Ventas por método de pago",
            "¿Cuánto me entró por transferencia hoy?",
            "¿Cuánto hay en caja?",
            "¿Quién me debe plata?",
            "Margen del mes",
            "Gastos del mes",
            "IVA del mes",
            "Top productos",
            "Mejores clientes",
            "¿Cuántos clientes tengo?",
            "Busca el cliente Juan Pérez",
            if (feria) "Precio de los tomates" else "Precio de paracetamol",
            "¿Cuánto me cuesta el tomate?",
            "¿Qué vendí recién?",
            "¿Qué se vence esta semana?",
            "¿Qué tengo que reponer?",
            "Resumen del inventario",
            "Prepárame el día",
            "Libro de controlados",
            "Recetas del mes",
            "Órdenes de compra pendientes",
            // "Proveedores" a secas parsea igual (`intent.rs:941`), pero un chip
            // de una palabra no se lee como algo que se le pueda *decir* al
            // agente: se lee como el nombre de una sección. La forma larga está
            // en los tests del parser (`intent.rs:942`) y enseña el tono.
            "Lista de proveedores",
        ),
    )

    val anotarLoQuePaso = GrupoAyuda(
        titulo = "Anotar lo que pasó",
        frases = listOfNotNull(
            if (feria) "Vendí 2 kg de tomates a 2000" else "Véndeme 2 paracetamol",
            if (feria) "Anota 2 kg de tomates a 2000 fiado a Don Juan" else "Fíale 2 paracetamol a Juan",
            "Registra un gasto de 5000 en arriendo",
            "Crea un cliente Juan Pérez rut 12.345.678-9",
            "Crea el proveedor Farmaltda rut 76.123.456-7",
            if (feria) "Crea un producto Tomates a $1000" else "Crea un producto Aspirina a $1000",
            "Abónale 5000 a doña Ana",
            "Abre la caja con $50.000",
            if (feria) {
                "Crea una orden de compra de 10 tomates a Farmaltda a $500"
            } else {
                "Crea una orden de compra de 10 paracetamol a Farmaltda a $500"
            },
            if (!feria) "Registra una receta a Juan Pérez rut 12.345.678-9 de paracetamol" else null,
        ),
    )

    val arreglarAlgo = GrupoAyuda(
        titulo = "Arreglar algo que salió mal",
        frases = listOf(
            if (feria) "Cambia el precio de los tomates a $1500" else "Cambia el precio de paracetamol a $1500",
            if (feria) "Repón 40 de tomates" else "Repón 40 de paracetamol",
            if (feria) {
                "Ajusta el inventario de tomates a 100"
            } else {
                "Ajusta el stock de paracetamol a 100"
            },
            "Cierra la caja con $50.000",
            "Recibe la orden de compra",
            "Cancela la orden de compra",
        ),
    )

    return listOf(saberComoVoy, anotarLoQuePaso, arreglarAlgo)
}

internal fun copyAyuda(feria: Boolean): CopyAyuda =
    if (feria) {
        CopyAyuda(
            titulo = "Todo lo que me puedes preguntar",
            verTodo = "¿Qué más te puedo decir?",
            cerrar = "Volver a la charla",
            cuandoNoEntiendeTitulo = "Eso no lo pillé.",
            cuandoNoEntiendeVerTodo = "Mira todo lo que te puedo decir",
        )
    } else {
        CopyAyuda(
            titulo = "Todo lo que me puedes pedir",
            verTodo = "¿Qué más puedo preguntarte?",
            cerrar = "Volver a la conversación",
            cuandoNoEntiendeTitulo = "No entendí esa.",
            cuandoNoEntiendeVerTodo = "Ver todo lo que puedo preguntarte",
        )
    }
