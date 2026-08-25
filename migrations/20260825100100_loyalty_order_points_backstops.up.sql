-- Cross-column backstops for promo.loyalty_order_points.
--
-- The schema DSL cannot express cross-column constraints, so the three reversal invariants ride
-- this hand migration:
--   * a grant clawback can never exceed what was granted;
--   * a spend restoration can never exceed what was spent;
--   * a row must record at least one movement (no empty shells) — every INSERT the write service
--     issues carries at least one non-zero leg, so this holds by construction.
--
-- Together with the conservation trigger on the entry ledger these are the DB-level bounds for the
-- per-order points accounting: over-reversal is refused here even if a caller bypasses the verbs.
--
-- RLS posture (ADR-0014 strict, same shape as the 20260819 fence migration): the table is
-- company-scoped; a session sees only rows whose company_id equals the request-scoped company
-- (`set_config('app.company_id', <uuid>, true)`); an unset var sees zero rows (fail-closed).

ALTER TABLE promo.loyalty_order_points
    ADD CONSTRAINT loyalty_order_points_grant_reversal_bounded
    CHECK (granted_reversed_points <= granted_points);

ALTER TABLE promo.loyalty_order_points
    ADD CONSTRAINT loyalty_order_points_spend_reversal_bounded
    CHECK (spent_reversed_points <= spent_points);

ALTER TABLE promo.loyalty_order_points
    ADD CONSTRAINT loyalty_order_points_records_a_movement
    CHECK (NOT (granted_points = 0 AND spent_points = 0));

ALTER TABLE promo.loyalty_order_points ENABLE ROW LEVEL SECURITY;
ALTER TABLE promo.loyalty_order_points FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS loyalty_order_points_company_isolation ON promo.loyalty_order_points;
CREATE POLICY loyalty_order_points_company_isolation ON promo.loyalty_order_points
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
