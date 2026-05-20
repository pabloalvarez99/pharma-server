# ADR-0005: Invariantes del core gratis (sin paywall a export, sin kill-switch)

- **Status**: Accepted
- **Date**: 2026-05-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, compliance

## Context and Problem Statement

El modelo freemium ([ADR-0001](./0001-freemium-pivot.md)) introduce un riesgo cultural y
ético: la tentación de mover features del Free a tiers pagos cuando los KPIs presionan
("upgrade or lose access"). Sin invariantes explícitos, el producto se degrada en el
tiempo hacia patrones extractivos típicos de SaaS comoditizado.

Este ADR codifica los **principios no-negociables** que protegen al usuario del Free y
sostienen la tesis de pharma-server como "ERP que respeta a su dueño".

## Decision Drivers

- Reputación del producto en una comunidad pequeña (CL farmacéutico).
- Coherencia con el pitch ("vendor-agnostic, vendor-respect").
- Compliance Ley 19.628 + futura normativa de datos CL.
- Reglas que el fundador puede invocar para vetar regresiones internas.

## Considered Options

1. **Sin invariantes explícitos** — cada decisión a futuro se evalúa caso por caso.
2. **Invariantes documentados pero blandos** — pueden cambiarse por mayoría de equipo.
3. **Invariantes hard-locked en ADR + CLAUDE.md** — sólo cambiables por nuevo ADR
   explícito + OK escrito del fundador.

## Decision Outcome

**Elegida: Opción 3 (invariantes hard-locked)**. Los siguientes principios son
inmutables sin ADR-superseder explícito y consentimiento escrito del fundador:

### Invariante 1: Core ERP siempre gratis offline
Funcionalidades garantizadas en el tier Free, para siempre:
- POS completo (ventas + devoluciones + idempotencia + loyalty)
- Inventario (SKU + lote + vencimiento + stock por sucursal)
- Caja (apertura/cierre/arqueo)
- Gastos
- Recetas (incluyendo manejo manual Ley 20.000 + export ISP)
- Backup local on-demand + scheduled
- Reporte `sales-daily`
- Multi-usuario LAN (cajeros + admin + dueño)
- Auditoría completa

**No se mueven a tiers pagos.** Sólo se *agregan* capacidades nuevas al Free
(siempre additive, nunca subtractive).

### Invariante 2: License OFFLINE-FIRST
El server NO requiere internet para operar features ya activadas. Validación 100% local
con clave pública del licenser embebida en el binario. Internet sólo para:
(a) compra/upgrade,
(b) refresh opcional de revocation list,
(c) telemetría opt-in.

### Invariante 3: Telemetría OPT-IN siempre, nunca opt-out, nunca por defecto
- Toggle en setup wizard. Default = OFF.
- Granularidad usuario-controlada (errores / uso / performance independientes).
- Cero PII bajo ninguna circunstancia.
- IDs anonimizados con `tenant_id`-derived hash + salt rotativo mensual.
- Cumple Ley 19.628 y futuro reglamento de datos personales CL.
- Endpoint `pharma telemetry status` muestra qué se reportó en última hora.

### Invariante 4: Sin lock-in de datos
Tier Free incluye **export completo CSV/JSON de TODAS las tablas** (productos,
inventario, ventas, devoluciones, caja, gastos, recetas, audit log).
Jamás se cobra por exportar la propia data del cliente.
Comando: `pharma export --all --output /path`.

### Invariante 5: Sin dark patterns
- Máximo **1 upgrade prompt por sesión** (toast no-modal).
- **Cero prompts durante el POS hot path** (vender es sagrado).
- Sin "free trial que cobra al expirar sin avisar". Tiers pagos NUNCA se activan sin
  cobro explícito y consentimiento.
- Sin "fake discount" anclado a precio inventado.

### Invariante 6: Sin kill-switch remoto
El binario NUNCA se desactiva remotamente. Si el license expira o es revocado:
- Features pagadas → 402 Payment Required.
- Core gratis sigue operativo. **Para siempre.**

### Invariante 7: Compromiso de continuidad
Si en algún punto pharma-server deja de mantenerse comercialmente:
- El binario más reciente seguirá funcionando offline indefinidamente (no expira).
- Pubkey del licenser NO se desactiva — licenses siguen siendo válidas.
- Se publica una "last release" con `tier` upgraded a Pro/Business para todos los
  usuarios pagos activos al momento del cierre.
- Schema de export queda documentado públicamente.

### Consequences

#### Positivas
- Reputación de producto íntegro. Word-of-mouth positivo en comunidad farmacéutica CL.
- Reduce ansiedad del usuario ("¿qué pasa si la compañía cierra?").
- Diferencia claramente de SaaS extractivos.
- Defendible legalmente: compromiso público con Ley 19.628.

#### Negativas
- Limita opciones de monetización agresiva si el negocio presiona en el futuro.
- Costo de soporte gratis no recortable (mitigado por comunidad/docs).
- Renunciamos a estrategia "free → trial → forced upgrade".

#### Neutras
- Estos invariantes son **publicables**. Forman parte del pitch externo.

## Pros and Cons of the Options

### Opción 1: Sin invariantes
- **Pros**: máxima flexibilidad táctica.
- **Cons**: drift inevitable hacia patrones extractivos. Pérdida de carácter del producto.

### Opción 2: Invariantes blandos
- **Pros**: revisables.
- **Cons**: lo que se puede revisar fácilmente se revisa cuando los KPIs presionan.
  Equivalente a no tener invariantes.

### Opción 3: Invariantes hard-locked (elegida)
- **Pros**: ver decisión.
- **Cons**: ver consecuencias negativas.

## More Information

- [`docs/strategy/freemium-master-plan.md`](../strategy/freemium-master-plan.md) §6 — invariantes en plan maestro.
- [ADR-0001](./0001-freemium-pivot.md) — pivote a freemium.
- Ley 19.628 (Protección de Vida Privada Chile): https://www.bcn.cl/leychile/navegar?idNorma=141599
