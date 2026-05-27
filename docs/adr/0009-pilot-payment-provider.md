# ADR-0009: Pilot payment provider — Mercado Pago + Stripe antes de Webpay

- **Status**: Accepted (amends ADR-0003 *priority order* for pilot phase)
- **Date**: 2026-05-27
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: pagos, fase-11, presupuesto, pilot
- **Amends**: [ADR-0003](./0003-payments-webpay-first.md) (priority order during pilot only;
  Webpay sigue siendo target final para escala)

## Context and Problem Statement

[ADR-0003](./0003-payments-webpay-first.md) decidió **Webpay primario + Stripe secundario**
en rollout staged (Fase 11.0 sólo Webpay → Fase 11.1 Stripe → Fase 11.2 Khipu → Fase 11.3
Mercado Pago). Esa decisión asume que el **primer cobro** ocurre en Fase 11.0 con Webpay
ya integrado.

El fundador pidió **camino $0 hasta primer cobro** (mensaje 2026-05-27). Webpay tiene:

1. **Onboarding ~2-4 semanas** KYC + certificación Transbank.
2. **Requiere RUT empresa** (persona jurídica). Pilot phase usa RUT persona natural del
   fundador.
3. **Contrato con Transbank** (fee fijo + transaccional).
4. **Certificación de comercio** antes de mover dinero real.

Esto **no es gratis** ni **inmediato**. Bloquea primer cobro mínimo 2-4 semanas + papeleo
empresa.

Alternativas con onboarding <1 día + RUT persona natural OK:

| Provider | Onboarding | RUT | Costo fijo | Costo tx | Recurring |
|---|---|---|---|---|---|
| **Mercado Pago Chile** | <24h self-service | Persona natural OK | $0 | 2.99% + IVA | Preapproval API |
| **Stripe** | <24h self-service | Persona natural con cuenta US/internacional | $0 | 3.6% + $0.30 USD | Subscriptions |
| **Webpay Transbank** | 2-4 sem | Persona jurídica obligatoria | Contrato | ~2.99% | Oneclick Mall |
| **Khipu** | <1 sem | Persona natural OK pero pensado B2B | $0 | ~1% (transferencia) | Sin recurring nativo |

Hipótesis post-pivote freemium (ver [`freemium-master-plan.md`](../strategy/freemium-master-plan.md)
§5): primer ingreso probable = **microtransacción one-time** (branding pack, SII unlock)
del primer piloto, NO suscripción Pro. Para microtx no se necesita Oneclick — basta
Checkout one-shot.

## Decision Drivers

- **Tiempo a primer cobro**: minimizar.
- **Costo upfront $0** (ver [ADR-0008](./0008-self-sign-pilot-msi.md) coherente).
- **RUT persona natural OK** (fundador antes de constituir SpA).
- **Compatible con Fase 11 staged target**: Webpay sigue siendo el objetivo de escala
  según ADR-0003. Este ADR sólo cambia el *orden de integración* en pilot.
- **Confianza percibida en CL**: Mercado Pago tiene brand awareness B2C aceptable en CL;
  Stripe es desconocido B2C-CL pero familiar al admin/dueño técnico.

## Considered Options

1. **Mantener orden ADR-0003** — esperar 2-4 semanas Webpay antes de cobrar.
2. **Stripe primero + Webpay después** — más rápido que Webpay pero requiere cuenta US o
   integración Atlas (~$500). No-cero costo.
3. **Mercado Pago primero + Stripe segundo + Webpay tercero** (pilot order) — onboarding
   <24h, RUT persona natural OK, $0 setup, cobertura CL.
4. **Khipu como bridge** — sólo transferencia bancaria. Sin recurring. Útil pero
   limitado.

## Decision Outcome

**Elegida: Opción 3 (Mercado Pago Chile primero + Stripe segundo + Webpay tercero)**
para **pilot phase únicamente**.

### Orden de integración revisado (pilot)

| Fase | Provider | Casos | Estado |
|---|---|---|---|
| **11.0a (pilot)** | **Mercado Pago Chile** | Microtx + suscripción Pro/Business CL | Nuevo (este ADR) |
| **11.0b (pilot)** | **Stripe Checkout** | Microtx tarjeta internacional, dueño con tarjeta personal | Adelantado vs ADR-0003 |
| **11.1 (escala)** | **Webpay Transbank** | Suscripción CL B2B con brand awareness máximo | Diferido hasta SpA constituida (era 11.0 en ADR-0003) |
| **11.2 (escala)** | **Khipu** | Enterprise transferencia bancaria | Sin cambio vs ADR-0003 |
| **11.3+** | **Mercado Pago multi-país** | Expansión LATAM (ARG/MEX) | Sin cambio vs ADR-0003 |

### Architecture compatible

El license-server (repo separado `pharma-license-server`, ver
[`docs/strategy/license-server-skeleton.md`](../strategy/license-server-skeleton.md))
implementa un `PaymentProvider` trait:

```typescript
interface PaymentProvider {
  createCheckout(tier: Tier | Microtx, customerEmail: string): Promise<CheckoutSession>;
  handleWebhook(payload: unknown, signature: string): Promise<PaymentEvent>;
}
```

Implementaciones: `MercadoPagoProvider`, `StripeProvider`, `WebpayProvider` (TODO),
`KhipuProvider` (TODO). Habilitadas por env var `PROVIDERS_ENABLED=mercadopago,stripe`.

Esto garantiza que **agregar Webpay después no exige refactor** — sólo implementar
`WebpayProvider` y habilitar en env. El trait queda en pilot.

### Consequences

#### Positivas

- **Primer cobro posible en días** (no semanas).
- **Cero costo upfront** (MP fee per-tx, no contrato fijo).
- **RUT persona natural** del fundador suficiente.
- **Stripe abre microtx internacional** desde día 1 (dueños con tarjeta extranjera).
- **Compatible con ADR-0003** — Webpay no se descarta, sólo se difiere.

#### Negativas

- **MP en CL B2B** tiene brand awareness menor que Webpay → percepción "menos formal"
  para clientes empresa grandes. Mitigante: el target pilot son farmacias independientes
  (no cadenas), donde el dueño-operador prioriza velocidad sobre marca de gateway.
- **Comisiones MP** (2.99% + IVA) ~igual a Webpay pero sin descuentos por volumen.
- **Sin Oneclick equivalente real** en MP — Preapproval (subscription) existe pero es
  menos pulido. Mitigante: pilot phase puede cobrar sub mes-a-mes con email reminder
  hasta cierta escala.
- **Fragmentación post-escala** — al activar Webpay (11.1), el license-server queda con
  3 providers simultáneos. Manejable con el `PaymentProvider` trait y feature flags.

#### Neutras

- Cuando el fundador constituya SpA, **Webpay se activa según ADR-0003 original** sin
  refactor (trait + env var).
- Stripe en Chile **NO** requiere Stripe Atlas si el fundador tiene cuenta bancaria US o
  acepta payouts vía Wise/Payoneer. Caso contrario, MP queda primario y Stripe se
  activa cuando hay cuenta US.

## Pros and Cons of the Options

### Opción 1: Mantener ADR-0003 (Webpay primero)
- **Pros**: cumple ADR-0003 al pie de la letra. Brand awareness máximo.
- **Cons**: bloquea primer cobro 2-4 sem + RUT empresa. Incompatible con camino $0 + velocidad.

### Opción 2: Stripe primero
- **Pros**: onboarding rápido, infra robusta, Subscriptions maduras.
- **Cons**: cuenta bancaria US (o Atlas $500) → no es $0. Brand awareness B2B-CL bajo.

### Opción 3: MP + Stripe + Webpay después (elegida)
- **Pros**: ver "Decision Outcome > Positivas".
- **Cons**: ver "Decision Outcome > Negativas".

### Opción 4: Khipu como bridge
- **Pros**: $0 onboarding, RUT persona natural, comisión baja (~1%).
- **Cons**: sólo transferencia bancaria (sin tarjeta) → microtx friction alta. Sin
  recurring nativo. Útil como complemento Enterprise, no como primer rail.

## More Information

- [ADR-0003](./0003-payments-webpay-first.md) — decisión original (sigue siendo target
  final).
- [ADR-0008](./0008-self-sign-pilot-msi.md) — paralelo: cert MSI para pilot.
- [`docs/strategy/zero-cost-launch-plan.md`](../strategy/zero-cost-launch-plan.md) §4 — secuencia día-a-día.
- [`docs/strategy/license-server-skeleton.md`](../strategy/license-server-skeleton.md) — `PaymentProvider` trait + webhook flow.
- [`docs/strategy/payments-cl.md`](../strategy/payments-cl.md) — comparativa providers (existente).
- Mercado Pago docs: https://www.mercadopago.cl/developers
- Stripe Checkout docs: https://stripe.com/docs/payments/checkout
