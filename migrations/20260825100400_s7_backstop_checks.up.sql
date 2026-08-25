-- Database backstops for the guarded write paths (defense-in-depth behind the verbs).
--
-- 1. Coupon cap: the guarded UPDATE on used_count is the runtime bound; this CHECK makes
--    used_count > max_use unrepresentable for ANY writer. Existing-row risk is unreachable via the
--    guarded write path (possible only via direct SQL). NOT VALID + VALIDATE in the same migration:
--    on a fresh/scratch database VALIDATE trivially passes; on an existing environment with drifted
--    rows VALIDATE fails AT DEPLOY TIME — the intended surfacing. Operator remediation (judgment
--    call, NOT auto-applied by this migration):
--      UPDATE promo.coupon_codes SET used_count = max_use
--       WHERE max_use IS NOT NULL AND used_count > max_use;
--
-- 2. Window ordering on the four windowed masters (a window that ends before it starts is
--    nonsensical and previously crashed nothing — it just never matched). Same NOT VALID + VALIDATE
--    posture and rationale as the coupon cap.

ALTER TABLE promo.coupon_codes
    ADD CONSTRAINT coupon_codes_used_within_max
    CHECK (max_use IS NULL OR used_count <= max_use) NOT VALID;
ALTER TABLE promo.coupon_codes VALIDATE CONSTRAINT coupon_codes_used_within_max;

ALTER TABLE promo.coupon_codes
    ADD CONSTRAINT coupon_codes_window_ordered
    CHECK (valid_upto IS NULL OR valid_from <= valid_upto) NOT VALID;
ALTER TABLE promo.coupon_codes VALIDATE CONSTRAINT coupon_codes_window_ordered;

ALTER TABLE promo.loyalty_programs
    ADD CONSTRAINT loyalty_programs_window_ordered
    CHECK (to_date IS NULL OR from_date <= to_date) NOT VALID;
ALTER TABLE promo.loyalty_programs VALIDATE CONSTRAINT loyalty_programs_window_ordered;

ALTER TABLE promo.pricing_rules
    ADD CONSTRAINT pricing_rules_window_ordered
    CHECK (valid_to IS NULL OR valid_from <= valid_to) NOT VALID;
ALTER TABLE promo.pricing_rules VALIDATE CONSTRAINT pricing_rules_window_ordered;

ALTER TABLE promo.promo_bundles
    ADD CONSTRAINT promo_bundles_window_ordered
    CHECK (valid_to IS NULL OR valid_from <= valid_to) NOT VALID;
ALTER TABLE promo.promo_bundles VALIDATE CONSTRAINT promo_bundles_window_ordered;
