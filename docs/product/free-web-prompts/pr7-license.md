# PR7 — License matrix: paid web.* keys (RutBusiness)

Engineer on **pharma-license-server** (SEPARATE repo, sibling folder
`D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-license-server`).
Free Web ships ungated (ADR-0016): catalog + pickup orders are FREE forever.
This session appends PAID growth features only. Small, surgical.

## Rules

- NEVER gate free catalog/orders on license. Only additive paid keys.
- Follow the existing entry shape in `src/lib/feature-catalog.ts` exactly (read 2–3
  existing entries first; match tier naming already used there).

## Deliverable

Append to `src/lib/feature-catalog.ts`:

| key | tier (match existing naming) | label es |
|---|---|---|
| `web.custom_domain` | pro | Dominio propio para tu web |
| `web.branding_advanced` | pro | Marca avanzada (colores, logo, tipografía) |
| `web.payments_online` | business | Pago online (Webpay) en tu web |
| `web.marketing_automation` | business | Automatización de marketing web |

Run the repo's checks (see its package.json: lint/test/build) → commit on a branch
`feat/web-paid-features`, push, PR:

```powershell
cd "D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-license-server"
git checkout -b feat/web-paid-features && git add -A
git commit -m "feat: paid web.* feature keys (free web stays ungated, ADR-0016 pharma-server)"
git push -u origin feat/web-paid-features && gh pr create --fill
```

Done → `✅ PR7 LISTO — PR abierto en pharma-license-server`.
