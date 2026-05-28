# ADR-0011: DTE (boleta/factura electrónica SII) implementado nativo en Rust

- **Status**: Accepted
- **Date**: 2026-05-21
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, compliance, integraciones

## Context and Problem Statement

Fase 9.1 requiere soporte completo de **Documentos Tributarios Electrónicos** del
SII chileno: boleta electrónica (tipo 39), factura electrónica (33), nota de débito
(56), nota de crédito (61), guía de despacho (52). Sin esto el ERP no es vendible a
ninguna farmacia que facture (≈100% del mercado objetivo desde 2022 obligación SII).

DTE involucra: generación XML schema SII (xsd 1.0), firma RSA-SHA1 con cert digital
empresa, `TimbreElectronico` con CAF (Código de Autorización de Folios), envío al
endpoint SII (sandbox `maullin.sii.cl` / prod `palena.sii.cl`), polling de estado,
libro de ventas mensual, reportes X/Z. Es módulo crítico, hot path operacional, y
toca compliance (errores = multas SII al cliente).

Tres caminos posibles para construirlo. La decisión define ownership, costo ongoing,
friction de onboarding, y alineación con los pillars del producto.

## Decision Drivers

- **Pillar offline-first**: core ERP debe operar sin internet. DTE envío SII es
  eventual (cola + retry) no hot path POS.
- **Pillar vendor-agnostic**: cliente debe ser dueño de sus datos y no depender de
  un tercero para emitir.
- **Pillar 1-click MSI sin runtime extra**: nada de Docker, PHP, Java, Node externo.
- **Pillar costo cero ongoing**: licencia única + soporte. Microtx opcionales.
- **Compliance CL**: schema SII evoluciona (xsd nuevos), debemos poder adaptar
  rápido sin esperar a un proveedor externo.
- **Time-to-market**: cuanto antes vendible mejor — pero no a costa de los pillars.

## Considered Options

1. **Native Rust** — implementación propia (XML, firma, timbre, envío) en `crates/dte`.
2. **SimpleAPI / OpenFactura managed gateway** — proveedor cloud chileno, API REST.
3. **LibreDTE FOSS self-hosted** — PHP runtime que orquesta firma + envío.

## Decision Outcome

**Elegida: Opción 1 (Native Rust en `crates/dte`)**.

Razón principal: las tres alternativas managed/self-host violan al menos un pillar
no-negociable. SimpleAPI rompe vendor-agnostic + costo ongoing ($15-30 CLP/DTE
revendidos). LibreDTE rompe "sin runtime extra" (PHP). Native Rust mantiene los
pillars y nos da control total sobre evolución del schema SII.

### Diseño

#### Crate nuevo `crates/dte`

```
crates/dte/
  src/
    lib.rs          // re-exports + tipos públicos
    types.rs        // DteTipo enum, Dte, DteItem, Caf, CertDigital
    xml/
      mod.rs        // ser/de XML schema SII xsd 1.0
      boleta.rs     // tipo 39
      factura.rs    // tipo 33
      nota_credito.rs // tipo 61
      nota_debito.rs  // tipo 56
      guia.rs       // tipo 52
      libro.rs      // libro ventas mensual
    timbre.rs       // TED: hash SHA1 + firma RSA-SHA1
    sign.rs         // firma XML completa con cert empresa
    sii.rs          // POST multipart sandbox/prod, parse track_id
    caf.rs          // parse CAF XML + folio assignment atómico
    cert.rs         // PFX load + encrypt-at-rest (argon2id derived key)
    error.rs        // DteError enum
  tests/
    fixtures/       // XML samples SII (boleta_39_ejemplo.xml, etc)
    xml_boleta_39.rs
    timbre_roundtrip.rs
    caf_folio_atomic.rs
```

Dependencias clave (workspace-level):
- `quick-xml = "0.36"` — ser/de XML rápido.
- `rsa = "0.9"` — RSA-SHA1 firma (SII requiere SHA1, no negociable).
- `sha1 = "0.10"` — hash legacy obligatorio SII.
- `reqwest` — ya en workspace, multipart upload SII.
- `tokio` — async runtime.

#### Endpoints API (módulo `crates/api/src/v1/dte/`)

| Método | Path | Tier |
|---|---|---|
| POST | `/api/v1/dte/issue` | Free (local-only) / Pro (envío SII) |
| GET | `/api/v1/dte/:id` | Free |
| GET | `/api/v1/dte` | Free |
| POST | `/api/v1/dte/:id/cancel` | Free (draft/signed) |
| POST | `/api/v1/dte/:id/resend` | Pro |
| GET | `/api/v1/dte/reports/libro-ventas?month=YYYY-MM` | Business |
| GET | `/api/v1/dte/reports/x` | Pro |
| GET | `/api/v1/dte/reports/z` | Pro |
| POST | `/api/v1/admin/caf/upload` | Free |
| POST | `/api/v1/admin/cert/upload` | Free |

#### Tier monetización

- **Free**: generar DTE local (XML firmado válido), export PDF/XML. Cliente envía
  manual al portal SII. Cero dependencia internet.
- **Pro**: envío SII automático boleta (39). Polling estado. Resend automático.
- **Business**: todos tipos (33/56/61/52), libro ventas SII mensual, X/Z reportes,
  multi-cert (cadenas con holdings).
- **Microtx "SII managed unlock"**: futuro Fase 9.1.5 — gateway SimpleAPI/OpenFactura
  como alternativa al cert+CAF propio para clientes que no quieren manejarlo. Opt-in,
  no default.

#### Migración `0017_dte.surql`

3 tablas: `dte`, `caf`, `cert_digital`. Multi-tenant scoped (`tenant: record<tenant>`
+ índices compuestos). CAF tiene `UNIQUE(tenant, tipo_dte, folio_desde)` para evitar
duplicados al subir XMLs. Folio assignment atómico via `BEGIN; UPDATE caf SET
next_folio = next_folio + 1 WHERE ... RETURN BEFORE; COMMIT` (SurrealDB transacciones).

#### Cert digital encrypt-at-rest

PFX blob persistido como `bytes` cifrado con clave derivada `argon2id(tenant_id,
master_key)`. Master key en env `PHARMA__DTE__MASTER_KEY` (32 bytes hex). Default
deriva de `PHARMA__JWT__SECRET` si no existe (warning log: documentar para producción).
Password del PFX cifrado con mismo esquema. Decifra solo en memoria al firmar, descarta.

#### Sandbox vs prod switch

Env `PHARMA__DTE__SII_ENV=sandbox|prod`, default `sandbox`. Solo `prod` toca
`palena.sii.cl`. Tests integration apuntan siempre `sandbox` (`maullin.sii.cl`),
nunca `prod`. CLI requiere `--confirm-prod` para operaciones reales.

### Consequences

#### Positivas

- Vendor-lock-in **cero** — cliente puede operar indefinidamente con el último
  release aunque la empresa cierre (compromiso ADR-0005).
- Costo ongoing **cero** — cliente paga licencia una vez (microtx o tier), no
  per-DTE.
- Schema SII actualizable en horas si SII publica xsd nuevo (control total).
- Offline-first **respetado**: Free tier emite local sin internet. Pro/Business
  envío SII es cola + retry, no bloquea POS.
- Differentiation real vs SICO/GOLAN/iFarmacias (todos dependen de proveedor SII
  managed externo).

#### Negativas

- Esfuerzo dev 4-6 semanas vs 1-2 con managed. Mitigación: subtasks a-m bien
  granulares, primer PR (subtask a-c: XML+timbre+CAF) entregable en 1 semana.
- Mantenimiento ongoing: cambios SII schema, certs caducados, edge cases. Mitigación:
  tests integration vs sandbox SII en CI nocturno.
- Cliente debe gestionar su propio cert digital + CAF (friction onboarding). Mitigación:
  docs guiados + microtx "SII managed" Fase 9.1.5 como escape hatch.

#### Neutras

- Crate `dte` separado permite open-sourcear independiente del resto (decisión
  Fase 13+). Mientras tanto privado.
- Reusa primitivas existentes (`reqwest`, `tokio`, `chrono`, `serde`, `arc-swap`
  para cache CAF).

## Pros and Cons of the Options

### Opción 1: Native Rust (elegida)

- **Pros**: ver decisión.
- **Cons**: ver consecuencias negativas.

### Opción 2: SimpleAPI / OpenFactura

- **Pros**: time-to-market 1-2 semanas. Vendor mantiene compliance SII.
- **Cons**:
  - **Rompe pillar vendor-agnostic** — cliente atado a vendor externo para
    operar (si vendor cae, farmacia no factura).
  - **Rompe pillar costo cero ongoing** — $15-30 CLP/DTE * miles/mes = pasivo permanente.
  - Requiere internet para emitir (rompe offline-first).
  - Compliance del vendor ≠ compliance nuestro (multas SII al cliente si
    vendor falla).

### Opción 3: LibreDTE FOSS self-hosted

- **Pros**: cero costo ongoing. FOSS GPL3.
- **Cons**:
  - **Rompe pillar "sin runtime extra"** — requiere PHP runtime + composer +
    extensions en la máquina del cliente. MSI 1-click pierde su simplicidad.
  - Empaquetar PHP en MSI es posible pero abre superficie de soporte (versiones,
    parches, conflictos con instalaciones existentes).
  - Code review: el módulo SII de LibreDTE es viejo, no tipado, difícil auditar.
  - Forking para mantener seguro es comparable en esfuerzo a Rust nativo.

## More Information

- Plan implementación: subtasks 9.1.a-m documentadas en `bitacora.md` BACKLOG
  Fase 9.1.
- Schema SII oficial: https://palena.sii.cl/cvc_cgi/dte/of_oferta (xsd 1.0).
- Sandbox endpoint: `https://maullin.sii.cl/cgi_dte/UPL/DTEUpload`.
- Prod endpoint: `https://palena.sii.cl/cgi_dte/UPL/DTEUpload`.
- CAF (Código Autorización de Folios): trámite SII en
  https://palena.sii.cl/cvc_cgi/dte/of_solicita_folios.
- Cert digital: proveedores autorizados SII (E-CertChile, etc).
- ADR-0010 — roadmap Fase 9.x parity (DTE es entrada principal).
- ADR-0005 — compromiso de continuidad: razón clave para preferir native.
