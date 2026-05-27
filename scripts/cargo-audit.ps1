# cargo-audit wrapper with project ignore list.
# Rationale per ignored advisory documented in audit.toml.
# Re-evaluate quarterly when bumping surrealdb / reqwest.

$ignores = @(
    "RUSTSEC-2021-0046",  # FP: name-collision with local crate `telemetry`
    "RUSTSEC-2023-0071",  # upstream: rsa Marvin via surrealdb
    "RUSTSEC-2026-0049",  # upstream: rustls-webpki via surrealdb/reqwest
    "RUSTSEC-2026-0098",
    "RUSTSEC-2026-0099",
    "RUSTSEC-2026-0104",
    "RUSTSEC-2023-0089",  # unmaintained: atomic-polyfill
    "RUSTSEC-2025-0141",  # unmaintained: bincode
    "RUSTSEC-2024-0436",  # unmaintained: paste
    "RUSTSEC-2025-0134",  # unmaintained: rustls-pemfile
    "RUSTSEC-2026-0002"   # unsound: lru IterMut
)

$ignoreArgs = @()
foreach ($id in $ignores) { $ignoreArgs += "--ignore"; $ignoreArgs += $id }

& cargo audit @ignoreArgs @args
exit $LASTEXITCODE
