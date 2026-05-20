# ADR-0006: Revocación vía CRL firmado distribuido por CDN

- **Status**: Accepted
- **Date**: 2026-05-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: licencia, seguridad, infra

## Context and Problem Statement

El modelo de licenciamiento ([ADR-0002](./0002-license-ed25519-offline.md)) emite license
files firmados con `expires_at` distante (típico 1 año). Necesitamos un mecanismo para
**revocar una license antes de su expiración natural** en casos como:
- Refund de suscripción.
- Chargeback exitoso del banco.
- Fraude detectado.
- Compromiso de keypair específica de un tenant.

Restricciones derivadas de ADRs previos:
1. **Offline-first**: la revocación NO puede requerir conexión obligatoria del nodo.
2. **Sin kill-switch del core gratis** ([ADR-0005](./0005-core-gratis-no-locked-in.md)
   §6) — revocar license NUNCA bloquea POS/inventario.
3. **Validación criptográficamente verificable** — un atacante MITM no debe poder
   inyectar revocaciones falsas.

## Decision Drivers

- Latencia de propagación (qué tan rápido un refund se refleja en el nodo).
- Escalabilidad (millones de licenses históricas potenciales).
- Bandwidth en el nodo (CRLs grandes consumen ancho de banda en revisión periódica).
- Costo del licenser (KMS sign + CDN bandwidth).
- Compatibilidad con offline-first.

## Considered Options

1. **OCSP-like (online check por license_id antes de cada uso)** — máxima freshness,
   peor offline.
2. **CRL completo descargado periodicamente** — un blob con todos los license_ids
   revocados, firmado.
3. **CRL versionado incremental (diff)** — `crl-vN.json` con versión monotónica;
   nodo guarda última versión vista y descarga diffs.
4. **Short-lived licenses con renovación frecuente** — sin CRL, renovar cada 24-48h.

## Decision Outcome

**Elegida: Opción 3 (CRL versionado incremental, firmado, distribuido por CDN)**.

### Diseño

#### Esquema de cada versión CRL

```jsonc
{
  "schema_version": 1,
  "crl_version": 247,                              // monotónico, increment-only
  "previous_version": 246,                         // nulo en v1, sino enlaza cadena
  "published_at": "2026-05-20T03:00:00Z",
  "diff": {
    "added": [
      { "license_id": "lic_01HX...", "revoked_at": "2026-05-19T22:14:32Z", "reason": "refund" }
    ],
    "removed": []                                  // típicamente vacío; revocaciones son permanentes
  },
  "issuer_did": "did:pharma:<bs58>",
  "key_id": "lk-2026-01",
  "signature": "<base64 ed25519 sobre canonical sin signature>"
}
```

#### Endpoints

```
GET https://cdn.pharma-server.cl/crl/crl-v{N}.json
  Cache-Control: public, max-age=31536000, immutable

GET https://cdn.pharma-server.cl/crl/crl-latest.json
  → 302 Location: /crl/crl-v{N}.json   (donde N es el último publicado)
  Cache-Control: public, max-age=60
```

#### Snapshot completo (cuando un nodo arranca por primera vez)

```
GET https://cdn.pharma-server.cl/crl/snapshot-v{N}.json
  → Lista completa (no diff) de license_ids revocados hasta crl_version=N
  Cache-Control: public, max-age=31536000, immutable
  Re-publicado mensualmente (último día) para mantener tamaño manejable
```

#### Flujo de consumo en el nodo (si `refresh_enabled=true`)

```
1. Al startup: leer ./data/crl_state.json → { last_seen_version: M }.
2. GET /crl/crl-latest.json → resuelve a N actual.
3. Si N == M: no-op.
4. Si N > M:
   a. Si N - M > 100 (gap grande): GET /crl/snapshot-v{N}.json y reemplazar cache.
   b. Si N - M <= 100: GET cada /crl/crl-v{M+1..N}.json secuencialmente, aplicar diffs.
5. Verificar firma de cada versión.
6. Persistir cache local + last_seen_version = N.
7. Próximo refresh: jittered 7d ±24h.
```

### Consequences

#### Positivas
- **Bandwidth eficiente**: nodos sólo descargan diffs incrementales (~bytes/día).
- **CDN-friendly**: todos los archivos son inmutables y cacheables forever.
- **Offline graceful**: nodo sin red sigue funcionando con el último CRL conocido.
- **Auditable**: el chain de versiones permite reconstruir el orden histórico.
- **Cheap to scale**: CRL distribution es ~99% gratis vía CDN.

#### Negativas
- **Latencia de propagación**: refund tarda hasta 7 días en propagarse a nodos offline
  (con refresh_enabled). Para fraude crítico, mitigación = `expires_at` corto + forzar
  refresh por contrato.
- **Cold-start cost**: nodo nuevo descarga snapshot completo (~MB potencialmente).
- **Garbage collection**: license_ids viejos podrían quedar en CRL forever. Mitigación:
  expirar entries del CRL cuando `revoked_at + max_license_lifetime < now` (la license
  ya expiró naturalmente, no hay que seguir tracking).

#### Neutras
- La cadena de versiones depende de monotonicidad. Si el license-server colapsa y se
  restaura desde backup antiguo, hay que **continuar** la numeración (nunca reiniciar
  desde N-K).

## Pros and Cons of the Options

### Opción 1: OCSP online check
- **Pros**: freshness inmediata.
- **Cons**: viola offline-first. Cada uso de feature paga = round-trip al server.

### Opción 2: CRL completo cada vez
- **Pros**: simple.
- **Cons**: bandwidth crece linealmente con # revocaciones históricas. Inviable a escala.

### Opción 3: CRL incremental versionado (elegida)
- **Pros**: ver decisión.
- **Cons**: ver consecuencias negativas.

### Opción 4: Short-lived licenses
- **Pros**: sin CRL needed.
- **Cons**: renovar cada 48h **viola offline-first hard**. Nodo sin internet pierde acceso
  a features pagadas. Inaceptable.

## More Information

- [`license-architecture.md`](../strategy/license-architecture.md) §5.3 — flow detallado.
- [`scaling-architecture.md`](../strategy/scaling-architecture.md) §4 — CDN distribution.
- [ADR-0007](./0007-key-rotation-licenser.md) — CRL firma usa misma key del licenser.
- X.509 CRL (referencia inspiracional): RFC 5280.
