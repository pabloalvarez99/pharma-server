curs---
title: Architecture Decision Records (ADRs)
status: Activo
owners: pabloalvarez99
last_review: 2026-05-20
template: MADR 3.0
---

# Architecture Decision Records

Registro estructurado de **decisiones arquitectónicas significativas**. Una decisión por archivo.
Inmutable una vez aceptada — si cambia, se crea un ADR nuevo que la supersede.

## Template (MADR 3.0)

Copiar para nuevas decisiones, numerar al siguiente disponible:

```markdown
# ADR-NNNN: Título corto en imperativo

- **Status**: Proposed | Accepted | Deprecated | Superseded by ADR-MMMM
- **Date**: YYYY-MM-DD
- **Deciders**: lista de personas
- **Tags**: [licencia, pagos, seguridad, ...]

## Context and Problem Statement

¿Qué problema estamos resolviendo? ¿Qué fuerzas están en juego? 2-4 párrafos.

## Decision Drivers

- Driver 1 (ej. costo, compliance, time-to-market)
- Driver 2
- ...

## Considered Options

1. **Opción A** — descripción 1 línea.
2. **Opción B** — descripción 1 línea.
3. **Opción C** — descripción 1 línea.

## Decision Outcome

**Elegida**: Opción X, porque <razón principal>.

### Consequences

- **Positivas**: ...
- **Negativas**: ...
- **Neutras**: ...

## Pros and Cons of the Options

### Opción A
- Pros: ...
- Cons: ...

### Opción B
- Pros: ...
- Cons: ...

## More Information

- Referencias, links, commits, RFCs externos.
```

## Reglas

1. **Numeración monotónica** — `0001`, `0002`, ... sin huecos. Reservar el ID al crear el archivo.
2. **Status inicial**: `Proposed`. Pasa a `Accepted` cuando el fundador (o board) firma.
3. **No editar ADRs aceptados**. Si cambia el contexto, ADR nuevo + marcar el viejo `Superseded by ADR-MMMM`.
4. **Tamaño**: idealmente 1 pantalla, máximo 2. Si es más largo, está mezclando varias decisiones — separar.
5. **Tags consistentes**: `licencia`, `pagos`, `seguridad`, `compliance`, `producto`, `infra`, `protocolo`.

## Índice

| ID | Título | Status | Tags |
|---|---|---|---|
| [0001](./0001-freemium-pivot.md) | Pivote a modelo freemium MSI | Accepted | producto, modelo-negocio |
| [0002](./0002-license-ed25519-offline.md) | Licencia Ed25519 con validación offline-first | Accepted | licencia, seguridad |
| [0003](./0003-payments-webpay-first.md) | Webpay como rail de pago primario para Chile | Accepted | pagos |
| [0004](./0004-license-server-separado.md) | License-server vive en repo separado | Accepted | infra, licencia |
| [0005](./0005-core-gratis-no-locked-in.md) | Invariantes del core gratis (sin paywall a export, sin kill-switch) | Accepted | producto, compliance |
| [0006](./0006-revocation-strategy-signed-crl.md) | Revocación vía CRL firmado distribuido por CDN | Accepted | licencia, seguridad, infra |
| [0007](./0007-key-rotation-licenser.md) | Rotación de claves del licenser con key-id en license | Accepted | licencia, seguridad |
| [0008](./0008-self-sign-pilot-msi.md) | Self-sign pilot MSI antes de cert pago (camino $0) | Accepted | distribución, msi, presupuesto |
| [0009](./0009-pilot-payment-provider.md) | Mercado Pago primer rail LIVE pilot (Webpay ya en sandbox) | Accepted | pagos, presupuesto |
| [0010](./0010-roadmap-fase-9-parity.md) | Roadmap Fase 9.x — paridad mínima vendible vs competencia CL | Accepted | producto, roadmap |
| [0011](./0011-dte-provider-native-rust.md) | DTE (boleta/factura SII) implementado nativo en Rust | Accepted | producto, compliance, integraciones |
| [0012](./0012-web-onprem-interop.md) | Interop web ↔ pharma-server vía HTTP only (3 patrones) | Accepted | producto, infra, protocolo, interop |
| [0013](./0013-sync-bidireccional-stock.md) | Sync de stock núcleo→storefront (push webhook, no bloquea POS) | Accepted | interop, storefront |
| [0014](./0014-dss-storefront-integration.md) | DSS como capa storefront por seam HTTP (no merge) | Accepted | producto, storefront, multi-rubro |
| [0015](./0015-universal-cross-platform-client.md) | Cliente universal Tauri 2 + PWA sobre API-first | Accepted | cliente, cross-platform |
| [0016](./0016-agent-assist-architecture.md) | Arquitectura del agente assist (provider trait, propose→confirm) | Accepted | agente, arquitectura |
| [0017](./0017-byo-ai-provider.md) | BYO-AI-provider (LLM opt-in con key del dueño, default OFF) | Accepted | agente, IA |
| [0018](./0018-cloud-multitenant-saas.md) | Tier cloud multi-tenant (web zero-install) + signup por RUT | Proposed | cloud, distribución |
| [0019](./0019-product-surface-taxonomy.md) | Taxonomía de superficies (núcleo + operador + storefront), genérica para todo cliente | Accepted | producto, arquitectura, naming |
