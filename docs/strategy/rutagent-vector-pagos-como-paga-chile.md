# Vector V4 — Pagos como paga Chile (transferencia + apps + conciliación)

> Ultra-plan del próximo vector de profundidad. Orquestador, 2026-07-26.
> Enmarcado por la visión `docs/strategy/rutagent-vision-microempresa-cl.md` (principio
> "paga como Chile" + "cero fricción"). Se construye COMPLETO, sin módulos a medias.

## Por qué este vector, ahora

El loop diario del negocio es **vender → cobrar → formalizar**. El cobro hoy soporta
efectivo, tarjeta y fiado (V1). **Falta transferencia** — el método no-efectivo dominante
del micronegocio chileno ("te hago la transfer"), más las apps (MercadoPago, MACH, Sumup,
Getnet). Sin transferencia, la plata que entra queda mal registrada → IVA, reportes y las
respuestas del agente ("¿cuánto vendí hoy?") mienten. Es el eslabón débil del loop, se
siente todos los días, no tiene bloqueo externo (ni LLM, ni certificación SII, ni API de
banco, ni costo por uso), y sigue el patrón ya probado del tender `pos_fiado`.

## Retrato de uso (qué debe funcionar)

- Don Luis en la feria cobra $2.000 y el cliente dice "te transfiero". Marca **Transferencia**
  en el POS, la venta queda registrada como ingreso electrónico, **la caja física NO lo
  espera en el arqueo** (no es efectivo en el cajón).
- Al cierre, la caja cuadra con el efectivo real; la transferencia va a su propio bucket.
- El dueño le pregunta al agente: *"¿cuánto me entró por transferencia hoy?"* / *"¿cuánto
  por efectivo vs transferencia?"* y responde correcto.
- Venta partida: $3.000 efectivo + $2.000 transferencia en una sola venta (slice 3).
- *"¿qué transferencias tengo pendientes?"* → las que aún no confirmó que llegaron; *"marca
  la de la mesa 3 como recibida"* (slice 2).

## Principios de diseño (correctitud, casa)

1. **Ledger append-only** (estilo `customer_ledger`/`stock_movement`): el ingreso electrónico
   es una fila inmutable, el total se CALCULA sumando, nunca se muta.
2. **Invariante de arqueo intacto** (el guardián de correctitud de este vector): el efectivo
   esperado en el cajón = suma de tenders EFECTIVO solamente. Los tenders no-efectivo
   (tarjeta, transferencia, app) NO inflan el arqueo de caja. Espejar exactamente cómo se
   comporta hoy `tarjeta` (confirmar en el código antes de construir; mirrorear su ruta).
3. **Idempotencia por venta** (ya existe `Idempotency-Key`).
4. **Whitelist de tender** vía `DEFINE FIELD OVERWRITE` sobre `order.payment_method`
   (mismo patrón que 0039 agregó `pos_fiado` — el test cazó que sin esto la venta no
   persistía; repetir el test).
5. **Móvil primero** (celular es el dispositivo real): tender en 1 toque, sub-selector de
   app claro, estados pendiente/confirmado legibles en pantalla chica.

## Build en slices (cada uno COMPLETO y shippable — DoD por slice)

### Slice 1 — Transferencia como tender, punta a punta (lane inmediata)
El slice marquee, completo y mergeable solo:
- **Backend**: migración nueva (nº libre según bitácora al arrancar, ≈0044 tras venc):
  `transferencia` al whitelist de `order.payment_method`; ledger/registro de ingreso
  electrónico por venta (o reutilizar la ruta de `tarjeta` si ya modela no-efectivo).
  Endpoint POS acepta el tender. Invariante de arqueo: la venta por transferencia NO entra
  al efectivo esperado del `cash_register_session`.
- **Reporte por método de pago**: ventas del día/período desglosadas efectivo / tarjeta /
  transferencia / fiado. Deepen el reporte que el agente lee.
- **Agente** (`crates/assist`, intents por regla, límite de palabra como el matcher IVA):
  `IngresosPorMetodo` → *"¿cuánto me entró por transferencia hoy?"*, *"efectivo vs
  transferencia"*. Texto de ayuda del agente actualizado.
- **Cliente** (Tauri + shim web): botón **Transferencia** en el POS (móvil 1-toque),
  estados empty/error es-CL. Comando Tauri + handler del shim web (funciona igual en PWA).
- **GATE (DoD)**: `RUSTC_WRAPPER= CARGO_INCREMENTAL=0 cargo test -p api -p domain` con test
  nuevo del invariante de arqueo (transferencia NO toca efectivo) + persistencia del tender
  (guardián del whitelist) + `e2e_concurrency_fefo` VERDE (no regresión del hot-path) +
  cliente `npm run test` (tsc+vitest) + **smoke en vivo**: venta por transferencia →
  arqueo de caja NO la incluye → reporte "por transferencia hoy" cuadra → efectivo del
  cierre = solo ventas en efectivo.

### Slice 2 — Conciliación (pendiente/confirmado)
- Estado del ingreso electrónico: `pendiente` (dijeron "te transfiero", aún no confirma que
  llegó) vs `confirmado` (lo vio caer). Confirmación MANUAL (sin API de banco en tier gratis).
- Agente: acción `confirmar_transferencia` + read `TransferenciasPendientes`.
- Cliente: vista/lista de ingresos electrónicos con toggle confirmar (o plegado en caja/
  reportes). GATE + smoke: marcar confirmado, pendientes listadas correctas.

### Slice 3 — Apps + venta partida
- Sub-método de app: MercadoPago / MACH / Sumup / Getnet como sabor de tender electrónico.
- **Venta partida** (multi-tender por venta): $ efectivo + $ transferencia en una venta.
  Requiere que la orden guarde una LISTA de pagos (no un solo `payment_method`) — cambio de
  esquema mayor, por eso va al final. GATE + smoke: split cuadra, arqueo solo suma la parte
  efectivo, reporte por método reparte bien.

## Fuera de alcance (explícito, para no dejar módulo a medias por sobre-alcance)
- Integración con API de banco / open banking (no existe en Chile para micronegocio gratis;
  la conciliación es manual — es lo correcto para el tier gratis).
- Apps como pasarela de cobro real (link de pago) — eso es otro vector futuro; aquí las apps
  son solo una etiqueta de método de ingreso.

## Coordinación
- Un solo cargo pesado en la PC (RUSTC_WRAPPER= CARGO_INCREMENTAL=0). Constructor construye
  en su worktree; Consolidador integra con el ritual (GATE + smoke en vivo) y actualiza
  bitácora ESTADO ACTUAL.
- Antes de codear, el Constructor confirma en el código cómo se maneja hoy `tarjeta`
  (no-efectivo, arqueo) y espeja esa ruta — reduce riesgo a casi cero.
