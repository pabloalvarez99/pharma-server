# ADR-0001: Pivote a modelo freemium MSI

- **Status**: Accepted
- **Date**: 2026-05-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, modelo-negocio

## Context and Problem Statement

pharma-server hoy (v0.1.23) está concebido como **ERP on-prem vendible por licencia única**:
una farmacia paga un cheque upfront, recibe el MSI, lo opera para siempre. La validación
con prospectos en Coquimbo/La Serena reveló que el modelo upfront genera fricción
desproporcionada al valor percibido inicial: farmacias independientes piden ver el
producto operativo en su flujo real durante semanas antes de firmar un cheque ≥ CLP $300k.

Paralelamente, el repo ya construyó (Fases 11) el activo diferencial estructural:
**protocolo de comercio federado firmado** (`crates/agent/*`, `migrations/0008-0014`,
endpoints `/agent/inbox`, `agent_orders`). Ese activo sólo gana valor con masa crítica
de nodos instalados — una premisa imposible con un funnel de venta upfront cara.

## Decision Drivers

- **Velocidad de adopción** > margen unitario inmediato.
- **Network effects** (Fase 13 marketplace) requieren miles de instalaciones, no docenas.
- **Validación rápida de hipótesis de producto** con farmacias reales.
- **Defensibilidad estructural**: protocolo + reputación distribuida > feature parity.
- **Compliance con invariantes existentes**: offline-first, no telemetría obligatoria,
  sin lock-in.

## Considered Options

1. **Mantener licencia única upfront** — modelo original. Status quo.
2. **Trial limitado de 30 días + licencia única al expirar** — modelo SaaS-lite.
3. **Freemium con core gratis + tiers pagos + microtransacciones** — modelo estilo
   *League of Legends* / Notion / Figma.
4. **SaaS puro hosted en cloud** — abandona on-prem.

## Decision Outcome

**Elegida: Opción 3 (Freemium con core gratis + tiers + microtx)**, porque maximiza
adopción sin sacrificar la tesis on-prem y crea funnel natural hacia tiers pagos con
fricción mínima.

### Consequences

#### Positivas
- Eliminación total de friction de instalación para usuarios nuevos.
- Funnel de conversión medible (Free → Pro/Business → Enterprise + microtx).
- Más datos de uso real (vía telemetría opt-in) para priorizar roadmap.
- Network effect path claro para Fase 13.
- Permite microtx low-friction tipo "branding pack" como vector de monetización
  adicional.

#### Negativas
- Costo de soporte gratis aumenta (mitigado por soporte = comunidad/docs en tier Free).
- Necesidad de construir license-server + integración pagos (Fase 11) antes de cobrar.
- Riesgo de canibalización Free → Pro si la tier matrix queda mal calibrada
  (mitigación: iteración con design partners).
- Complejidad ingenierítica adicional: `crates/license`, feature gates, web admin,
  CRL distribution. Documentado en
  [`license-architecture.md`](../strategy/license-architecture.md) y
  [`scaling-architecture.md`](../strategy/scaling-architecture.md).

#### Neutras
- Pricing real lo cierra el fundador con design partners (no decidido en este ADR).
- El modelo aún permite contratos Enterprise tradicionales.

## Pros and Cons of the Options

### Opción 1: Licencia única
- **Pros**: simple de comunicar, alto ARPU por venta, sin infra SaaS.
- **Cons**: adopción limitada por cheque upfront, no genera network effects, validación de hipótesis lenta.

### Opción 2: Trial 30 días → licencia
- **Pros**: prueba antes de comprar.
- **Cons**: misma fricción de cheque al final del trial. Crea ansiedad ("se me vence"). No abre el funnel de microtx.

### Opción 3: Freemium + tiers + microtx
- **Pros**: ver "Consequences > Positivas".
- **Cons**: ver "Consequences > Negativas".

### Opción 4: SaaS hosted puro
- **Pros**: control total del runtime, telemetría sin opt-in, billing centralizado.
- **Cons**: **viola la tesis del producto** (offline-first, datos en la farmacia, sin
  internet). Inaceptable.

## More Information

- [`docs/strategy/freemium-master-plan.md`](../strategy/freemium-master-plan.md) — plan completo del modelo elegido.
- [ADR-0005](./0005-core-gratis-no-locked-in.md) — invariantes del core gratis (derivados de esta decisión).
- Fase 11 del roadmap (`bitacora.md`).
