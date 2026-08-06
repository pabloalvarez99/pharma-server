# Free Web prompt pack — cold-session execution

**Usage: paste exactly ONE file below as message 1 of a FRESH session (`/clear` first).**
Each prompt is self-contained: verified repo facts (2026-07-20) baked in — the session
does NOT read the master doc (`../free-web-services-shopify-parity-prompt.md`, now appendix)
and does NOT re-derive the repo. Say "execute" and it builds, gates, ships.

| # | File | Builds | Deps | Est. session size |
|---|------|--------|------|-------------------|
| 0 | `pr0-docs.md` | ADR-0016 + strategy gap doc | — | S |
| 1 | `pr1-public-catalog.md` | mig 0018 + public catalog read API + tests | 0 (soft) | L |
| 2 | `pr2-api-keys.md` | mig 0019 web_api_key + public auth middleware + admin keys CRUD | 1 | L |
| 3 | `pr3-web-orders.md` | mig 0020 + pickup order create (idempotency+HMAC+stock reserve) | 2 | L |
| 4 | `pr4-tooling.md` | scripts/web-sync node clients + interop doc | 3 | M |
| 5 | `pr5-storefront.md` | Next 14 storefront beachhead (separate folder/repo) | 3 | L |
| 6 | `pr6-operator.md` | operator flow verify + docs | 3 | S |
| 7 | `pr7-license.md` | paid `web.*` keys in pharma-license-server | — | S |
| 8 | `pr8-polish.md` | SEO + demo recording + bitácora milestone | 5 | M |
| 10 | `pr10-landing-checkout.md` | Landing RutBusiness + rediseño checkout (license-server) | 9 (lane cerrado) | M |

Order: 0→1→2→3 (core value, one lane branch, one growing PR). 4–8 after. 7 anytime.

**Lane:** all server PRs commit to branch `feature/free-web-shopify-parity`
(worktree `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web`, base
`origin/feature/erp-parity`). PR1 session opens the GitHub PR; later sessions push
commits to the same PR. Founder merges when ready.

Changes vs master doc (ground truth 2026-07-20): next free ADR = **0016** (not 0018);
keys PR moved **before** orders PR (no dev-token hack); error code reuses existing
`INSUFFICIENT_STOCK` (not `STOCK_INSUFFICIENT`); public reads keyless behind
`web.published` 404-darkness (tenant slug in path); pagination = `limit`+`offset`
(not cursor); `order` table already had `reserved`/`store`/`transfer`/customer fields.
