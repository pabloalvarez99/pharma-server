# ADR-0010: Roadmap Fase 9.x — paridad mínima vendible vs competencia CL

- **Status**: Accepted
- **Date**: 2026-05-21
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, roadmap, competidores

## Context and Problem Statement

MSI v0.1.24 (Fase 9 cierre técnico) es **instalable, offline-first, license-gated**, pero el catálogo de módulos no alcanza paridad mínima vs competidores serios CL (SICO, GOLAN, t-Farmacias, iFarmacias, Bsale). Sin paridad mínima en POS/DTE/multi-caja/Webpay, el producto NO es vendible a una farmacia operativa real — solo a hobbyists o evaluadores técnicos.

Análisis competitivo completo: ver [`docs/strategy/competitor-parity-analysis.md`](../strategy/competitor-parity-analysis.md).

## Decision Drivers

- **Tabla stake**: features que ninguna farmacia operacional puede operar sin (DTEs SII + multi-caja + POS Webpay).
- **Diferenciadores que tenemos** (offline real, MSI 1-click, no lock-in, Ed25519 license offline, freemium permanente, Fase 13 federación): mantener intactos.
- **Costo/beneficio**: priorizar features que desbloquean adopción inmediata y conversión Free→Pro, no features nice-to-have.
- **Compromiso freemium master plan ([ADR-0005](./0005-core-gratis-no-locked-in.md))**: core gratis sigue siendo invariante, las features nuevas se distribuyen entre Free (lo que toda farmacia espera) y Pro/Business/microtx (lo que diferencia).

## Considered Options

1. **Ship MSI v0.1.24 ya + venderlo sin más features** — rápido al mercado, pero rechazo seguro de prospects que ven que no hay boleta SII automática.
2. **Pivotar 100% a features paid antes de MSI ship** — desfasa ship indefinidamente.
3. **Ship MSI v0.1.24 + roadmap Fase 9.x ordenada por impacto vendible** — secuencia 9.1→9.5 (8-10 semanas) llega a paridad mínima vendible; 9.6→9.14 expanden microtx upsell.

## Decision Outcome

**Elegida: Opción 3** — Ship MSI v0.1.24 inmediato como demo/early-access + ejecutar Fase 9.x secuenciada.

### Secuencia Fase 9.x

| Fase | Feature | Tier monetización | Esfuerzo aprox | Status |
|---|---|---|---|---|
| 9.0 | MSI v0.1.24 ship-ready (Start Menu + auto-launch dashboard) | — | hecho | ✅ esta sesión |
| 9.1 | DTEs completos (boleta + factura + NC + ND + GD + reportes X/Z) | Free básico + Pro avanzado | 2-3 sem | pending |
| 9.2 | Multi-caja apertura/cierre/arqueo | Free 1, Pro 3, Business 10 | 1 sem | pending |
| 9.3 | Multi-bodega + transfers inter-bodega | Pro | 1-2 sem | pending |
| 9.4 | Max/min stock + auto-PO generation | Pro | 1 sem | pending |
| 9.5 | Webpay POS integration (cobro físico) | Pro/Business + microtx unlock | 1 sem | pending |
| 9.6 | Etiquetas ZPL impresora Zebra | Free | 3 días | pending |
| 9.7 | ISAPRE convenios (varios pagadores) | Pro/Business | 2-3 sem | pending |
| 9.8 | Recetario magistral cálculo por PA | Microtx unlock | 1-2 sem | pending |
| 9.9 | Alternativa terapéutica auto-sugerida | Microtx unlock | 1 sem | pending |
| 9.10 | Promo kits / combos / bundles | Free | 3-5 días | pending |
| 9.11 | YAPP / ChileSalud reembolsos | Microtx | 1-2 sem | pending |
| 9.12 | Fractionamiento dispensación (medio blister, etc.) | Microtx (segmento comunal) | 1 sem | pending |
| 9.13 | RAYEN integration (sistema clínico APS MINSAL) | Microtx (municipal) | 2-3 sem | pending |
| 9.14 | POS móvil web (boleta electrónica desde celular) | Pro | 2 sem | pending |

**Paridad mínima vendible** se alcanza con 9.0→9.5 (≈ 8-10 semanas dev).
**Diferenciación microtx + segmentos especializados** con 9.6→9.14 (≈ 12-16 semanas adicionales).

### Consequences

#### Positivas
- Roadmap defendible frente a evaluación cliente real ("¿tienes boleta SII?" → sí en Fase 9.1).
- Funnel Free → Pro/Business + microtx con valor concreto, no marketing vacío.
- Mantiene diferenciadores estructurales sin sacrificar paridad operacional.
- Permite captar farmacia popular/comunal (9.12-9.13) sin abandonar farmacia independiente premium (9.5, 9.7).

#### Negativas
- Postpone Fase 12 (sync online opt-in) y Fase 13 (marketplace B2B) al menos 6 meses.
- Requiere validación adicional: priorización 9.7 (ISAPRE) vs 9.13 (RAYEN) depende de qué segmento adopta primero, sin datos hoy.
- Riesgo de scope creep — cada feature de Fase 9.x puede engordar (ej. ISAPRE convenios "completo" = 6+ pagadores con APIs distintas).

### Reglas de ejecución

1. Cada fase 9.x = una rama, un PR, una entrada bitácora dual.
2. Cada fase incorpora tests integration + smoke MSI nuevo + bump version semver minor.
3. Antes de empezar 9.x+1: validar 9.x con al menos 1 farmacia real (mismo Coquimbo o virtual).
4. Telemetría opt-in mide qué features se usan post-instalación → re-prioriza siguiente fase si los datos contradicen el orden actual.
5. **Microtx 9.8/9.9/9.11/9.12/9.13 NO bloquean Free tier** — siguen siendo opcionales, no degradan experiencia base.

## Links

- [ADR-0001: Pivote a freemium](./0001-freemium-pivot.md)
- [ADR-0005: Core gratis no lock-in](./0005-core-gratis-no-locked-in.md)
- [Freemium master plan](../strategy/freemium-master-plan.md)
- [Competitor parity analysis](../strategy/competitor-parity-analysis.md)
