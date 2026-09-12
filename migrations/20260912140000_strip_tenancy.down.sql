-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with the company-leading uniques and the company isolation policy shape, but restores NO
-- data — rows written after the strip (or after the decorator re-keyed them) carry
-- org_unit_id only. The composing service's tenancy decorator remains the live fence;
-- treat this down as a schema-shape sketch for archaeology, not a usable rollback.
--
-- The org-scoped re-declarations are NOT restored here either: they were never this
-- module's post-strip shape.

-- The tenant-agnostic re-declaration of the one-active-claim-per-cart partial unique (added
-- by the strip's up) goes back to nothing here — the down sketch restores the company axis,
-- under which the old unique was company-leading.
DROP INDEX IF EXISTS promo.idx_coupon_claims_one_active_per_cart;

ALTER TABLE promo.coupon_claims           ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.coupon_codes            ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.coupon_redemptions      ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.loyalty_member_anchors  ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.loyalty_order_points    ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.loyalty_point_entries   ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.loyalty_programs        ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.pricing_rules           ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.promo_bundle_components ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.promo_bundle_gifts      ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE promo.promo_bundles           ADD COLUMN IF NOT EXISTS company_id uuid;

ALTER TABLE promo.loyalty_member_anchors
    ADD CONSTRAINT loyalty_member_anchors_company_id_customer_id_loyalty_program_id_key
    UNIQUE (company_id, customer_id, loyalty_program_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_coupon_codes_company_id_code
    ON promo.coupon_codes (company_id, code) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_coupon_codes_company_id_code_casefold
    ON promo.coupon_codes (company_id, UPPER(code)) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_coupon_redemptions_company_id_coupon_id_source_type_source_id
    ON promo.coupon_redemptions (company_id, coupon_id, source_type, source_id) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_loyalty_point_entries_company_id_source_type_source_id_entry_type
    ON promo.loyalty_point_entries (company_id, source_type, source_id, entry_type) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_loyalty_order_points_company_id_loyalty_program_id_order_ref_type_order_ref_id
    ON promo.loyalty_order_points (company_id, loyalty_program_id, order_ref_type, order_ref_id) WHERE (metadata->>'deleted_at') IS NULL;

-- The balance-conservation backstop's company arm returns with the column (the strip's up
-- re-keyed it to the (customer, program) member partition).
CREATE OR REPLACE FUNCTION promo.assert_loyalty_balance_non_negative() RETURNS trigger AS $$
DECLARE
    bal NUMERIC;
BEGIN
    SELECT COALESCE(SUM(points) FILTER (WHERE expiry_date IS NULL OR expiry_date > now()), 0)
      INTO bal
      FROM promo.loyalty_point_entries
     WHERE company_id = NEW.company_id
       AND customer_id = NEW.customer_id
       AND loyalty_program_id = NEW.loyalty_program_id
       AND (metadata->>'deleted_at') IS NULL;

    IF bal < 0 THEN
        RAISE EXCEPTION 'loyalty balance would go negative for member % under program %',
            NEW.customer_id, NEW.loyalty_program_id
            USING ERRCODE = 'P0001';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE POLICY coupon_claims_company_isolation ON promo.coupon_claims
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY coupon_codes_company_isolation ON promo.coupon_codes
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY coupon_redemptions_company_isolation ON promo.coupon_redemptions
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY loyalty_member_anchors_company_isolation ON promo.loyalty_member_anchors
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY loyalty_order_points_company_isolation ON promo.loyalty_order_points
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY loyalty_point_entries_company_isolation ON promo.loyalty_point_entries
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY loyalty_programs_company_isolation ON promo.loyalty_programs
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY pricing_rules_company_isolation ON promo.pricing_rules
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY promo_bundle_components_company_isolation ON promo.promo_bundle_components
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY promo_bundle_gifts_company_isolation ON promo.promo_bundle_gifts
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
CREATE POLICY promo_bundles_company_isolation ON promo.promo_bundles
    FOR ALL USING (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
