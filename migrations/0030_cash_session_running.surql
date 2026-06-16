-- pharma-server 0030: cash_register_session.cash_sales_running maintained
-- aggregate (BUG-perf-005 fix)
-- ---------------------------------------------------------------------------
-- The cierre/arqueo close computation summed the cash side of the shift with a
-- live aggregate over `order`:
--
--   SELECT math::sum(cash_amount) FROM order
--   WHERE tenant=$t AND payment_method IN ['pos_cash','pos_mixed']
--     AND status NOT IN ['refunded','cancelled']
--     AND created_at >= opened_at AND created_at <= now GROUP ALL
--
-- The planner serves it from `order_tenant_created` as a *range scan over the
-- whole shift window* (it can't use `order_tenant_payment` because the
-- predicate is `payment_method IN [..]`, not `=`), then fetches every matching
-- order doc to read `cash_amount`. Cost is O(orders in the shift): measured
-- cierre_caja_agg p50 ~30ms / p99 121.9ms @50k-catalog + a busy day's orders
-- (bob's pos_hotpath bench) — violating the <50ms POS p99 budget, and the only
-- POS read op two orders of magnitude over the others. A covering index does
-- NOT help: SurrealDB's "Iterate Index" still fetches the record for the
-- projection (verified empirically — identical timing with/without).
--
-- Fix: maintain the cash-sales total per session INCREMENTALLY on the
-- `cash_register_session` row, so close/arqueo read it as an O(1) field load.
--
-- Why a DEFINE EVENT and not a pre-computed table view (`DEFINE TABLE … AS
-- SELECT`): SurrealDB view maintenance mis-handles UPDATEs (a sale flipped to
-- `status='refunded'` would not be subtracted) — see migration 0029 and the
-- `surrealdb-view-update-gotcha` note. The event uses `$before`/`$after` so
-- every op — CREATE / UPDATE (incl. refund) / DELETE — applies an exact delta.
--
-- Semantics: byte-identical to the old scan for the only call pattern that
-- exists — compute_summary() on an OPEN session at `now` (both close_session
-- and arqueo pass `Utc::now()`). Each open session accrues the cash side of
-- every qualifying order whose `created_at >= opened_at`; a closed session stops
-- accruing (events only touch `status='open'` rows), which freezes its number
-- at close time — the correct accounting behaviour (a refund processed after a
-- drawer is closed must not retroactively change that shift's expected cash).
-- `order.created_at` is immutable (DEFAULT only, no VALUE), so the window guard
-- is stable across an order's later UPDATEs.
-- Append-only: never edit once applied.
-- ---------------------------------------------------------------------------

DEFINE FIELD cash_sales_running ON cash_register_session TYPE decimal DEFAULT 0dec;

-- Per-order delta applied to the open sessions whose window covers the order.
-- A qualifying order contributes its `cash_amount`; a non-qualifying one
-- contributes 0, so a sale that flips into/out of refunded/cancelled (or
-- changes cash_amount) applies the exact (after − before) difference. The
-- `IF delta != 0` guard skips card-only sales entirely (no write on the POS
-- hot path for non-cash orders).
DEFINE EVENT cash_sales_running_maint ON TABLE order
  WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN {
    LET $tenant  = IF $event = "DELETE" THEN $before.tenant ELSE $after.tenant END;
    LET $created = IF $event = "DELETE" THEN $before.created_at ELSE $after.created_at END;

    LET $bq = IF $event = "CREATE" THEN false
              ELSE ($before.payment_method IN ['pos_cash','pos_mixed']
                    AND $before.status NOT IN ['refunded','cancelled']) END;
    LET $bc = IF $bq THEN ($before.cash_amount ?? 0dec) ELSE 0dec END;

    LET $aq = IF $event = "DELETE" THEN false
              ELSE ($after.payment_method IN ['pos_cash','pos_mixed']
                    AND $after.status NOT IN ['refunded','cancelled']) END;
    LET $ac = IF $aq THEN ($after.cash_amount ?? 0dec) ELSE 0dec END;

    LET $delta = $ac - $bc;

    IF $delta != 0dec THEN
      UPDATE cash_register_session SET cash_sales_running += $delta
        WHERE tenant = $tenant AND status = 'open' AND opened_at <= $created
    END;
  };

-- Backfill: existing sessions get a correct value immediately (events only fire
-- on future order writes). Open sessions sum up to now; closed sessions sum up
-- to their close time so the stored field stays meaningful even though
-- compute_summary only reads open sessions. No-op on a fresh install.
FOR $s IN (SELECT id, tenant, opened_at, closed_at FROM cash_register_session) {
  LET $upper = $s.closed_at ?? time::now();
  LET $sum = (SELECT VALUE math::sum(cash_amount) FROM order
              WHERE tenant = $s.tenant
                AND payment_method IN ['pos_cash','pos_mixed']
                AND status NOT IN ['refunded','cancelled']
                AND created_at >= $s.opened_at
                AND created_at <= $upper
              GROUP ALL)[0];
  UPDATE $s.id SET cash_sales_running = ($sum ?? 0dec);
};
