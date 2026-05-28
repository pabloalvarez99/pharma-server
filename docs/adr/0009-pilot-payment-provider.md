# ADR-0009: Rail de cobro pilot — Mercado Pago como primer rail LIVE (Webpay ya está en sandbox)

- **Status**: Accepted (amends ADR-0003 *go-live order* for pilot phase)
- **Date**: 2026-05-27
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: pagos, fase-11, presupuesto, pilot
- **Amends**: [ADR-0003](./0003-payments-webpay-first.md) (orden de *go-live* en pilot; Webpay
  sigue siendo el rail de confianza para escala CL).

> **Nota de numeración**: este es el ADR-0009 de **pharma-server** (rail de cobro pilot).
> El repo `pharma-license-server` tiene su PROPIO `docs/adr/0009-admin-auth.md` (NextAuth),
> que es un documento distinto. Citar siempre con prefijo de repo.

## Context and Problem Statement

[ADR-0003](./0003-payments-webpay-first.md) decidió **Webpay primario** en rollout staged
(Fase 11.0 Webpay → 11.1 Stripe → 11.2 Khipu → 11.3 Mercado Pago).

**Estado real verificado 2026-05-27** (repo `pharma-license-server`, branch
`feat/webpay-checkout-fase-11b`, code-complete):

- **Webpay YA está implementado en sandbox** (`src/lib/webpay.ts` con `transbank-sdk`
  6.1.1, checkout start/return idempotente, emisión de license post-pago). Falta sólo el
  deploy + el switch a producción.
- El schema Prisma `Order` ya tiene **ambos** campos `webpayToken` y `stripeSessionId`
  (Stripe anticipado).

Por lo tanto el blocker para **cobrar dinero real** NO es escribir código de pagos — es que
**Webpay producción** requiere:

1. **RUT empresa** (persona jurídica). El fundador opera hoy como persona natural (SpA
   no constituida aún).
2. **Certificación de comercio** Transbank (~2-4 semanas).

El fundador pidió **camino $0 + velocidad a primer cobro** (mensaje 2026-05-27). Esperar a
constituir SpA + certificar Transbank retrasa el primer ingreso semanas, por papeleo, no
por ingeniería.

Rails que permiten cobrar **dinero real** sin SpA y con onboarding <1 día:

| Provider | Onboarding | RUT | Costo fijo | Costo tx | Código en repo |
|---|---|---|---|---|---|
| **Mercado Pago CL** | <24h self-service | **Persona natural OK** | $0 | 2.99% + IVA | ❌ falta (F11c') |
| **Stripe** | <24h pero requiere banca US/Atlas | Persona natural + cuenta US | $0 setup | 3.6% + $0.30 USD | ❌ falta (F11c), schema listo |
| **Webpay prod** | 2-4 sem + cert | **Persona jurídica obligatoria** | contrato | ~2.99% | ✅ sandbox completo |
| **Khipu** | <1 sem | Persona natural OK | $0 | ~1% transferencia | ❌ falta |

Hipótesis post-pivote freemium ([`freemium-master-plan.md`](../strategy/freemium-master-plan.md)
§5): primer ingreso probable = **microtransacción one-time** (branding pack, SII unlock),
no suscripción. Microtx = pago one-shot, no requiere mandato recurrente.

## Decision Drivers

- **Tiempo a primer cobro real**: minimizar (sin esperar SpA).
- **Costo upfront $0** (coherente con [ADR-0008](./0008-self-sign-pilot-msi.md)).
- **RUT persona natural OK** mientras no haya SpA.
- **No botar el código Webpay** ya escrito (es activo, no deuda).
- **Compatible con ADR-0003**: Webpay sigue siendo el rail de confianza CL para escala.

## Considered Options

1. **Esperar Webpay producción** (terminar el deploy + constituir SpA + certificar) — 0
   código nuevo, pero bloquea primer cobro 2-4 sem + papeleo empresa.
2. **Stripe primero** — schema listo, pero requiere banca US o Atlas (~$500) → no es $0 en CL.
3. **Mercado Pago como primer rail LIVE** — persona natural CL OK, $0, <24h. Requiere ~1
   día de código (provider nuevo) reusando el patrón checkout existente.
4. **Khipu** — $0 + persona natural, pero sólo transferencia bancaria (microtx friction
   alta, sin tarjeta).

## Decision Outcome

**Elegida: Opción 3 (Mercado Pago como primer rail LIVE de pilot)**, conservando todo el
código Webpay existente para activarlo al constituir SpA.

### Orden de go-live revisado (pilot)

| Orden | Provider | Estado código | Bloqueo a go-live | Cuándo |
|---|---|---|---|---|
| **1º LIVE** | **Mercado Pago CL** | ❌ por escribir (~1 día, reusa checkout pattern) | Ninguno ($0, persona natural) | Pilot — primer cobro real |
| **2º LIVE** | **Webpay** | ✅ sandbox completo | RUT empresa + cert Transbank (2-4 sem) | Al constituir SpA |
| **3º LIVE** | **Stripe** | schema listo, lógica por escribir | Banca US / Wise / Payoneer | Microtx internacional |
| **4º** | **Khipu** | ❌ | — | Enterprise transferencia (ADR-0003) |

### Architecture

El license-server ya tiene un patrón de checkout (`/api/checkout/start` + `/return`) y
`Order` con campos por-provider (`webpayToken`, `stripeSessionId`). Agregar Mercado Pago =
añadir `Order.mercadopagoPaymentId` + un `src/lib/mercadopago.ts` análogo a `webpay.ts` +
rutas `/api/checkout/mp/{start,return}` o webhook. **No exige refactor** del flujo de
emisión de license (issuance es provider-agnóstico: recibe un Order confirmado → firma →
persiste → entrega vía descarga en `/checkout/success` + `GET /api/licenses/[id]`).

### Consequences

#### Positivas

- **Primer cobro real en días** (no semanas), con RUT persona natural, $0 upfront.
- **El código Webpay NO se bota** — queda listo para activar (`WEBPAY_INTEGRATION_TYPE=
  PRODUCTION` + creds) cuando haya SpA.
- **Compatible con ADR-0003** — Webpay sigue siendo el rail de confianza para escala CL.
- **Microtx-first** encaja: MP Checkout one-shot cubre el primer caso de ingreso probable.

#### Negativas

- **~1 día de código nuevo** (provider MP) — no es "cero ingeniería" como terminar Webpay,
  pero desbloquea ingresos semanas antes.
- **MP brand awareness B2B-CL** < Webpay → percepción "menos formal" para cadenas grandes.
  Mitigante: target pilot = farmacias independientes (dueño-operador prioriza velocidad).
- **Fragmentación** — al activar Webpay (SpA) habrá 2+ providers live. Manejable: issuance
  ya es provider-agnóstico.

#### Neutras

- Stripe queda 3º (no 2º como en ADR-0003 original) porque su blocker en CL (banca US) es
  mayor que el de MP (ninguno).
- Cuando el fundador constituya SpA, **Webpay se vuelve el rail primario** per ADR-0003.

## Pros and Cons of the Options

### Opción 1: Esperar Webpay producción
- **Pros**: 0 código nuevo (sandbox ya hecho). Brand máximo CL.
- **Cons**: bloquea primer cobro 2-4 sem + RUT empresa. Incompatible con $0+velocidad.

### Opción 2: Stripe primero
- **Pros**: schema listo, Subscriptions maduras.
- **Cons**: banca US o Atlas $500 → no $0 en CL. Brand B2B-CL bajo.

### Opción 3: Mercado Pago primer rail LIVE (elegida)
- **Pros**: ver "Decision Outcome > Positivas".
- **Cons**: ver "Decision Outcome > Negativas".

### Opción 4: Khipu bridge
- **Pros**: $0, persona natural, comisión baja.
- **Cons**: sólo transferencia (sin tarjeta) → microtx friction alta. Sin recurring.

## More Information

- [ADR-0003](./0003-payments-webpay-first.md) — decisión original (Webpay = target escala).
- [ADR-0008](./0008-self-sign-pilot-msi.md) — paralelo: cert MSI pilot $0.
- [`docs/strategy/license-server-skeleton.md`](../strategy/license-server-skeleton.md) — estado real del license-server (Webpay sandbox completo, MP/Stripe = gaps).
- [`docs/strategy/zero-cost-launch-plan.md`](../strategy/zero-cost-launch-plan.md) §4 — encaje en plan $0.
- [`docs/strategy/payments-cl.md`](../strategy/payments-cl.md) — comparativa providers.
- Repo license-server: `C:/Users/Administrator/Documents/GitHub/pharma-license-server/` (su `bitacora.md` es fuente de verdad del estado de pagos).
- Mercado Pago developers CL: https://www.mercadopago.cl/developers
- Webpay / Transbank: https://www.transbankdevelopers.cl/
