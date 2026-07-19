# Stale / superseded branches (audit, 2026-05-24)

Read-only audit triggered by 5-agent integration analysis. No branches were
deleted, pushed, or modified. Operator decides cleanup.

Status legend:
- **DROP** — fully redundant; safe to remove locally once operator confirms.
- **ALIAS** — exact-SHA twin of another branch; one name is enough.
- **ANCESTOR** — strictly older commit on the same line; superseded by tip.
- **MISLABEL** — branch name does not match the commit it points at.
- **PARTIAL** — claimed to roll up a cascade but does not; keep with caveat.

## Verdict table

| Branch | SHA | Status | Replacement | Evidence |
|---|---|---|---|---|
| `feat/dte-fase-9-1` | `9d3aa5a` | ANCESTOR / DROP | `feat/dte-9-1-abc-xml-ted-caf` (`bf39b68`) | `git merge-base --is-ancestor feat/dte-fase-9-1 feat/dte-9-1-abc-xml-ted-caf` → ancestor=YES. Tip subject: "Fase 9.1 arranque — skeleton crates/dte". |
| `feat/dte-9-1-d-e-sii-upload` | `1c20490` | MISLABEL / DROP | `feat/dte-9-1-d-e-sii-upload-v2` (`0f55861`) | NOT ancestor of v2. Tip subject is actually "9.1.i — cert encrypt-at-rest (AES-256-GCM + argon2id)" — name says d+e upload, commit is i cert. v2 carries the real d+e SII upload. |
| `feat/dte-cli` | `996fd42` | ALIAS | `feat/dte-9-1-d-e-i-sii-cert` (same SHA); k-cli content lives on `feat/dte-9-1-k-cli` (`fda7b98`) | `git rev-parse` identical. Subject: "9.1.d+e SII upload/polling + XML DSig sign" — note this branch is NOT k-cli content despite the name. Not an ancestor of `feat/dte-9-1-k-cli`. |
| `feat/dte-9-1-d-e-i-sii-cert` | `996fd42` | ALIAS | `feat/dte-cli` (same SHA) — pick one canonical name and drop the other | Same SHA as above; `git diff` between them is empty. |
| `feat/client-shell-tier-badge` | `c6229c9` | ALIAS / DROP | `feat/client-tauri-scaffold-v2` (`c6229c9`) | Identical SHA. Subject: "Tauri 2 scaffold — LoL-style login -> shell". Tier-badge work never landed on this ref. |
| `feat/client-functional-views` | `0fb6f1d` | ANCESTOR / DROP | `feat/client-pos-reports` (`9c6220e`) | Ancestry check → YES. Subject: "Tu Farmacia branding — logo, wordmark, teal". |
| `feat/client-login-polish` | `aee0e7a` | ANCESTOR / DROP (or keep if you want the scaffold→polish runbook) | `feat/client-pos-reports` (`9c6220e`) | Ancestry check → YES. Subject: "brand AppShell — Tu Farmacia wordmark". |
| `feat/api-audit-log-query` | `dcdcdb8` | SUPERSEDED / DROP | `feat/api-audit-log-query-v2` (`32d915e`) | NOT a direct ancestor of v2 (v2 is on a different lineage that includes the merged main with license-reload). v1 tip is `fix(api): audit-log tenant filter binding`. v2 is the canonical filterable+paginated endpoint and is the SHA aliased by `feat/audit-log-before-after`. |
| `feat/audit-log-before-after` | `32d915e` | ALIAS | `feat/api-audit-log-query-v2` (same SHA) | Identical SHA; empty diff. |
| `feat/dte-9-1-lm-tests-docs` | `b2376ad` | PARTIAL — keep, do **not** treat as cascade roll-up | use the individual feature branches: `feat/dte-9-1-abc-xml-ted-caf`, `feat/dte-9-1-d-e-sii-upload-v2`, `feat/dte-9-1-fgh-cancel-libro-xz`, `feat/dte-9-1-j-tier-gating`, `feat/dte-9-1-k-cli` | `git diff --name-only feat/dte-9-1-d-e-sii-upload-v2..feat/dte-9-1-lm-tests-docs` → empty. So `lm-tests-docs` content = abc + upload-v2 only. Misses cancel/libro, tier gating, and k-cli. NOT a roll-up. |

## Couldn't verify / caveats

- `feat/client-shell-tier-badge` being a dup is mechanically true (identical
  SHA). I did not verify whether there is *unpushed tier-badge work* sitting in
  a stash, worktree, or reflog elsewhere; before deleting confirm with
  `git reflog feat/client-shell-tier-badge`.
- `feat/api-audit-log-query` vs `-v2`: v1 is not an ancestor of v2 (different
  base). I confirmed v1 is the older "single fix commit" variant and v2 is the
  consolidated feature; both still have unique content relative to `main`.
  Operator should diff v1 against v2 manually if there is any worry that v1
  carries a fix not present in v2.
- `feat/dte-9-1-lm-tests-docs`: the agent claim that it "misses
  cancel/reports/gating/cli" is confirmed — diff against `-d-e-sii-upload-v2`
  is empty. The branch name implies an l+m tests/docs layer that does not
  exist in the tree.

## Suggested follow-up (human-driven, none autonomous)

1. **Commit this report** (no `-D` yet):
   ```
   git add docs/ops/stale-branches.md
   git commit -m "docs(ops): stale-branches audit 2026-05-24"
   ```
2. **Optional local cleanup** (after operator review, one-by-one, NEVER batch):
   ```
   git branch -D feat/dte-fase-9-1
   git branch -D feat/dte-9-1-d-e-sii-upload
   git branch -D feat/dte-9-1-d-e-i-sii-cert    # or drop feat/dte-cli, pick one
   git branch -D feat/client-shell-tier-badge
   git branch -D feat/client-functional-views
   git branch -D feat/client-login-polish
   git branch -D feat/api-audit-log-query
   git branch -D feat/audit-log-before-after
   ```
3. **DO NOT** `git push origin --delete ...` for any of these without an
   explicit ask from the founder. Remote deletes are irreversible and these
   branches may still be referenced by open PRs, draft PRs, or external
   review tooling.
4. Re-label rather than delete if the branch name carries product meaning
   (e.g. `feat/dte-cli` could be renamed to `feat/dte-9-1-d-e-sii-upload-v3`
   to match its actual content) — but renaming + force-pushing is also
   destructive; queue for operator.
