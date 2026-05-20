# ADR-0002: Licenciamiento Ed25519 con validación offline-first

- **Status**: Accepted
- **Date**: 2026-05-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: licencia, seguridad

## Context and Problem Statement

La decisión de pivotar a freemium ([ADR-0001](./0001-freemium-pivot.md)) requiere un
mecanismo técnico para distinguir entre **tier Free** (todo el mundo) y **tiers/microtx
pagos** (usuarios autorizados). Restricciones duras:

1. **Offline-first absoluto** — el nodo NO debe requerir internet para validar entitlements
   ya activados (CLAUDE.md L13). Internet sólo para compra/refresh opcional.
2. **No-repudio** — el license debe ser inforjable. Firma criptográfica fuerte.
3. **Compat con multi-instalación legítima** — un cliente puede mover su tenant a otra
   máquina sin perder license.
4. **Reuso de infra existente** — el repo ya implementa Ed25519 + canonical-JSON en
   `crates/agent/`. Introducir nueva primitiva sería waste.
5. **Rotación de claves** — escenario realista en 12-24 meses. Debe soportarse sin
   invalidar licenses pre-rotación.

## Decision Drivers

- Tamaño + performance de firma (verify en hot path de startup del nodo).
- Determinismo (Ed25519 elimina riesgo nonce-reuse de ECDSA).
- Side-channel resistance.
- Stack existente.

## Considered Options

1. **JWT firmado HS256** — usaría el mismo flujo que la auth interna (`auth` crate).
2. **RSA-2048 PKCS1** — estándar viejo, ampliamente soportado.
3. **ECDSA P-256** — moderno, pero requiere nonce per signature.
4. **Ed25519 con canonical-JSON** — reusa exactamente `crates/agent/identity.rs` +
   `envelope.rs`.

## Decision Outcome

**Elegida: Opción 4 (Ed25519 con canonical-JSON)**, porque:
- La infra ya está en el repo (`crates/agent/`).
- Performance verify ~70k ops/s, irrelevante en startup.
- Determinístico (no requiere RNG en producción).
- Tamaños pequeños (pubkey 32B, sig 64B) → ideal embedder en binario.
- Curve25519 ~128-bit security, computacionalmente inviable falsificar.

### Consequences

#### Positivas
- Cero deps nuevas en `crates/license`. Sólo wire-up.
- Patrón consistente con el resto del repo. Devs familiares.
- Tests existentes en `crates/agent/identity.rs` cubren la mayor parte de los edge cases.

#### Negativas
- Ed25519 no soporta "key derivation" trivial (PKI hierarchies). Para rotación se requiere
  modelo multi-key con `key_id` en el license. Diseñado en
  [ADR-0007](./0007-key-rotation-licenser.md).
- Ed25519 no es FIPS 140-2 (relevante sólo si se vende a gobierno/defensa, no es target
  ahora).

#### Neutras
- Decisión consistente con cómo el resto de la industria moderna firma licencias
  (Tailscale, 1Password CLI, Authy).

## Pros and Cons of the Options

### Opción 1: JWT HS256 (símétrico)
- **Pros**: trivial, ya en stack.
- **Cons**: **simétrico** — el secret de firma viviría en el binario distribuido = robable.
  Inutilizable para licencias.

### Opción 2: RSA-2048
- **Pros**: estándar antiguo, ubicuo.
- **Cons**: firmas 256B (más grandes), verify ~5k ops/s. Innecesariamente pesado para
  nuestro caso. Necesitaría nueva dep.

### Opción 3: ECDSA P-256
- **Pros**: standard NIST, FIPS-compatible.
- **Cons**: requiere RNG por firma — bug de nonce-reuse históricamente catastrófico
  (Sony PS3, Bitcoin). Más superficie de error.

### Opción 4: Ed25519 (elegida)
- **Pros**: ver decisión.
- **Cons**: ver consecuencias negativas.

## More Information

- [`license-architecture.md`](../strategy/license-architecture.md) §3 (fundamento criptográfico).
- `crates/agent/src/identity.rs:79-117` — funciones reusables.
- `crates/agent/src/envelope.rs:42-99` — patrón canonical-then-sign.
- [ADR-0006](./0006-revocation-strategy-signed-crl.md) — CRL firmado con misma key.
- [ADR-0007](./0007-key-rotation-licenser.md) — rotación.
- RFC 8032 (EdDSA): https://datatracker.ietf.org/doc/html/rfc8032
- RFC 8785 (canonical JSON): https://datatracker.ietf.org/doc/html/rfc8785
