-- Down: restore the four is_active booleans exactly as they were.
-- Only 'inactive' rows are written back as FALSE; rows at the column default
-- map to the boolean default TRUE without an UPDATE. The status indexes die
-- with their column; the original is_active indexes are recreated by name.

ALTER TABLE promo.coupon_codes ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE promo.coupon_codes SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE promo.coupon_codes DROP COLUMN status;

ALTER TABLE promo.loyalty_programs ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE promo.loyalty_programs SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE promo.loyalty_programs DROP COLUMN status;

ALTER TABLE promo.pricing_rules ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE promo.pricing_rules SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE promo.pricing_rules DROP COLUMN status;

ALTER TABLE promo.promo_bundles ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE promo.promo_bundles SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE promo.promo_bundles DROP COLUMN status;

CREATE INDEX IF NOT EXISTS idx_coupon_codes_company_id_is_active ON promo.coupon_codes (company_id, is_active);
CREATE INDEX IF NOT EXISTS idx_loyalty_programs_company_id_is_active ON promo.loyalty_programs (company_id, is_active);
CREATE INDEX IF NOT EXISTS idx_pricing_rules_company_id_is_active_apply_on ON promo.pricing_rules (company_id, is_active, apply_on);
CREATE INDEX IF NOT EXISTS idx_pricing_rules_company_id_item_id_is_active ON promo.pricing_rules (company_id, item_id, is_active);
CREATE INDEX IF NOT EXISTS idx_promo_bundles_company_id_is_active ON promo.promo_bundles (company_id, is_active);

DROP TYPE IF EXISTS coupon_code_status;
DROP TYPE IF EXISTS loyalty_program_status;
DROP TYPE IF EXISTS pricing_rule_status;
DROP TYPE IF EXISTS promo_bundle_status;
