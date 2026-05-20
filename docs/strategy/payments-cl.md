---
title: Rails de pago — Chile
status: Research v1 (no implementación)
date: 2026-05-20
owners: pabloalvarez99 (fundador)
related_adrs:
  - ADR-0003 (Webpay first)
implements_phase: Fase 11
last_review: 2026-05-20
---

# Rails de pago — Chile

> **Research only.** Este documento informa la decisión de Fase 11 (integración de pagos
> con `pharma-license-server`). No es un plan de implementación. Los precios y políticas
> finales requieren validación con design partners + asesoría tributaria.

---

## 1. Objetivo

Seleccionar y secuenciar los rails de pago para:
1. **Suscripciones mensuales/anuales** (Pro, Business).
2. **Microtransacciones one-time** (branding, SII unlock, etc.).
3. **Contratos Enterprise** (factura, transferencia, anual).

Optimizando por: **adopción local CL** > integración técnica > fees > geografía futura.

---

## 2. Comparativa de rails

| Rail | Tipo | Suscripción | One-time | Fees (referencia) | SDK Rust | SDK Node | Setup CL | Payout |
|---|---|---|---|---|---|---|---|---|
| **Webpay (Transbank)** | Tarjeta CL | ✅ Oneclick | ✅ | ~2.95% + IVA | ❌ | ✅ (REST) | KYC formal, ~2-4 semanas | 24-72h hábil |
| **Mercado Pago** | Multi-rail | ✅ Preapproval | ✅ | ~3.49% + IVA | ❌ | ✅ oficial | Self-service, mismo día | T+1 |
| **Khipu** | Transferencia bancaria | ❌ | ✅ | ~1.49% (sin tarjeta) | ❌ | ✅ REST | Self-service | Inmediato |
| **Stripe** | Tarjeta internacional | ✅ Subscriptions | ✅ | ~2.9% + USD 0.30 | community | ✅ oficial | Self-service global | T+2 USD |
| **Mach (BCI)** | Wallet CL | ✅ | ✅ | ~2.5% | ❌ | community | KYC, requiere POS | T+1 |
| **Klap (BCI)** | Tarjeta CL | Limitado | ✅ | ~2.5% | ❌ | community | KYC formal | T+2 |
| **PayKu (gateway agregador)** | Multi-rail | ✅ | ✅ | ~3.9% | ❌ | ✅ | Self-service rápido | T+1 |

### 2.1 Notas por rail

#### Webpay (Transbank)
- **Estándar de facto** B2C/B2B en Chile. El cliente farmacia lo reconoce y confía.
- Onboarding pesado (contrato, KYC formal, integración con `IntegrationCertificateError`
  hasta certificarse). Una vez en producción, fricción cero del lado cliente.
- API REST + webhooks. SDK oficial Node/PHP/Java. Para Rust, llamadas HTTP directas con
  `reqwest` (no requiere SDK específico).
- Oneclick Mall permite suscripción (cobro periódico autorizado).
- **Compliance**: emisión boleta SII obligatoria por venta. Webpay no emite — pharma-license-server debe integrar con servicio DTE (SimpleAPI, Bsale, etc.) o emitir manualmente vía API SII.

#### Mercado Pago
- Self-service onboarding. Bueno para arrancar rápido sin contratos.
- Fees ligeramente más altos.
- Preapproval = suscripción. Maneja retries y dunning automáticamente.
- Multi-país (útil futuro: Argentina, México).
- SDK Node muy maduro.

#### Stripe
- Internacional. **No es rail CL nativo** — el cliente CL ve cobro en USD/EUR y enfrenta
  comisión bancaria de cambio. **No adecuado para B2B farmacias CL**.
- **Sí adecuado** para microtransacciones digitales desde web admin global, donde el
  comprador (dueño/admin del local) puede usar tarjeta internacional.
- Subscriptions excelentes. Webhooks robustos. Idempotency built-in.

#### Khipu
- Transferencia bancaria pull. Fees bajísimos.
- UX: cliente redirige a banking, autoriza pago. No carga tarjeta.
- Sólo one-time, no suscripción (sin mandato persistente).
- Excelente para **Enterprise** y **contratos anuales** donde el cliente prefiere transferir
  vs cargar tarjeta corporativa.

#### Mach / Klap
- Wallets/rails BCI. Cuota de mercado menor que Webpay.
- Considerar en Fase 11+ como secundarios, no primarios.

---

## 3. Compliance Chile

### 3.1 IVA y precios mostrados al cliente

- **IVA 19%** sobre todo servicio digital prestado en Chile.
- Mostrar precios **IVA incluido** al consumidor (Sernac).
- En docs internos y B2B Enterprise, precios pueden ser netos (IVA agregado en factura).
- Stripe internacional: si la compra es desde CL, el IVA igual aplica si el prestador
  está en CL o presta servicio a CL. Asesorar con contador.

### 3.2 Boleta / Factura electrónica SII

- **Toda venta** (suscripción mensual, microtx, contrato Enterprise) requiere DTE
  electrónico al cliente final.
- Tipos relevantes:
  - **Boleta electrónica** (consumidor final, sin RUT detallado).
  - **Factura electrónica** (cliente empresa, con RUT y giro).
- Integración SII: opciones (no implementar en pharma-server, sino en license-server):
  - SimpleAPI, Bsale, Acepta, ChileSystems, Toku, Defontana.
  - Implementación nativa con WebServices SII (más trabajo, sin fee por documento).
- **Decisión Fase 11**: contratar provider DTE (SimpleAPI más probable por simplicidad
  + tier gratuito hasta 200 DTEs/mes). ADR pendiente: `0008-dte-provider-tbd.md`.

### 3.3 Ley 19.628 (Protección de Vida Privada)

- Aplica a **toda data personal** del cliente almacenada en pharma-license-server (email,
  RUT, dirección, métodos de pago tokenizados).
- Obligaciones:
  - Consentimiento informado al registrarse.
  - Derecho de acceso, rectificación, cancelación (ARCO).
  - Política de privacidad pública.
  - Notificación de brechas (estándar emergente, alineado con normativa próxima CL).
- Data del usuario final de la farmacia (clientes, ventas) NUNCA llega al license-server
  — vive sólo en pharma-server local. License-server sólo conoce al **comprador** de la
  licencia, no a los pacientes/clientes de la farmacia. Esto se documenta en política
  de privacidad explícita.

### 3.4 Ley 21.521 (Fintech, 2023)

- Aplica si pharma-license-server califica como **plataforma de financiamiento** o
  **prestador de servicios de iniciación de pagos**. No es nuestro caso: somos
  comerciante final cobrando por servicio, no intermediario.
- No requiere autorización CMF.

### 3.5 Tributario

- IVA mensual + Renta anual estándar.
- Si la entidad comercial es persona natural inicialmente: 1° Cat. + IVA.
- Recomendado constituir SpA antes del primer cobro Pro real (responsabilidad limitada +
  imagen B2B). Hito: tras 5 design partners pagando.

---

## 4. Idempotencia, webhooks y design escalable

> Detalle profundo en [`scaling-architecture.md`](./scaling-architecture.md) §4. Resumen acá.

### 4.1 Idempotency keys

Toda operación de cobro lleva `Idempotency-Key` generado client-side (ULID). Si el rail
o nuestro propio servicio reintenta, no genera doble cobro ni doble license.

Patrón ya usado en pharma-server: `crates/api/src/v1/pos.rs` tiene `Idempotency-Key`
sobre POS. Mismo patrón en license-server.

### 4.2 Webhooks de pago

| Evento | Acción en license-server |
|---|---|
| `payment.succeeded` | Emitir/renovar license, publicar en CDN, notificar admin |
| `payment.failed` | Marcar attempt failed, NO emitir license. Dunning email. |
| `subscription.cancelled` | Marcar `cancel_at_period_end`. License sigue válida hasta `expires_at`. |
| `subscription.renewed` | Re-emitir license con `expires_at` extendido. |
| `refund.created` | Emitir license nuevo con `bought_addons` reducido o tier downgraded. |
| `chargeback.opened` | Trigger dispute flow. License queda en quarantine. |

Reglas:
- Webhooks DEBEN ser idempotentes (verificar `event.id` ya procesado).
- DEBEN verificar firma del rail (`Stripe-Signature`, etc.) antes de procesar.
- Procesamiento async con cola (Redis/SQS). DLQ para fallos.

### 4.3 Reintentos y consistencia

- Si el webhook falla en emitir license (KMS caído): retornar 5xx para que el rail
  reintente. Stripe/MercadoPago hacen retry con exponential backoff.
- Si la emisión es lenta (>5s): responder 200 al webhook con un job-id, procesar async,
  emails el license cuando esté listo (esperable <1min).

---

## 5. Refunds y chargebacks

### 5.1 Política comercial (alineada con `freemium-master-plan.md` §5.1)

- **Suscripciones (Pro/Business)**: 14 días de garantía full refund sin preguntas.
- **Microtx**: 7 días si la feature no fue usada. "Usada" = endpoint que requiere la
  feature key fue llamado al menos 1×.
- **Enterprise**: por contrato, típicamente sin garantía post-onboarding.

### 5.2 Implementación

Refund triggered desde web admin → license-server llama refund API del rail → al webhook
`refund.created`, license-server emite license nuevo (sin la feature) y publica CRL diff
que revoca la license vieja.

### 5.3 Chargebacks

- Disputa: pausar la suscripción, mantener license activa pendiente resolución.
- Si pierde el chargeback: revocar license definitivamente (CRL).
- Si gana: notificar cliente, license sigue activa.

Métrica vigilada: chargeback rate. Si >0.5% → revisión. Si >1% → riesgo de cuenta
suspendida por el rail.

---

## 6. Recomendación staged

### 6.1 Fase 11.0 — MVP cobro
- **Webpay Oneclick** para suscripción Pro/Business mensual.
- **Webpay (oneclick o redirect)** para microtx CL.
- Provider DTE: **SimpleAPI** (free tier hasta 200/mes).
- **No Stripe** todavía.
- **No Khipu** todavía.

### 6.2 Fase 11.1 — Diversificación
- **Stripe** habilitado para compradores con tarjeta internacional (microtx desde web admin
  global). Útil para diáspora CL en otros países.
- **Mercado Pago** como alternativa secundaria (cliente que no quiere ingresar tarjeta a
  Webpay).

### 6.3 Fase 11.2 — Enterprise
- **Khipu** + **transferencia manual + factura** para contratos anuales Enterprise.
- Onboarding asistido por ingeniero asignado.

### 6.4 Fase 11.3+ — Multi-país (LATAM)
- **Mercado Pago** se vuelve primary para Argentina/México/Colombia.
- **Stripe** para resto del mundo.
- Webpay sólo CL.

---

## 7. Pricing display y multi-currency-ready

- v1: precios en CLP únicamente. Mostrados con punto miles (`$14.900` no `$14,900`).
- Schema license soporta `metadata.billing_currency` para futuro multi-currency.
- UI checkout: hardcoded CLP, con TODO para `Accept-Language` based switching.
- **No usar conversión spot live** — fija precios por moneda y revisa trimestral.

---

## 8. Open questions (decisiones pendientes para sesiones futuras)

1. **¿Provider DTE final?** SimpleAPI vs Bsale vs nativo SII. ADR pendiente.
2. **¿Constituir SpA antes de Pro launch?** Asesoría legal.
3. **¿Dunning automático Webpay?** Cuantos intentos antes de cancel.
4. **¿Suscripción anual con descuento o no?** Validar con design partners (impacto en CAC payback).
5. **¿Stripe + IVA CL?** Confirmar con contador si retención aplica.

---

## 9. Referencias

- [ADR-0003 — Webpay como rail primario](../adr/0003-payments-webpay-first.md)
- [`freemium-master-plan.md`](./freemium-master-plan.md) §5 — política de cobros
- [`license-architecture.md`](./license-architecture.md) §4 — activation flow
- [`scaling-architecture.md`](./scaling-architecture.md) §4 — webhook ingestion design
- Webpay docs: https://www.transbankdevelopers.cl/
- Mercado Pago dev: https://www.mercadopago.cl/developers
- Stripe docs: https://stripe.com/docs
- Khipu docs: https://khipu.com/page/api
- SII DTE: https://www.sii.cl/factura_electronica/
