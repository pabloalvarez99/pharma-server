-- pharma-server 0045: índice full-text para la búsqueda por texto del POS.
--
-- `GET /api/v1/products?search=…` es lo que usa la cajera cuando NO escanea, o
-- sea la mitad de las ventas. La condición era
--
--   string::lowercase(name) CONTAINS $q
--   OR string::lowercase(active_ingredient ?? '') CONTAINS $q
--   OR external_id = $raw_q
--
-- y ningún índice puede servir eso, por dos razones independientes:
--
-- 1. `CONTAINS` sobre un string es subcadena. Un índice B-tree ordena por el
--    valor completo, así que sólo sabe responder por prefijo o por igualdad.
-- 2. Aunque el `CONTAINS` fuera indexable, el `OR` lo tira abajo igual:
--    SurrealDB 2.6 sólo arma un plan multi-índice cuando TODAS las expresiones
--    de la condición están respaldadas por un índice
--    (`idx/planner/plan.rs::build`, rama `all_expressions_with_index`), y una
--    expresión sólo cuenta como respaldada si su campo es la COLUMNA 0 del
--    índice (`idx/planner/tree.rs::eval_index_operator` devuelve `None` para
--    `col != 0`). `active` y `parent_id` viven en la columna 1 de sus índices
--    compuestos, así que nunca califican y el planner cae a `Iterate Table`.
--
-- Medido en release con 3.000 productos: 78 ms p50 / 110 ms p99 para devolver
-- 40 filas, plan estable `Iterate Table`.
--
-- La salida es un índice full-text. Tres cosas lo hacen encajar:
--
-- * **Un solo índice cubre los tres campos.** Un índice SEARCH con varias
--   columnas NO indexa cada campo por separado: `idx/index.rs::index_full_text`
--   le pasa al analizador los valores de TODAS las columnas juntos y el
--   documento queda con una sola bolsa de términos
--   (`idx/ft/mod.rs::index_document`). Por eso alcanza con preguntar por la
--   columna 0 (`name @@ $q`) para que un match en `active_ingredient` o en
--   `external_id` también entre: desaparece el `OR`, y con él el `Iterate
--   Table`.
-- * **EDGENGRAM da la búsqueda mientras se tipea.** El filtro de n-gramas se
--   aplica sólo al INDEXAR, nunca al consultar
--   (`idx/ft/analyzer/filter.rs::is_stage`). El documento «Paracetamol» deja los
--   términos p, pa, par, para, …; la consulta «para» se busca tal cual y pega.
--   Desde 1 carácter porque el cliente Android busca desde la primera tecla
--   (debounce de 250 ms, sin largo mínimo).
-- * **ASCII antes que LOWERCASE** para que «Ibuprofeno» y «ibuprofeno» y
--   «IBUPROFÉNO» caigan en el mismo término.
--
-- Qué cambia para el usuario: la búsqueda pasa de subcadena en cualquier
-- posición a prefijo de palabra. «para» sigue encontrando «Paracetamol» y
-- «amox» a «Amoxicilina»; «cetamol» ya no encuentra «Paracetamol». Es lo que
-- hace cualquier buscador de POS y es el precio de que la consulta use índice:
-- una subcadena arbitraria no es indexable con ninguna estructura ordenada.
-- El tokenizador CLASS corta al cambiar de clase de carácter, así que «500 mg»,
-- «500mg» y «SKU000123» quedan como («500», «mg») y («sku», «000123»): buscar
-- el código externo completo sigue funcionando.
--
-- No toca el esquema ni los datos: define un analizador y un índice, y no
-- obliga a reimportar nada.
--
-- Lo que sí cuesta es construirlo, una sola vez, al arrancar la primera vez con
-- esta versión: medido, 59 s con 3.000 productos (el arranque normal es de
-- segundos). Es el precio de los n-gramas — cada palabra deja un término por
-- cada prefijo — y se paga una vez por instalación, no por consulta. Un tenant
-- con un catálogo grande va a ver ese arranque largo y después nada.

DEFINE ANALYZER producto_busqueda
    TOKENIZERS CLASS
    FILTERS ASCII, LOWERCASE, EDGENGRAM(1,30);

DEFINE INDEX product_search ON product
    FIELDS name, active_ingredient, external_id
    SEARCH ANALYZER producto_busqueda;
