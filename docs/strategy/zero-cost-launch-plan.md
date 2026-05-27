---
title: Zero-cost launch plan — pilot a primer cobro sin gastar
status: Lockeado v1
date: 2026-05-27
owners: pabloalvarez99 (fundador)
supersedes:
  - "Nada — convive con freemium-master-plan.md y payments-cl.md (los implementa al modo $0)"
related_adrs:
  - ADR-0001 (pivote freemium)
  - ADR-0003 (Webpay-first, diferido en pilot por ADR-0009)
  - ADR-0008 (self-sign pilot MSI)
  - ADR-0009 (MP + Stripe antes de Webpay en pilot)
last_review: 2026-05-27
---

# Zero-cost launch plan — pilot a primer cobro sin gastar

> **Plan operativo lockeado.** Este es el documento que un agente nuevo lee primero para
> entender qué hay que ejecutar y por qué. Cualquier desviación requiere ADR. El
> objetivo es **0 USD gastados** hasta el primer cobro real de una farmacia piloto.

---

## 1. TL;DR

3 bloqueos cierran el deploy del MSI vendible (regla #9 `CLAUDE.md`):

1. **Cert Authenticode** (anti-SmartScreen) — antes costaba $80-600/año.
2. **Smoke install en VM Windows limpia** — antes costaba 0 si tienes VM.
3. **Cobrar (Fase 11)** — antes costaba 2-4 semanas Webpay + RUT empresa.

Cada uno tiene workaround $0 ejecutable hoy:

1. → **Self-signed cert PowerShell** + onboarding asistido <20 pilotos
   ([ADR-0008](../adr/0008-self-sign-pilot-msi.md)).
2. → **Hyper-V + Windows 11 Dev VM** (image MS gratis 90 días, snapshot baseline,
   re-imagen on demand).
3. → **Mercado Pago Chile + Stripe Checkout** ($0 onboarding, RUT persona natural OK),
   `pharma-license-server` en Vercel Hobby + Neon free tier
   ([ADR-0009](../adr/0009-pilot-payment-provider.md) +
   [`license-server-skeleton.md`](./license-server-skeleton.md)).

**Upgrade staged a costo real cuando entren ingresos**:

| Hito | Inversión | Habilita |
|---|---|---|
| Hoy | $0 | Distribuir MSI piloto, cobrar microtx vía MP/Stripe |
| Primer cliente | $19 MS Store dev | MSIX firmado oficial (cero SmartScreen) |
| $300/mes MRR | $10/mes Azure Trusted Signing | CI firma automática, escala distribución |
| SpA constituida | Webpay onboarding (~$0-100 + 2-4 sem) | Sub recurrente B2B con brand local |
| Producto mainstream | $400-600/año cert EV | Reputación pública máxima |

---

## 2. Bloqueo 1 — Cert Authenticode

### 2.1 Estado actual

- `installer/wix/main.wxs` produce `.msi` sin firmar.
- `release-publisher.yml` workflow no incluye paso `signtool`.
- Cliente final ve SmartScreen warning al hacer doble click.

### 2.2 Workaround $0 (pilot)

[ADR-0008](../adr/0008-self-sign-pilot-msi.md) decide: **self-signed cert PowerShell**.

**Pasos ejecutables** (todos automatizados en `installer/sign/`):

1. **Generar cert** (1 vez por máquina dev):
   ```powershell
   $env:PHARMA_CERT_PASSWORD = "<password-fuerte>"
   pwsh installer/sign/generate-pilot-cert.ps1
   ```
   Output: `installer/sign/pilot.pfx` (gitignored, secret) + `installer/sign/pilot.cer`
   (public, distribuible).

2. **Firmar MSI** (cada release):
   ```powershell
   $env:PHARMA_CERT_PASSWORD = "<password-fuerte>"
   pwsh installer/sign/sign-msi.ps1 -MsiPath target/wix/pharma-server-0.1.25-x86_64.msi
   ```
   Aplica timestamp `http://timestamp.digicert.com` (gratis, sin cuenta) y verifica
   firma post-firma con `signtool verify /pa`.

3. **Cliente piloto importa cert** (15 min asistido la primera vez):
   - Baja `pilot.cer` del mirror público (o el agente del piloto lo entrega).
   - Doble click → Install Certificate → Local Machine → Trusted Publishers.
   - Después doble click MSI sin warning.

### 2.3 Cuándo subir de nivel

| Trigger | Acción |
|---|---|
| Primera venta cerrada | Pagar $19 MS Store dev → repackage MSI → MSIX → publicar (acepta sideload + Store) |
| Onboarding manual >20 clientes | Migrar a Azure Trusted Signing $10/mes → automatizar en `release-publisher.yml` |
| Lanzamiento masivo planeado | Comprar cert EV USB HSM ($400-600/año) o seguir en Azure Trusted Signing si CI-friendly importa más |

---

## 3. Bloqueo 2 — Smoke install VM

### 3.1 Estado actual

Regla #9 exige smoke verde antes de deploy automático. `installer/wix/main.wxs` está
verificado en dry-run (`cargo wix --no-build`) pero **nunca se instaló en VM limpia**.

### 3.2 Workaround $0

**Componentes gratis**:

- **Windows 11 Pro** del fundador trae **Hyper-V** built-in (Enterprise/Pro/Education).
  Habilitar con:
  ```powershell
  Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All
  ```
  (Restart requerido. Verificar virtualization on en BIOS.)
- **Windows 11 Dev VM** image oficial Microsoft: https://aka.ms/windev — gratis, 90
  días, viene con Hyper-V/VirtualBox/VMware/Parallels formats. Re-imagen cada 90d.
- **Snapshot baseline** después de boot + Windows Update + reboot = "clean Win11" — se
  reverta entre smoke runs.

**Pasos ejecutables** (todos automatizados en `installer/smoke/`):

1. **Setup VM** (1 vez):
   ```powershell
   pwsh installer/smoke/setup-vm.ps1 -VmName PharmaSmoke -IsoPath C:/path/to/Win11Dev.iso
   ```
   Crea VM Hyper-V, monta ISO, espera boot inicial, toma snapshot `baseline`.

2. **Run smoke** (cada release candidata):
   ```powershell
   pwsh installer/smoke/run-smoke.ps1 -VmName PharmaSmoke -MsiPath target/wix/pharma-server-0.1.25-x86_64.msi
   ```
   - Revierte VM a snapshot `baseline`.
   - Copia MSI a VM vía `Copy-VMFile`.
   - Ejecuta `installer/smoke/smoke-install.ps1` dentro de la VM via `Invoke-Command`.
   - Verifica: install exit 0 → service `PharmaServer` Running → `GET /health` 200 →
     uninstall exit 0 → service gone.
   - Retorna exit 0 si todo verde, !=0 si falla.

3. **CI optional** (Fase 11 cuando MSRR justifique runner): self-hosted Windows runner
   con Hyper-V nested → smoke en cada PR.

### 3.3 Cuándo subir de nivel

| Trigger | Acción |
|---|---|
| Más de 1 release por semana | Self-hosted Windows runner con Hyper-V nested o Azure VM dedicada |
| Necesidad CI cloud | Azure DevTest Lab (~$10-30/mes según uso) o GitHub Actions windows-latest (limited a nested) |

---

## 4. Bloqueo 3 — Cobrar (Fase 11)

### 4.1 Estado actual

- `crates/license` (Fase 10) ya firma/verifica licenses offline (Ed25519). PR #47 mergeado.
- `crates/cli` tiene `pharma license import|status|features|verify|export|clear`.
- **No existe** el license-server real — el binario hoy carga `pilot.pfx` placeholder
  vía `License::free_default()` si no hay archivo.
- **No existe** rail de pago integrado.

### 4.2 Workaround $0

[ADR-0009](../adr/0009-pilot-payment-provider.md) decide: **Mercado Pago Chile + Stripe**
antes de Webpay. Repo separado `pharma-license-server` (ver
[ADR-0004](../adr/0004-license-server-separado.md)) implementa:

- **Vercel Hobby (free)** — hosting Next.js.
- **Neon free tier** — Postgres 0.5GB (~50k licenses cabe holgado).
- **Resend free tier** — envío email license `.json` al cliente (100 emails/día).
- **Mercado Pago Checkout Pro** — $0 onboarding, RUT persona natural OK.
- **Stripe Checkout** — backup para tarjeta internacional.
- **Ed25519 priv key** vive sólo en Vercel env var (cifrado at rest); pub key
  embebida en binario pharma-server (`crates/license/src/keys.rs`).

Blueprint completo de implementación: [`license-server-skeleton.md`](./license-server-skeleton.md).

### 4.3 Pasos ejecutables (próxima semana)

1. **Crear repo `pharma-license-server`** (separado, ADR-0004). Cuenta GitHub
   `pabloalvarez99`, repo privado.
2. **Bootstrap Next.js 14 App Router** + Postgres schema + Stripe + MP webhooks.
   Ver `license-server-skeleton.md` §4 para skeleton checklist.
3. **Generar keypair Ed25519** localmente:
   ```bash
   openssl genpkey -algorithm ed25519 -out license-signer.pem
   openssl pkey -in license-signer.pem -pubout -out license-signer.pub
   ```
4. **Embed pub key** en `crates/license/src/keys.rs` (reemplaza placeholder).
5. **Deploy Vercel** + provisionar Neon + Stripe test keys + MP test keys.
6. **Landing `/buy`** con 4 botones (Free, Pro $X/mes, Business $Y/mes, Microtx $Z).
7. **Webhook handler** → emite license JSON firmado → email Resend al cliente.

### 4.4 Cuándo subir de nivel

| Trigger | Acción |
|---|---|
| Primer ingreso real | Pagar dominio custom $12/año (Cloudflare Registrar = at-cost) |
| Más de 50k licenses | Upgrade Neon Pro ($19/mes) |
| Necesidad Webpay (B2B confianza) | Constituir SpA → onboarding Transbank → integrar `WebpayProvider` en license-server |
| Más de 100 emails/día | Resend pro ($20/mes) o Mailgun |

---

## 5. Camino crítico día-a-día

```
DÍA 0 (HOY)
├── [✅ HECHO]    Scripts installer/sign + installer/smoke escritos
├── [✅ HECHO]    ADR-0008 + ADR-0009 escritos
├── [✅ HECHO]    Blueprint license-server-skeleton.md escrito
└── [✅ HECHO]    Bitácora + vault + memoria actualizados

DÍA 1
├── Generar pilot.pfx local + commit pilot.cer + .gitignore pilot.pfx
├── Habilitar Hyper-V Windows 11 Pro del fundador
└── Bajar Windows 11 Dev VM image MS oficial

DÍA 2
├── Crear VM Hyper-V baseline + snapshot
├── Build MSI 0.1.25 local + firmar con sign-msi.ps1
└── Run smoke contra VM → verde

DÍA 3
├── Publicar MSI 0.1.25 firmado self-sign al mirror `pharma-server-releases` (workflow dispatch)
├── Subir pilot.cer al mirror público (release asset)
└── Probar instalación end-to-end desde laptop limpia (proxy de farmacia piloto)

DÍA 4-7 (paralelo a outreach piloto)
├── Crear repo pharma-license-server
├── Bootstrap Next.js + Neon + Stripe test + MP test
├── Embed pub key en crates/license
└── Deploy Vercel + landing /buy + webhooks

DÍA 8-14 (outreach piloto)
├── Identificar 3-5 farmacias en Coquimbo/La Serena
├── Demo + install asistido + onboard cert
└── Primer microtx pagado → unlock $19 MS Store → migrar a MSIX
```

Ningún paso del DÍA 0-7 requiere salir de pharma-server local + cuenta GitHub +
Vercel/Neon/Stripe/MP free tiers. **Costo total runway hasta primer cobro: $0**.

---

## 6. Métricas de éxito

| Métrica | Target piloto | Cuándo |
|---|---|---|
| Pilotos instalados (asistido) | 3-5 | Semana 2 |
| Primer microtx cobrado | 1 | Semana 4 |
| Pilotos retenidos >30 días | ≥60% | Semana 8 |
| Conversión Free → Pro/Business | 1+ | Semana 12 |
| MRR > $100/mes | 1+ cliente Pro | Semana 16 |

Por debajo de targets sostenido → revisar **producto** (no presupuesto): qué falta para
que pilotos paguen.

---

## 7. Riesgos $0

| Riesgo | Severidad | Mitigación |
|---|---|---|
| Self-signed cert fricciona piloto | Alta | Onboarding asistido + video Loom + guía paso-a-paso |
| Hyper-V no habilita en hardware fundador | Media | Fallback VirtualBox (gratis, multi-platform) |
| MP CL rechaza cuenta persona natural | Baja | Stripe como backup → Khipu como tercer plano |
| Vercel Hobby limit 100GB bandwidth | Baja | No-issue early (license JSON ~2KB cada uno) |
| Neon free 0.5GB se llena | Baja | ~50k licenses cabe; upgrade $19/mes cuando llegue |
| Resend 100 emails/día se queda corto | Media | Stripe/MP webhook tienen retry — re-emitir on demand. Migrar a Mailgun si frecuente. |
| Primer cliente no quiere self-sign | Alta | Acelerar MSIX MS Store ($19 sale del primer cobro o del fundador como inversión) |

---

## 8. Handoff a agentes nuevos

**Si llegas en sesión nueva y NO sabes qué hacer aquí, lee en este orden**:

1. Este documento (zero-cost-launch-plan.md) — qué + por qué + cuándo.
2. `bitacora.md` § ESTADO ACTUAL + § BACKLOG — qué hay y qué falta.
3. [ADR-0008](../adr/0008-self-sign-pilot-msi.md) — política cert.
4. [ADR-0009](../adr/0009-pilot-payment-provider.md) — política pagos pilot.
5. [`license-server-skeleton.md`](./license-server-skeleton.md) — cómo bootstrap el
   repo separado.
6. `installer/sign/README.md` — operativa de firma.
7. `installer/smoke/README.md` — operativa de smoke VM.

**Si el fundador dice "continúa con el plan zero-cost"** → ejecutar siguiente paso pendiente
del §5 día-a-día arriba. No re-discutir presupuesto sin razón nueva (presupuesto = $0
hasta primer cobro, decisión lockeada).

**Si el fundador dice "compré cert/MSIX/Azure Trusted Signing"** → upgradear según §2.3 y
actualizar este documento + ADR-0008 status a "Superseded by ADR-0010 (Azure Trusted
Signing)" cuando corresponda.

**Si el fundador dice "constituí SpA"** → reactivar [ADR-0003](../adr/0003-payments-webpay-first.md)
secuencia original (Webpay primero) y mover MP+Stripe a roles secundarios. Marcar
ADR-0009 como "Superseded" en ese momento.

---

## 9. Lo que NO está en este plan (alcance)

- **Marketing pagado** (ads, SEO, content) — pilot phase es outreach 1-a-1.
- **Cloud companion** (Fase 14) — diferido hasta tener señal de demanda real.
- **Marketplace federado B2B** (Fase 13) — diferido hasta ≥100 instalaciones.
- **Constituir SpA / RUT empresa** — decisión del fundador con asesoría contable;
  no es prerrequisito de pilot phase.
- **DTE Fase 9.1** (boleta SII automatizada) — sigue en paralelo per
  [ADR-0011](../adr/0011-dte-provider-native-rust.md). NO bloquea pilot (las farmacias
  pueden seguir usando boletas manuales hasta que Fase 9.1 cierre y el feature gate
  abra como microtx).

## More information

- [`docs/strategy/freemium-master-plan.md`](./freemium-master-plan.md) — modelo de negocio
  completo.
- [`docs/strategy/payments-cl.md`](./payments-cl.md) — comparativa providers (referenciado).
- [`docs/strategy/license-architecture.md`](./license-architecture.md) — arquitectura del
  license layer (Fase 10 + 11).
- [`installer/sign/README.md`](../../installer/sign/README.md) — operativa.
- [`installer/smoke/README.md`](../../installer/smoke/README.md) — operativa.
- Regla #9 + #10 `CLAUDE.md` — invariantes que este plan respeta.
