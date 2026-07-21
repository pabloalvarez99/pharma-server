# SaaS Web prompt pack — cold-session execution

**Uso: pegar exactamente UN archivo como mensaje 1 de una sesión FRESCA (`/clear` antes).**
Cada prompt es autocontenido: hechos verificados 2026-07-21 baked. La sesión NO lee la
spec maestra (`../../superpowers/specs/2026-07-21-saas-web-cloud-design.md`) ni re-deriva
el repo. Decir "execute" y construye, gatea, shipea.

| # | Archivo | Construye | Deps | Tamaño est. |
|---|---------|-----------|------|-------------|
| 1 | `sp1-server-linux-vm.md` | `pharma-api` Linux corriendo en GCE VM + Caddy TLS + systemd + backup GCS + script deploy | — | L |
| 2 | `sp2-provisioning-api.md` | `POST /admin/v1/tenants` con secret + tests + test aislamiento cross-tenant | 1 (deploy), código sin dep | L |
| 3 | `sp3-web-client.md` | Shim invoke→fetch (73 cmds) + build web PWA de `client/src` | 1 para probar en vivo; código sin dep | L |
| 4 | `sp4-signup-landing.md` | Form signup en license-server + email verify + CTA landing "Usar gratis en web" | 2 | M |

Orden: 1 → 2 → 3 → 4. SP2 y SP3 paralelizables (repos/paths disjuntos: SP2 crates/, SP3 client/).

**Lane:** SP1-SP3 committean a branch `feature/saas-web`
(worktree `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\saas-web`, base
`origin/feature/erp-parity`, spec ya committeada `0ecea8d`). SP1 abre el PR GitHub;
siguientes sesiones pushean commits al mismo PR. SP4 va al repo hermano
`pharma-license-server` con su propia branch/PR. Founder mergea cuando quiera.

Decisiones lockeadas (founder 2026-07-21, NO re-preguntar): SaaS cloud completo ·
Free tier también en cloud · GCE e2-micro free tier en proyecto GCP **NUEVO**
(`rutbusiness-cloud`; NUNCA `tu-farmacia-prod`) · v1 = signup + ERP core (sin pago
cloud, sin migración MSI↔cloud, sin offline web).
