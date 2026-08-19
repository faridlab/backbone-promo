-- Revert the ADR-0014 strict fence re-statement for promo module.
-- The fence predates this migration (ADR-0008-era), so the honest reverse is to
-- re-state the same live policy, not to disarm the tables: a down that disabled RLS
-- would leave company data unfenced — a posture this module never had.

-- Re-state the pre-existing fence for promo.coupon_codes (identical policy; see header).
DROP POLICY IF EXISTS coupon_codes_company_isolation ON promo.coupon_codes;
CREATE POLICY coupon_codes_company_isolation ON promo.coupon_codes
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for promo.coupon_redemptions (identical policy; see header).
DROP POLICY IF EXISTS coupon_redemptions_company_isolation ON promo.coupon_redemptions;
CREATE POLICY coupon_redemptions_company_isolation ON promo.coupon_redemptions
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for promo.loyalty_point_entries (identical policy; see header).
DROP POLICY IF EXISTS loyalty_point_entries_company_isolation ON promo.loyalty_point_entries;
CREATE POLICY loyalty_point_entries_company_isolation ON promo.loyalty_point_entries
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for promo.loyalty_programs (identical policy; see header).
DROP POLICY IF EXISTS loyalty_programs_company_isolation ON promo.loyalty_programs;
CREATE POLICY loyalty_programs_company_isolation ON promo.loyalty_programs
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for promo.pricing_rules (identical policy; see header).
DROP POLICY IF EXISTS pricing_rules_company_isolation ON promo.pricing_rules;
CREATE POLICY pricing_rules_company_isolation ON promo.pricing_rules
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for promo.promo_bundle_components (identical policy; see header).
DROP POLICY IF EXISTS promo_bundle_components_company_isolation ON promo.promo_bundle_components;
CREATE POLICY promo_bundle_components_company_isolation ON promo.promo_bundle_components
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for promo.promo_bundle_gifts (identical policy; see header).
DROP POLICY IF EXISTS promo_bundle_gifts_company_isolation ON promo.promo_bundle_gifts;
CREATE POLICY promo_bundle_gifts_company_isolation ON promo.promo_bundle_gifts
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for promo.promo_bundles (identical policy; see header).
DROP POLICY IF EXISTS promo_bundles_company_isolation ON promo.promo_bundles;
CREATE POLICY promo_bundles_company_isolation ON promo.promo_bundles
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

