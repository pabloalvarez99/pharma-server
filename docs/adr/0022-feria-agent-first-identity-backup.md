# ADR-0022: Feria agent-first + identidad Google + respaldo cifrado con llave del usuario

- **Status**: Accepted
- **Date**: 2026-08-08
- **Deciders**: pabloalvarez99 (founder) + orquestador Grok (ejecución)
- **Tags**: producto, feria, agente, identidad, backup, multi-rubro, android
- **Related**: [ADR-0005](./0005-core-gratis-no-locked-in.md) (offline core) · [ADR-0021](./0021-android-compose-nativo.md) (Android Compose) · pack `feria` en `domain::rubro`

## Context

El founder redefinió el cliente prioritario: **feriantes y vendedores de calle** que
hoy viven del cuaderno, con celular antiguo, poco de tecnología, sol, una mano libre,
datos caros, a menudo sin SII. El competidor es un cuaderno de mil pesos.

El ERP actual está sesgado a farmacia/retail formal (barcode, impresora, DTE). Esos
módulos valen casi cero en el puesto. El activo diferencial para este usuario es el
**agente conversacional** (`Action::Vender` / `FiarVenta`) más offline + fiado.

En paralelo: identidad sin PC (Google) y respaldo en bucket propio de RutBusiness
con **llave del usuario** (RutBusiness no lee plaintext).

## Decision

### 1. Un solo producto, pack de rubro `feria` (no app separada)

- Misma app, mismo server, mismo freemium, mismos rieles Fase 13.
- `business.vertical = feria` activa un `RubroPack` con:
  - `agent_home: true`
  - `barcode: false`, `printer: false`, `dte: false`
  - `informal_ok: true`
  - vocab: item "Cosa", catalog "Lo que vendo"
- Escáner, impresora y DTE **siguen en el código** para otros rubros y para un
  futuro "modo formal" del feriante; **no se muestran** day-1 en feria.
- Farmacia deja de ser beachhead de producto; permanece como vertical profundo.

### 2. Superficie day-1 (Android)

Pantallas: (1) **Agente** = home, (2) **Quién me debe** (fiado), (3) **Hoy**
(resumen), (4) ajustes mínimos. Sin POS denso, sin catálogo de farmacia, sin
caja de supermercado.

Vara: **ganar al cuaderno** en velocidad de anotar una venta y de saber quién debe.

### 3. Identidad: Google Sign-In

- Entrada primaria en móvil: cuenta Google (sin depender de PC).
- El tenant/negocio se asocia a esa identidad; el core offline sigue sin necesitar
  red una vez autenticado (ADR-0005).

### 4. Respaldo: ciphertext en bucket RutBusiness, llave del usuario

- Bytes en infraestructura propia (costo aceptado por founder).
- Cifrado en cliente **antes** de subir. El servidor de backup no ve plaintext.
- **Recuperación feriante (no gestor de contraseñas):**
  1. Al crear cuenta se genera una **clave del negocio** (12–16 palabras cortas
     o 8 bloques de 4 caracteres en español legible).
  2. La app **obliga** a mostrarla en pantalla grande + "escribila en tu cuaderno".
  3. Opción: **tarjeta de rescate** imprimible / PDF de una página con QR +
     palabras (el feriante pega la hoja en el cuaderno).
  4. Sin la clave, el backup es basura y **se lo decimos el día 1**, no el día del
     robo del teléfono.
  5. No hay "RutBusiness recupera tu clave". Hay solo re-ingreso de la tarjeta /
     palabras + login Google.

### 5. Offline-first no se toca en silencio

- El core sigue operando sin internet (ADR-0005).
- Backup remoto y Google son **opt-in de continuidad**, no requisito de venta del día.

## Consequences

### Positive

- Un solo codebase sirve feria y farmacia vía packs.
- La vara del cuaderno mata feature bloat.
- Backup zero-knowledge reduce blast radius de un leak del bucket.

### Negative / risks

- Pérdida de llave = pérdida de historia (mitigado por tarjeta en el cuaderno +
  copy agresivo day-1).
- Extender `RubroFeatures` exige actualizar cliente TS + Android gating.
- Google + bucket añaden ops (cuentas, costos, OAuth).

### Out of scope here

- Implementación completa OAuth / KMS / bucket (siguiente carril).
- iOS.
- Federated marketplace (Fase 13) — no se cierra la puerta; identidad Google es
  un ancla de usuario, el DID de nodo sigue siendo del ERP.
