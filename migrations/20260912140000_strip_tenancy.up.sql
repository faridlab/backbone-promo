-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the promo tables (ADR-0029): the module is
-- tenant-agnostic; org scoping is installed by the COMPOSING service's tenancy decorator,
-- never by the module. Dropped here, per table: the company-leading uniques and indexes,
-- the <table>_company_isolation RLS policy, and the company_id column itself.
--
-- The org-scoped re-declarations move to the composing service's tenancy decorator:
--   - coupon_codes: (org_unit_id, code) and the case-folded (org_unit_id, UPPER(code));
--   - coupon_redemptions: (org_unit_id, coupon_id, source_type, source_id);
--   - loyalty_point_entries: (org_unit_id, source_type, source_id, entry_type);
--   - loyalty_order_points: (org_unit_id, loyalty_program_id, order_ref_type, order_ref_id);
--   - the plain company-leading lookup indexes (status/priority/customer/item) return as
--     org-scoped twins there too.
-- Module-owned uniques with no company axis stay module-owned. The one-active-claim-per-cart
-- partial unique WAS company-leading and drops with the column; it is re-declared tenant-
-- agnostic at the coupon_claims block below (a cart lives in exactly one org unit, so the
-- guarantee survives the re-key without a tenant axis).
--
-- Ordering guard (the decorator must run FIRST on any database with data): the module
-- never moves tenancy data. A table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from company_id —
--      or b) it is empty (a fresh database: the earlier chain files created it empty).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping a column
-- that still holds the only tenancy key. The file is re-runnable (every drop is IF EXISTS),
-- so a failed run retries cleanly after the decorator lands.
--
-- RLS flags are deliberately NOT touched: all eleven tables carry ENABLE and FORCE from
-- earlier chain files, and both flags stay armed — zero policies means default-deny for
-- every role until the composing service's decorator installs the org-unit policy set.

DO $$
DECLARE
    t text;
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offenders text := '';
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'coupon_claims', 'coupon_codes', 'coupon_redemptions',
        'loyalty_member_anchors', 'loyalty_order_points', 'loyalty_point_entries',
        'loyalty_programs', 'pricing_rules', 'promo_bundle_components',
        'promo_bundle_gifts', 'promo_bundles'
    ]
    LOOP
        IF to_regclass(format('promo.%I', t)) IS NULL THEN
            CONTINUE; -- chain not fully applied on this database; nothing to strip
        END IF;

        SELECT EXISTS (
                   SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'promo' AND table_name = t AND column_name = 'org_unit_id'
               )
        INTO has_org;

        EXECUTE format('SELECT count(*) FROM promo.%I', t) INTO total;

        IF has_org THEN
            EXECUTE format(
                'SELECT count(*) FROM promo.%I WHERE org_unit_id IS NULL', t)
            INTO org_nulls;
        ELSE
            org_nulls := total; -- no org column: every row's only tenancy key is company_id
        END IF;

        IF has_org AND org_nulls = 0 THEN
            CONTINUE; -- decorator backfilled: safe
        END IF;
        IF total = 0 THEN
            CONTINUE; -- empty table (fresh database): safe
        END IF;
        offenders := offenders || format(' promo.%s (%s rows, %s rows not covered by org_unit_id);', t, total, org_nulls);
    END LOOP;

    IF offenders <> '' THEN
        RAISE EXCEPTION 'refusing to strip company_id — these tables are not yet covered by the tenancy decorator:%. Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', offenders;
    END IF;
END $$;

-- ── coupon_claims ───────────────────────────────────────────────────────────────
DROP POLICY IF EXISTS coupon_claims_company_isolation ON promo.coupon_claims;
ALTER TABLE promo.coupon_claims DROP COLUMN IF EXISTS company_id;
-- The one-active-claim-per-cart partial unique was company-leading, so the column drop above
-- took it with the column. A cart lives in exactly one org unit, so the guarantee needs no
-- tenant axis — it is re-declared here module-owned and tenant-agnostic (ADR-0029):
CREATE UNIQUE INDEX IF NOT EXISTS idx_coupon_claims_one_active_per_cart
    ON promo.coupon_claims (cart_ref_type, cart_ref_id)
    WHERE status = 'claimed';

-- ── coupon_codes ────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_coupon_codes_company_id_code;
DROP INDEX IF EXISTS promo.idx_coupon_codes_company_id_code_casefold;
DROP INDEX IF EXISTS promo.idx_coupon_codes_company_id_is_active;
DROP INDEX IF EXISTS promo.idx_coupon_codes_company_id_status;
DROP POLICY IF EXISTS coupon_codes_company_isolation ON promo.coupon_codes;
ALTER TABLE promo.coupon_codes DROP COLUMN IF EXISTS company_id;

-- ── coupon_redemptions ──────────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_coupon_redemptions_company_id_coupon_id_source_type_source_id;
DROP POLICY IF EXISTS coupon_redemptions_company_isolation ON promo.coupon_redemptions;
ALTER TABLE promo.coupon_redemptions DROP COLUMN IF EXISTS company_id;

-- ── loyalty_member_anchors ──────────────────────────────────────────────────────
ALTER TABLE promo.loyalty_member_anchors DROP CONSTRAINT IF EXISTS loyalty_member_anchors_company_id_customer_id_loyalty_program_id_key;
DROP POLICY IF EXISTS loyalty_member_anchors_company_isolation ON promo.loyalty_member_anchors;
ALTER TABLE promo.loyalty_member_anchors DROP COLUMN IF EXISTS company_id;

-- ── loyalty_order_points ────────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_loyalty_order_points_company_id_loyalty_program_id_order_ref_type_order_ref_id;
DROP INDEX IF EXISTS promo.idx_loyalty_order_points_company_id_customer_id;
DROP POLICY IF EXISTS loyalty_order_points_company_isolation ON promo.loyalty_order_points;
ALTER TABLE promo.loyalty_order_points DROP COLUMN IF EXISTS company_id;

-- ── loyalty_point_entries ───────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_loyalty_point_entries_company_id_source_type_source_id_entry_type;
DROP INDEX IF EXISTS promo.idx_loyalty_point_entries_company_id_customer_id_loyalty_program_id;
DROP POLICY IF EXISTS loyalty_point_entries_company_isolation ON promo.loyalty_point_entries;
ALTER TABLE promo.loyalty_point_entries DROP COLUMN IF EXISTS company_id;
-- The balance-conservation backstop read the dropped column (PL/pgSQL binds late, so the
-- column drop did not fail). Re-keyed to the member partition the anchor mutex now uses —
-- (customer, program) — same shape the company arm gave it before the strip:
CREATE OR REPLACE FUNCTION promo.assert_loyalty_balance_non_negative() RETURNS trigger AS $$
DECLARE
    bal NUMERIC;
BEGIN
    SELECT COALESCE(SUM(points) FILTER (WHERE expiry_date IS NULL OR expiry_date > now()), 0)
      INTO bal
      FROM promo.loyalty_point_entries
     WHERE customer_id = NEW.customer_id
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

-- ── loyalty_programs ────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_loyalty_programs_company_id_is_active;
DROP INDEX IF EXISTS promo.idx_loyalty_programs_company_id_status;
DROP POLICY IF EXISTS loyalty_programs_company_isolation ON promo.loyalty_programs;
ALTER TABLE promo.loyalty_programs DROP COLUMN IF EXISTS company_id;

-- ── pricing_rules ───────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_pricing_rules_company_id_is_active_apply_on;
DROP INDEX IF EXISTS promo.idx_pricing_rules_company_id_item_id_is_active;
DROP INDEX IF EXISTS promo.idx_pricing_rules_company_id_item_id_status;
DROP INDEX IF EXISTS promo.idx_pricing_rules_company_id_priority;
DROP INDEX IF EXISTS promo.idx_pricing_rules_company_id_status_apply_on;
DROP POLICY IF EXISTS pricing_rules_company_isolation ON promo.pricing_rules;
ALTER TABLE promo.pricing_rules DROP COLUMN IF EXISTS company_id;

-- ── promo_bundle_components ─────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_promo_bundle_components_company_id_bundle_id;
DROP POLICY IF EXISTS promo_bundle_components_company_isolation ON promo.promo_bundle_components;
ALTER TABLE promo.promo_bundle_components DROP COLUMN IF EXISTS company_id;

-- ── promo_bundle_gifts ──────────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_promo_bundle_gifts_company_id_bundle_id;
DROP POLICY IF EXISTS promo_bundle_gifts_company_isolation ON promo.promo_bundle_gifts;
ALTER TABLE promo.promo_bundle_gifts DROP COLUMN IF EXISTS company_id;

-- ── promo_bundles ───────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS promo.idx_promo_bundles_company_id_is_active;
DROP INDEX IF EXISTS promo.idx_promo_bundles_company_id_priority;
DROP INDEX IF EXISTS promo.idx_promo_bundles_company_id_status;
DROP POLICY IF EXISTS promo_bundles_company_isolation ON promo.promo_bundles;
ALTER TABLE promo.promo_bundles DROP COLUMN IF EXISTS company_id;
