-- Migration: replace the four promo lifecycle booleans with status enums
-- coupon_codes, loyalty_programs, pricing_rules and promo_bundles each carried
-- `is_active BOOLEAN NOT NULL DEFAULT TRUE`; the tree-wide convention is one
-- `status` enum field per lifecycle (see docs/refactoring-schema in the serpa
-- workspace). Each boolean migrates only rows deviating from its own column
-- default. The enum types are created unqualified so they land beside the
-- module's other enum types (public), where the generated sqlx type_name
-- resolves. The old is_active indexes die with their column (Postgres drops
-- indexes that reference a dropped column); status indexes take their place.

DO $$ BEGIN
    CREATE TYPE coupon_code_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    CREATE TYPE loyalty_program_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    CREATE TYPE pricing_rule_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    CREATE TYPE promo_bundle_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

ALTER TABLE promo.coupon_codes ADD COLUMN status coupon_code_status NOT NULL DEFAULT 'active';
UPDATE promo.coupon_codes SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE promo.coupon_codes DROP COLUMN is_active;

ALTER TABLE promo.loyalty_programs ADD COLUMN status loyalty_program_status NOT NULL DEFAULT 'active';
UPDATE promo.loyalty_programs SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE promo.loyalty_programs DROP COLUMN is_active;

ALTER TABLE promo.pricing_rules ADD COLUMN status pricing_rule_status NOT NULL DEFAULT 'active';
UPDATE promo.pricing_rules SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE promo.pricing_rules DROP COLUMN is_active;

ALTER TABLE promo.promo_bundles ADD COLUMN status promo_bundle_status NOT NULL DEFAULT 'active';
UPDATE promo.promo_bundles SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE promo.promo_bundles DROP COLUMN is_active;

CREATE INDEX IF NOT EXISTS idx_coupon_codes_company_id_status ON promo.coupon_codes (company_id, status);
CREATE INDEX IF NOT EXISTS idx_loyalty_programs_company_id_status ON promo.loyalty_programs (company_id, status);
CREATE INDEX IF NOT EXISTS idx_pricing_rules_company_id_status_apply_on ON promo.pricing_rules (company_id, status, apply_on);
CREATE INDEX IF NOT EXISTS idx_pricing_rules_company_id_item_id_status ON promo.pricing_rules (company_id, item_id, status);
CREATE INDEX IF NOT EXISTS idx_promo_bundles_company_id_status ON promo.promo_bundles (company_id, status);
