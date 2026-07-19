# ADR-0007: Rotación de claves del licenser con `key_id` en license

- **Status**: Accepted
- **Date**: 2026-05-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: licencia, seguridad

## Context and Problem Statement

Una **clave de firma de licenses** que vive 12+ meses sin rotación es un riesgo de
seguridad creciente (compromiso, leak, sospecha de exposición). Industry best practice:
rotar cada 12-24 meses + procedimiento documentado de rotación de emergencia.

Pero la rotación tiene un problema: **las licenses ya emitidas siguen siendo válidas**
hasta su `expires_at`. Si el binario sólo conoce la pubkey actual, después de rotar
TODAS las licenses pre-rotación se invalidan inmediatamente — inaceptable.

Necesitamos un esquema que permita:
1. Rotar la keypair del licenser sin invalidar licenses históricas.
2. Validar licenses firmadas por cualquier key histórica aceptada.
3. Retirar keys viejas después de un periodo razonable (cuando todas las licenses
   firmadas con ellas hayan expirado).

## Decision Drivers

- Compat backward con licenses pre-rotación.
- Mínima superficie de complejidad en el nodo (verify es hot path de startup).
- Capacidad de respuesta rápida ante compromiso de key.
- Tamaño del binario MSI (cada pubkey ~32B + metadata — irrelevante).

## Considered Options

1. **Una key, sin rotación** — status quo simple, riesgo alto a largo plazo.
2. **Rotación con `key_id` en license + slice de keys históricas embebidas** —
   patrón JWT `kid`.
3. **Trust-store dinámico descargable** — el binario descarga keys actuales del licenser.

## Decision Outcome

**Elegida: Opción 2 (multi-key con `key_id` en license)**.

### Diseño

#### License lleva `key_id`

```jsonc
{
  ...
  "issuer_did": "did:pharma:<bs58 de la pubkey activa al momento de firmar>",
  "key_id": "lk-2026-01",
  ...
}
```

#### Binario embebe slice de pubkeys aceptadas

En `crates/license/src/lib.rs`:

```rust
pub const LICENSER_KEYS: &[(&str, &str)] = &[
    // key_id, did
    ("lk-2026-01", "did:pharma:abc...current"),   // emite licenses nuevas
    ("lk-2025-01", "did:pharma:xyz...retired"),   // sólo valida licenses pre-rotación
];

pub fn verify_license(license: &License) -> Result<(), LicenseError> {
    let (_, did) = LICENSER_KEYS
        .iter()
        .find(|(kid, _)| *kid == license.key_id)
        .ok_or(LicenseError::UnknownKeyId)?;

    verify_with_did(did, &license.canonical_payload(), &license.signature)?;
    Ok(())
}
```

#### Procedimiento de rotación programada (cadencia 12 meses)

1. **Día -90**: KMS genera keypair nueva. Pubkey publicada en `did:web:pharma-server.cl`
   y notas internas. Privada nunca sale del KMS.
2. **Día -90 a Día 0**: release del binario `pharma-server` que incluye AMBAS keys
   (nueva + actual). Despliegue staged a 100% de fleet.
3. **Día 0**: license-server hace **switch del signer activo a la key nueva**. Licenses
   nuevas firman con `lk-NEW`. Licenses viejas (con `lk-OLD`) siguen válidas.
4. **Día 0 a Día +360**: ambas keys aceptadas. Licenses viejas expiran naturalmente.
5. **Día +365**: monitoreo confirma que todas las licenses con `lk-OLD` ya expiraron.
6. **Día +730 (≈2 años post-rotación)**: próxima release puede remover entry de `lk-OLD`
   del slice. (No urgente; no compromete seguridad mantenerla.)

#### Procedimiento de rotación de emergencia (compromiso de key)

1. **T+0**: detección de compromiso (audit log alert, leak público, sospecha).
2. **T+1h**: KMS revoca permiso de signing con la key comprometida.
3. **T+1h**: license-server publica **CRL global** que invalida TODAS las licenses
   firmadas con `lk-COMPROMETIDA`. Esto es una entrada especial en CRL (`reason:
   "key_compromise"`).
4. **T+2h**: emisión urgente de release del binario con nueva key. CI emergency-build.
5. **T+24h**: re-emisión automática de licenses para todos los tenants legítimos con la
   nueva key. Gratis. Notificación email.
6. **T+48h**: post-mortem público (`docs/incidents/YYYY-MM-DD-key-rotation-emergency.md`).
7. **Compensación**: extensión de `expires_at` de licenses pagadas por al menos 30 días.

### Consequences

#### Positivas
- Rotación sin invalidar licenses históricas legítimas.
- Procedimiento de emergencia probado y documentado.
- Tamaño del binario impactado mínimamente (32B per key + metadata).
- Consistente con JWT `kid` pattern, familiar para devs.

#### Negativas
- Lista de keys históricas crece lentamente con el tiempo. Mitigación: pruning después
  de 24 meses post-rotación.
- Si la key se compromete, hay ventana hasta que el binario nuevo se despliega + CRL
  propaga. Mitigación: CRL global rápida + comunicación.

#### Neutras
- Cada release de binario es trivialmente diff-able en `LICENSER_KEYS`.
- Política documentada permite a clientes Enterprise auditar.

## Pros and Cons of the Options

### Opción 1: Sin rotación
- **Pros**: simple.
- **Cons**: riesgo de compromiso acumulado. Inaceptable a 12+ meses.

### Opción 2: Multi-key con `key_id` (elegida)
- **Pros**: ver decisión.
- **Cons**: ver consecuencias negativas.

### Opción 3: Trust-store dinámico descargable
- **Pros**: el binario nunca se queda con keys obsoletas.
- **Cons**: viola offline-first **hard** — un nodo aire-gapped no puede descargar keys
  nuevas, y por tanto no puede validar licenses futuras. Inaceptable.

## More Information

- [`license-architecture.md`](../strategy/license-architecture.md) §6 — descripción técnica.
- [ADR-0002](./0002-license-ed25519-offline.md) — primitiva Ed25519.
- [ADR-0006](./0006-revocation-strategy-signed-crl.md) — CRL global usa misma key
  rotation.
- JWT key rotation: RFC 7515 (JWS) §4.1.4 (`kid`).
- NIST SP 800-57 (cryptographic key management).
