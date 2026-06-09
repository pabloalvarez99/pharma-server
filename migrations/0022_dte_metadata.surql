-- pharma-server 0022: campo `metadata` en dte (Fase 9.1 wiring API).
--
-- `crates/dte` `Dte.metadata` existía sólo in-memory: la 0017 no lo definía y
-- SCHEMAFULL descarta campos no declarados, así que el trail de cancelación
-- (`cancelled_at`/`cancelled_reason`, escrito por `cancel_dte`) se perdía al
-- persistir. Este campo lo hace durable. FLEXIBLE: el contenido es libre
-- (append-only conceptual, lo escribe el módulo dte en cada transición).

DEFINE FIELD metadata ON dte FLEXIBLE TYPE option<object>;
