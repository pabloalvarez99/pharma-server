-- Migration 0017 — idempotency body fingerprint for `Idempotency-Key`
-- reuse detection (BUG-002).
--
-- Additive only. Adds `body_fingerprint` to `idempotency_key`: a stable
-- canonical-JSON serialization (sorted object keys) of the incoming POST body.
-- Computed by `domain::sales::repo::body_fingerprint` so:
--   * same body → same string → cached replay (existing behavior);
--   * SAME key + DIFFERENT body → 409 reuse-conflict instead of silent replay.
--
-- Bodies are small (POS carts ~1-2KB); storing the canonical bytes verbatim
-- avoids pulling a SHA dep (constraint: no `Cargo.toml` edits in this work)
-- AND removes collision risk vs. a fixed-width digest. Column name keeps the
-- semantic "fingerprint of the body".
--
-- Rows persisted before this migration carry NULL `body_fingerprint` and
-- behave like today (replay on key match) so idempotency_key rows mid-flight
-- on upgrade stay valid until their 24h TTL expires.

DEFINE FIELD body_fingerprint ON idempotency_key TYPE option<string>;
