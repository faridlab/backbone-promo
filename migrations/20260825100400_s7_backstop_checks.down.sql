-- Down: drop the coupon-cap and window-ordering backstop CHECKs.

ALTER TABLE promo.promo_bundles DROP CONSTRAINT IF EXISTS promo_bundles_window_ordered;
ALTER TABLE promo.pricing_rules DROP CONSTRAINT IF EXISTS pricing_rules_window_ordered;
ALTER TABLE promo.loyalty_programs DROP CONSTRAINT IF EXISTS loyalty_programs_window_ordered;
ALTER TABLE promo.coupon_codes DROP CONSTRAINT IF EXISTS coupon_codes_window_ordered;
ALTER TABLE promo.coupon_codes DROP CONSTRAINT IF EXISTS coupon_codes_used_within_max;
