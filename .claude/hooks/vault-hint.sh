#!/usr/bin/env bash
# SessionStart hook: detect recent file activity, suggest relevant vault refs.
# Output → stdout as additionalContext for Claude (token-cheap pointer, not full reference).

set -euo pipefail
cd "$(dirname "$0")/../.." 2>/dev/null || exit 0

VAULT="C:/Users/Administrator/Documents/obsidian-mind"

# Files modified in working tree + last 5 commits (uncached, tiny git op)
files=$(
  { git diff --name-only HEAD 2>/dev/null
    git diff --name-only --cached 2>/dev/null
    git log -5 --name-only --pretty=format: 2>/dev/null
  } | sort -u
)

[ -z "$files" ] && exit 0

hints=""
add() { hints="$hints\n- $1"; }

echo "$files" | grep -qE '^crates/db/|^migrations/|\.surql$'                    && add "Tocando DB/migraciones → leer \`$VAULT/reference/pharma-server-db.md\`"
echo "$files" | grep -qE '^crates/api/.*\.rs$|/middleware/|/routes\.rs$'         && add "Tocando API axum → leer \`$VAULT/reference/pharma-server-api.md\`"
echo "$files" | grep -qE '^crates/cli/'                                          && add "Tocando CLI → leer \`$VAULT/reference/pharma-server-cli.md\`"
echo "$files" | grep -qE '^crates/service/|windows-service'                      && add "Tocando service → leer \`$VAULT/reference/pharma-server-msi.md\` + \`$VAULT/brain/pharma-server-gotchas.md\`"
echo "$files" | grep -qE '^installer/|\.wxs$|\.wixproj$'                         && add "Tocando installer/MSI → leer \`$VAULT/reference/pharma-server-msi.md\`"
echo "$files" | grep -qE '^\.github/workflows/'                                  && add "Tocando CI → leer \`$VAULT/reference/pharma-server-ci.md\`"
echo "$files" | grep -qE '^config/.*\.toml$|rust-toolchain\.toml|\.env'          && add "Tocando config/env → leer \`$VAULT/reference/pharma-server-env.md\`"
echo "$files" | grep -qE '^crates/auth/|jsonwebtoken|argon2'                     && add "Tocando auth → revisar \`$VAULT/brain/pharma-server-gotchas.md\` (sección JWT/issuer)"
echo "$files" | grep -qE '^Cargo\.toml$|^crates/[^/]+/Cargo\.toml$'              && add "Tocando deps/workspace → leer \`$VAULT/reference/pharma-server-architecture.md\`"

[ -z "$hints" ] && exit 0

printf "Vault hints (archivos cambiados recientemente):%b\n" "$hints"
