-- Down: remove the company RLS fence for promo module

-- Reverse the company RLS fence for promo.coupon_codes
DROP POLICY IF EXISTS coupon_codes_company_isolation ON promo.coupon_codes;
ALTER TABLE promo.coupon_codes NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.coupon_codes DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for promo.coupon_redemptions
DROP POLICY IF EXISTS coupon_redemptions_company_isolation ON promo.coupon_redemptions;
ALTER TABLE promo.coupon_redemptions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.coupon_redemptions DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for promo.loyalty_programs
DROP POLICY IF EXISTS loyalty_programs_company_isolation ON promo.loyalty_programs;
ALTER TABLE promo.loyalty_programs NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.loyalty_programs DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for promo.loyalty_point_entries
DROP POLICY IF EXISTS loyalty_point_entries_company_isolation ON promo.loyalty_point_entries;
ALTER TABLE promo.loyalty_point_entries NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.loyalty_point_entries DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for promo.pricing_rules
DROP POLICY IF EXISTS pricing_rules_company_isolation ON promo.pricing_rules;
ALTER TABLE promo.pricing_rules NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.pricing_rules DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for promo.promo_bundles
DROP POLICY IF EXISTS promo_bundles_company_isolation ON promo.promo_bundles;
ALTER TABLE promo.promo_bundles NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.promo_bundles DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for promo.promo_bundle_components
DROP POLICY IF EXISTS promo_bundle_components_company_isolation ON promo.promo_bundle_components;
ALTER TABLE promo.promo_bundle_components NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.promo_bundle_components DISABLE ROW LEVEL SECURITY;

