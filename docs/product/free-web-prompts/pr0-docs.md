# PR0 — Docs lock (RutBusiness Free Web)

You are a principal engineer on **RutBusiness** (repo `pharma-server` — historical name):
Rust ERP, offline-first, freemium. Mission: free Shopify-grade storefront seam —
catalog + pickup orders served by the same offline ERP. This session ships **docs only**
(1 commit). Do not write code. Do not read other planning docs.

## Setup (PowerShell)

```powershell
cd "D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-server"
git fetch origin
# one lane worktree for the whole Free Web effort (skip add if it already exists):
git worktree add "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web" -b feature/free-web-shopify-parity origin/feature/erp-parity
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web"
```

## Verified facts (2026-07-20 — trust these, no re-derivation)

- ADRs exist through `docs/adr/0015-*.md` → **this ADR is 0016** (re-check `ls docs/adr` before writing; bump if taken).
- ADR-0014 (`0014-dss-storefront-integration.md`) covers the HTTP seam — keep seam, supersede any "web = paid" freemium clause (old plan called it ADR-0014 freemium; verify which ADR states web is paid via `rg -l "web" docs/adr` and cite precisely).
- Free-tier invariants live in `docs/adr/0005-core-gratis-no-locked-in.md`.

## Deliverables (2 files + 1 commit)

**1. `docs/adr/0016-free-web-as-core.md`** — status Accepted. Content:
- Decision: Free tier includes 1 public web storefront (catalog + pickup orders), served from the local ERP through the public HTTP seam. Free web is **ungated** (no 402 on catalog/order basics).
- Supersedes: any prior clause making basic web presence a paid feature (cite exact ADR/section found above). Keeps: seam architecture (API keys server-side, tunnel WAN).
- Paid remains: custom domain, advanced branding, online card payments, multi-site, marketing automation (`web.custom_domain`, `web.branding_advanced`, `web.payments_online`, `web.marketing_automation`).
- Invariants: public web opt-in (`web.published` default off → public 404); POS/ERP works offline forever; stock+money truth = ERP; prices decimal strings; `cost_price` never leaves the server.

**2. `docs/strategy/free-web-shopify-parity.md`** — short (≤120 lines):
- One-line mission + persona (Sandra: 1 local, WhatsApp CRM, pickup > courier, 15-min publish or fail).
- Gap table Shopify vs RutBusiness Free (catalog, cart, checkout=pickup, payments=POS/at-counter, domain=paid, themes=1).
- Free vs Paid split (table above).
- Build queue pointer: `docs/product/free-web-prompts/README.md` (PR1 catalog → PR2 keys → PR3 orders → PR4 tooling → PR5 storefront).
- Metrics: time-to-publish <15 min; first web order same day; zero oversell.

## Ship

```powershell
git add docs/adr/0016-free-web-as-core.md docs/strategy/free-web-shopify-parity.md
git commit -m "docs(web): ADR-0016 free web as core + shopify-parity strategy"
git push -u origin feature/free-web-shopify-parity
```

Append one line to `bitacora.md` (## section for 2026 entries, same commit or follow-up):
`- 2026-07-XX: PR0 Free Web — ADR-0016 (free web core) + strategy doc. Lane feature/free-web-shopify-parity.`

Done = 2 files committed + pushed. Print: `✅ PR0 LISTO — docs pushed · next: pr1-public-catalog.md in fresh session`.
