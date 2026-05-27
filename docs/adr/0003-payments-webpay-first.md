# ADR-0003: Webpay como rail de pago primario para Chile

- **Status**: Accepted
- **Date**: 2026-05-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: pagos

## Context and Problem Statement

El target inicial de comercialización es **farmacias independientes en Chile** (Coquimbo,
La Serena, expansión nacional). El cliente B2B chileno tiene preferencias fuertes y poco
flexibles en rails de pago:

1. Tarjetas de débito/crédito CL → **Webpay (Transbank)** es estándar de facto y genera
   más confianza que alternativas.
2. Cobros recurrentes (suscripción Pro/Business) requieren **mandato persistente**
   (Oneclick Mall en Webpay).
3. Tarjetas internacionales (Stripe) son inusuales en flujo B2B CL — el cliente puede
   percibir el cobro en USD como sospechoso.

Por otro lado, las **microtransacciones one-time** desde web admin (branding pack, SII
unlock) pueden tener compradores con perfil distinto (dueño/admin del local, dispuesto a
pagar con tarjeta personal o internacional).

## Decision Drivers

- Confianza percibida por el cliente B2B CL.
- Soporte de suscripción recurrente.
- Onboarding técnico (tiempo a primer cobro).
- Compliance CL (IVA, boleta SII).
- Fees.
- Multi-país futuro.

## Considered Options

1. **Stripe único** — rápido de setup, internacional.
2. **Mercado Pago único** — onboarding fácil, multi-país LATAM.
3. **Webpay primario + Stripe secundario** — Webpay para suscripción y microtx CL,
   Stripe para microtx con tarjeta internacional.
4. **Webpay + Mercado Pago + Khipu** — cobertura completa CL desde día 1.

## Decision Outcome

**Elegida: Opción 3 (Webpay primario + Stripe secundario)**, en **rollout staged**:

- **Fase 11.0** (MVP cobros): **sólo Webpay** (Oneclick para sub, redirect para microtx).
- **Fase 11.1**: agregar **Stripe** para microtx con tarjeta internacional (web admin).
- **Fase 11.2**: agregar **Khipu** para Enterprise (transferencia bancaria, contrato anual).
- **Fase 11.3+**: agregar **Mercado Pago** cuando expansión a Argentina/México justifique.

### Consequences

#### Positivas
- Aceptación máxima en mercado primario (CL).
- Onboarding técnico contenido (un solo rail al inicio).
- Confianza del cliente (Webpay = "es serio").
- Cobertura B2B sin tiers raros.

#### Negativas
- Webpay tiene onboarding pesado (~2-4 semanas KYC + certificación).
- Sin Stripe inicial → no se monetiza dueños fuera de CL hasta Fase 11.1.
- Provider DTE (boleta SII electrónica) requiere decisión separada
  (resuelta en [ADR-0011](./0011-dte-provider-native-rust.md); el slot 0008 ahora es
  self-sign cert MSI).
- **Orden de providers diferido en pilot**: ver [ADR-0009](./0009-pilot-payment-provider.md)
  — Mercado Pago + Stripe van primero en pilot phase (Webpay requiere RUT empresa +
  onboarding 2-4 sem). Webpay sigue siendo el target de escala de este ADR.

#### Neutras
- Fees ~3% son competitivos en CL.
- El diseño del license-server queda rail-agnóstico (ver
  [`scaling-architecture.md`](../strategy/scaling-architecture.md) §5).

## Pros and Cons of the Options

### Opción 1: Stripe único
- **Pros**: setup rápido, infra global, webhooks robustos.
- **Cons**: cliente B2B CL desconfía del cobro en USD. **No es estándar local**.

### Opción 2: Mercado Pago único
- **Pros**: self-service, multi-país.
- **Cons**: en CL específicamente, brand awareness inferior a Webpay. Suscripciones
  (Preapproval) son menos comunes que Oneclick.

### Opción 3: Webpay + Stripe staged (elegida)
- **Pros**: ver decisión.
- **Cons**: dos integraciones eventualmente. Manejable con abstracción `PaymentRail`
  trait.

### Opción 4: Webpay + MP + Khipu desde día 1
- **Pros**: cobertura completa.
- **Cons**: complejidad ingenierítica × 3 al MVP. Retrasa fecha de primer cobro.
  Violenta principio de iteración rápida.

## More Information

- [`docs/strategy/payments-cl.md`](../strategy/payments-cl.md) — comparativa completa.
- [`docs/strategy/freemium-master-plan.md`](../strategy/freemium-master-plan.md) §5 — política de cobros.
- Transbank docs: https://www.transbankdevelopers.cl/
- Stripe docs: https://stripe.com/docs
