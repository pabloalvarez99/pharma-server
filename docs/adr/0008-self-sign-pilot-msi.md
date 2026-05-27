# ADR-0008: Self-sign pilot MSI antes de cert EV / Azure Trusted Signing

- **Status**: Accepted
- **Date**: 2026-05-27
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: distribución, msi, presupuesto, fase-9

## Context and Problem Statement

`bitacora.md` (estado actual 2026-05-27) marca **Fase 9 vendible v1.0.0 BLOQUEADA por
cert Authenticode**. Sin firma, Windows muestra SmartScreen warning ("Windows protegió
su PC… Más información → Ejecutar de todas formas") al instalar el MSI. La regla #9 de
`CLAUDE.md` exige:

> deploy auto = MSI release al mirror público **una vez que**: cert Authenticode válido
> cargado + smoke install VM verde + sin bugs P0 abiertos en triage.

El cert Authenticode tiene costo real:

| Opción | Costo | Onboarding | SmartScreen |
|---|---|---|---|
| Certum Open Source CS | ~$80 USD/año | 1-2 semanas KYC | Reputación se gana con descargas |
| Sectigo / DigiCert OV | ~$300 USD/año | 1-2 semanas KYC | Reputación se gana con descargas |
| Sectigo / DigiCert EV | ~$400-600 USD/año | 2-4 semanas + USB HSM | Cero warning desde día 0 |
| Azure Trusted Signing | $9.99 USD/mes (~$120/año) | <1 día (cuenta Azure) | Cero warning, integra CI |
| SignPath Foundation OSS | $0 | Requiere repo público | Cero warning (cubre certs públicos) |
| Microsoft Store MSIX | $19 USD one-time | <1 día (cuenta dev) | Cero warning, MS firma |
| Self-signed PowerShell | $0 | <5 min local | Warning + import manual cert |

Bloqueos derivados del **dinero**:

1. Cualquier opción ≥$80/año **antes de tener primer cliente** = pérdida pura. No hay
   funnel de ingresos todavía (Fase 11 license-server no existe).
2. SignPath Foundation OSS exige repo público → choca con **regla #10** (repo source
   PRIVADO).
3. Microsoft Store MSIX requiere repackage MSI → MSIX (manifest distinto, herramientas
   distintas) + cuenta dev pagada $19 → barato, pero requiere primer cliente que
   justifique el gasto.

El fundador pidió explícitamente **camino $0** hasta primer cobro (mensaje
2026-05-27: *"no se puede de alguna forma avanzar gratis?"*).

## Decision Drivers

- **Costo inicial $0** hasta primer cliente real.
- **Capacidad de distribuir MSI hoy** a 3-10 farmacias piloto hand-holding.
- **No violar regla #10** (repo source PRIVADO).
- **Camino de upgrade claro** cuando entren ingresos.
- **Tiempo a primer install**: minutos, no semanas.
- **No comprometer la regla #9** (smoke VM verde + cert firmado siguen siendo prerrequisitos
  para deploy automático al mirror público; este ADR sólo cambia *qué cert*).

## Considered Options

1. **Comprar cert EV ahora** ($400-600/año, 2-4 semanas) — bloquea velocidad por presupuesto.
2. **Azure Trusted Signing** ($120/año + CI-friendly) — más barato, sin USB, pero
   sigue siendo costo upfront sin ingresos.
3. **SignPath Foundation OSS** ($0) — choca con regla #10 (repo público).
4. **Self-signed PowerShell + hand-holding piloto** ($0) — fricción por SmartScreen,
   pero viable para <20 farmacias con onboarding asistido. Camino de upgrade staged
   (MSIX MS Store $19 al primer cliente → Azure Trusted Signing $10/mes a escala).
5. **MSIX Microsoft Store directamente** ($19 one-time, casi-cero) — requiere repackage
   + manifest Windows Service distinto + cuenta dev. Salto técnico.

## Decision Outcome

**Elegida: Opción 4 (self-signed + onboarding asistido) con upgrade staged**.

### Staging

| Hito | Cert | Costo | Cuándo |
|---|---|---|---|
| **Fase 9 piloto** | Self-signed PowerShell `pilot.pfx` | $0 | HOY (<20 farmacias hand-holding) |
| **Primer cliente real** | Microsoft Store MSIX (repackage) | $19 USD one-time | Al cerrar primera venta Pro/Business |
| **≥10 clientes pagos** | Azure Trusted Signing | $9.99 USD/mes | Cuando soporte manual no escale |
| **Producto mainstream** | Cert EV USB HSM | $400-600 USD/año | Cuando reputación importe en lanzamiento masivo |

### Self-signed details (Fase 9 piloto)

- Cert generado con `New-SelfSignedCertificate -Type CodeSigning -KeyAlgorithm RSA
  -KeyLength 2048 -KeyUsage DigitalSignature -KeyExportPolicy Exportable` (PowerShell
  Windows ≥10).
- Subject: `CN=Pharma Server Pilot, O=Pharma Server, C=CL`.
- Vigencia: 3 años (rotar antes).
- `.pfx` password vive en env `PHARMA_CERT_PASSWORD` (NUNCA en repo; junto a
  `config/local.toml` en gitignore).
- Public `.cer` SI puede committearse (es público por diseño) — cliente lo importa a
  Trusted Publishers store antes de instalar el MSI.
- Firma con `signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256
  *.msi`.
- Timestamp es crítico: sin él la firma expira con el cert (cliente no podrá reinstalar
  después de 3 años).

### Onboarding del cliente piloto (15 min asistido)

1. Cliente baja `.cer` desde mirror público (es público).
2. Doble click `.cer` → "Install Certificate" → Local Machine → "Place all certificates
   in the following store" → Browse → **Trusted Publishers** → OK.
3. Doble click `.msi`. Sin SmartScreen warning porque el cert está en Trusted Publishers
   de la máquina.
4. Agente del piloto (o documentación `installer/sign/README.md`) acompaña el flujo
   la primera vez.

Alternativa sin importar el cert: cliente acepta SmartScreen warning ("Más información
→ Ejecutar de todas formas"). Funciona pero requiere educación.

### Consequences

#### Positivas

- **Costo $0 hasta primer cobro**. Permite avanzar la regla #9 sin comprometer presupuesto.
- **MSI distribuible HOY**: el script `installer/sign/sign-msi.ps1` queda listo y CI
  puede consumirlo (env var del password en GitHub Actions secret).
- **Upgrade path** claro y escalonado: cada paso de cert se desbloquea con un ingreso
  proporcional.
- **No viola regla #10**: repo source sigue privado.

#### Negativas

- **No escala más allá de ~20 farmacias** (cada cliente nuevo = 15min onboarding asistido).
  Acción mitigante: cerrar primera venta lo más rápido posible para liberar $19 MSIX.
- **SmartScreen warning si el cliente no importa el cert** — fricción real. Mitigante:
  guía paso-a-paso en `installer/sign/README.md` + video Loom del fundador.
- **Reputación del cert NO se acumula** (self-signed no tiene Microsoft chain) — al
  upgradear a MSIX/EV en el futuro hay que **reonboardear** los pilotos para que confíen
  el nuevo cert. Aceptable por volumen bajo.
- **Mensajes "Windows no puede verificar el editor"** en algunos contextos (no en MSI
  per se si el cert está en Trusted Publishers, pero sí en `.exe` correr standalone).

#### Neutras

- El `pilot.pfx` queda como secret en GitHub Actions secrets. Rotarlo es trivial.
- La parte de **smoke install en VM** (regla #9 prerequisito 2) NO cambia con este ADR.
  Sigue siendo bloqueante y se atiende en paralelo con `installer/smoke/` (ADR fuera de
  scope; ver `docs/strategy/zero-cost-launch-plan.md` §3).

## Pros and Cons of the Options

### Opción 1: Cert EV ahora
- **Pros**: cero SmartScreen desde día 0, escalable a mainstream.
- **Cons**: ≥$400/año upfront sin ingresos. Onboarding 2-4 semanas KYC. USB HSM no
  CI-friendly (requiere self-hosted runner). Prematuro para piloto.

### Opción 2: Azure Trusted Signing ahora
- **Pros**: $10/mes (barato relativo), CI-friendly, sin SmartScreen.
- **Cons**: aún es costo recurrente sin ingresos. **Es la opción upgrade target**
  para >10 clientes — adelantarla quema runway sin justificación.

### Opción 3: SignPath Foundation OSS
- **Pros**: gratis.
- **Cons**: **violenta regla #10** (repo source PRIVADO). Descartado por incompatibilidad
  con política de distribución (ver regla #10 `CLAUDE.md`).

### Opción 4: Self-signed + onboarding piloto (elegida)
- **Pros**: ver "Decision Outcome > Positivas".
- **Cons**: ver "Decision Outcome > Negativas".

### Opción 5: MSIX MS Store directo
- **Pros**: $19 one-time, MS firma, sin SmartScreen.
- **Cons**: requiere repackage técnico (servicio Windows en MSIX necesita
  `<windows.service>` extension manifest — Win10 1709+). $19 al fundador SIN ingresos
  todavía. Mejor diferirlo a Hito 2.

## More Information

- [`docs/strategy/zero-cost-launch-plan.md`](../strategy/zero-cost-launch-plan.md) §2 — bloqueo cert y workaround.
- [`installer/sign/README.md`](../../installer/sign/README.md) — instrucciones operativas.
- [ADR-0009](./0009-pilot-payment-provider.md) — paralelo: payment provider para pilot phase.
- Regla #9 + Regla #10 `CLAUDE.md` — invariantes que este ADR respeta.
- `signtool.exe` docs: https://learn.microsoft.com/en-us/dotnet/framework/tools/signtool-exe
- Microsoft Store MSIX docs: https://learn.microsoft.com/en-us/windows/msix/
- Azure Trusted Signing: https://learn.microsoft.com/en-us/azure/trusted-signing/
