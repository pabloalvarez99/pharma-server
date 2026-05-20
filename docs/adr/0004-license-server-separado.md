# ADR-0004: License-server en repositorio separado

- **Status**: Accepted
- **Date**: 2026-05-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: infra, licencia

## Context and Problem Statement

El modelo freemium ([ADR-0001](./0001-freemium-pivot.md)) requiere un servicio cloud que:
- Reciba pagos de Webpay/Stripe (webhooks).
- Emita licenses firmadas con la private key del licenser (KMS).
- Distribuya licenses + CRL vía CDN.
- Provea web admin (checkout, downloads, status).

Pregunta: **¿vive este servicio en el mismo repo que `pharma-server` o en otro?**

Restricciones:
- `pharma-server` es **Rust on-prem para Windows** (target binario MSI).
- El servicio cloud sería **Next.js + Postgres** (Vercel).
- Stacks, build pipelines, deployment targets son completamente distintos.
- El binario distribuido NO debe contener credenciales del licenser ni código del backend.

## Decision Drivers

- Separación clara de superficies de ataque.
- CI/CD independiente (deploy a Vercel ≠ MSI build).
- Tamaño y simplicidad de cada repo (encore-of-concerns).
- Compatibilidad con regla precedente en el ecosistema:
  `pharma-server` ya está separado de `build-and-deploy-webdev-asap`
  (Tu Farmacia LOCAL Coquimbo) por la misma lógica
  ([CLAUDE.md L44-53](../../CLAUDE.md)).

## Considered Options

1. **Monorepo** — `pharma-server` con workspace adicional `license-server/` (Next.js
   convivirá con Rust workspace).
2. **Repo separado** — `pabloalvarez99/pharma-license-server`, independiente.

## Decision Outcome

**Elegida: Opción 2 (repo separado `pharma-license-server`)**.

### Consequences

#### Positivas
- Cero confusión sobre dónde vive cada cosa.
- CI/CD limpio (Vercel pipeline en el repo de license-server, GitHub Actions Windows en
  pharma-server, ningún paso compartido).
- Diferentes contributors, diferentes secrets, diferentes review processes.
- El binario distribuido nunca puede incluir accidentalmente código del backend.
- Permite open-sourcear pharma-server (futuro) sin exponer infra interna del licenser.
- Consistente con la regla establecida (`pharma-server` ≠ `build-and-deploy-webdev-asap`).

#### Negativas
- **Schema sync manual** — el formato del license JSON debe mantenerse compatible entre
  ambos repos. Mitigación: versionado explícito (`schema_version` en license, ver
  [`license-architecture.md`](../strategy/license-architecture.md) §2.1) + contracts test
  cross-repo (futuro `pharma-license-server/contracts/license-v1.schema.json` consumido
  por tests de `crates/license`).
- Onboarding requiere clonar 2 repos para entender flujo end-to-end. Mitigado por links
  cruzados en docs.

#### Neutras
- La pubkey del licenser es pública (embebida en binario). No requiere coordinación
  secret-management entre repos.

## Pros and Cons of the Options

### Opción 1: Monorepo
- **Pros**: un solo clone, refactors atomic cross-stack.
- **Cons**: build pipeline complejo (Rust + Node + Vercel deploy + MSI build). Onboarding
  ruidoso. Risk de leak de secrets/código backend al binario. Viola regla precedente.

### Opción 2: Repo separado (elegida)
- **Pros**: ver decisión.
- **Cons**: ver consecuencias negativas.

## More Information

- [`license-architecture.md`](../strategy/license-architecture.md) §10 — separación detallada.
- [CLAUDE.md L44-53](../../CLAUDE.md) — regla precedente.
- Próximo repo: `pabloalvarez99/pharma-license-server` (Next.js + Postgres + Stripe/Webpay).
