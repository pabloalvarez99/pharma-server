# Next session prompt — zero-cost launch (Fase 9 + 11) sin gastar

Copy/paste this into a fresh session.

---

```
ultrathink
continue working with team of agents

Project: C:\Users\Administrator\Documents\GitHub\pharma-server
Branch: integration/0.1.25

NORTH STAR THIS PHASE (founder directive 2026-05-27):
El producto core funciona end-to-end. Lo que falta para VENDER: (1) firmar el MSI,
(2) smoke install en VM limpia, (3) cobrar. Los 3 estaban "bloqueados por dinero".
El plan zero-cost los desbloquea con 0 USD gastados hasta el primer cobro.

READ FIRST (in order):
1. docs/strategy/zero-cost-launch-plan.md   <- single source of truth, §5 = camino día-a-día, §8 = handoff
2. docs/adr/0008-self-sign-pilot-msi.md     <- política cert ($0 self-sign -> $19 MSIX -> $10/mo Azure)
3. docs/adr/0009-pilot-payment-provider.md  <- MP = primer rail LIVE; Webpay ya en sandbox (live al constituir SpA)
4. docs/strategy/license-server-skeleton.md <- blueprint repo separado pharma-license-server
5. installer/sign/README.md + installer/smoke/README.md  <- operativa scripts
6. bitacora.md § ESTADO ACTUAL + entrada 2026-05-27

EXECUTE NEXT (zero-cost plan §5 día-a-día — pendientes, todos $0):
1. [DÍA 1] Generar cert pilot:
     $env:PHARMA_CERT_PASSWORD = "<strong>"
     pwsh installer/sign/generate-pilot-cert.ps1
   -> commit pilot.cer (público), pilot.pfx queda gitignored.
2. [DÍA 1] Habilitar Hyper-V (elevado, reboot):
     Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All
   -> bajar Win11 Dev ISO gratis https://aka.ms/windev
3. [DÍA 2] setup-vm.ps1 -> baseline snapshot. Build MSI 0.1.25 + sign-msi.ps1.
4. [DÍA 2] run-smoke.ps1 -> verde (install/health/uninstall).
5. [DÍA 3] Publicar MSI 0.1.25 self-signed al mirror pharma-server-releases (workflow_dispatch)
   + subir pilot.cer como release asset. NOTA: deploy al mirror = acción NO autónoma
   (regla #9/#10) -> PAUSAR + confirmar con fundador antes de dispatch.
6. [DÍA 4-7] license-server YA EXISTE (pharma-license-server, Fase 11b code-complete con
   Webpay sandbox, Prisma+Next14, PR #1 abierto). NO crear de cero. Cerrar deploy:
   vercel link + Neon free + prisma migrate + prodkey:seed + vercel --prod (todo $0,
   Webpay queda en TEST). GAP CRÍTICO: embeber prod key lk-prod-2026-01 en
   crates/license/src/keys.rs (hoy placeholder lk-dev-2026) — PR aparte + GATE; sin esto
   el binario no verifica licencias reales. Para cobro real sin SpA: implementar rail
   Mercado Pago (~1 día). Detalle: docs/strategy/license-server-skeleton.md.

GATE OBLIGATORIO antes de cualquier PR (regla #2/#9):
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
Verde -> commit + push + PR contra base correcta. NUNCA debilitar asserts.
NOTA: el trabajo de esta fase es mayormente docs + PowerShell scripts (no toca Rust),
pero igual correr GATE si tocas crates/ (ej. embed pubkey en crates/license).

NO AUTÓNOMO (pausar + confirmar): cortar MSI release al mirror público (regla #9),
hacer público el source (regla #10), force-push, destructivo. Push/PR a repo privado SÍ.

SECONDARY (integration/0.1.25 cleanup, aún pendiente de sesión previa):
- PR #78 abierta vs feature/erp-parity (87 commits). NO fast-forward erp-parity sin review humano.
- 10 PRs en triage (owner decision): #76 #68 #67 #66 #64 #63 #62 #61 #58 #56. No auto-merge.
- cargo audit baseline: RUSTSEC-2021-0046 "telemetry" = FALSO POSITIVO (colisión nombre);
  TODO renombrar crates/telemetry -> pharma-telemetry. Resto upstream-driven, known-known.

PROCESS RULES (no skip):
- Memory [[verify-agent-gate-claims]]: NUNCA confiar en notificación bg "exit 0" — leer
  CARGO_EXIT real en el output file. Re-grep sin truncar antes de declarar verde.
- git add -A BANEADO (memory [[add-A-banned-pharma-server]]): untracked tiene secrets +
  worktree gitlinks. Siempre git add <paths específicos>.
- Bitácora dual (regla #7): cada cambio significativo -> bitacora.md repo + vault
  obsidian-mind/work/active/pharma-server/bitacora.md + decisions-log-index.md.
```

---

## Notes for the human

- El plan zero-cost está 100% documentado. Un agente nuevo lee `zero-cost-launch-plan.md`
  y sabe exactamente qué ejecutar sin re-discutir presupuesto ($0 hasta primer cobro =
  decisión lockeada en ADR-0008/0009).
- Próximo paso manual tuyo: elegir un password fuerte para `pilot.pfx` y decidir cuándo
  bajar la Win11 Dev ISO (gratis). El resto los agentes lo ejecutan.
- Publicar MSI al mirror público y crear el repo license-server siguen requiriendo tu OK
  (no son acciones autónomas).
