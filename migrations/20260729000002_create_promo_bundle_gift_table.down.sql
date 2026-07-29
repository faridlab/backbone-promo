-- Down: drop promo.promo_bundle_gifts table
DROP TABLE IF EXISTS promo.promo_bundle_gifts CASCADE;
DROP FUNCTION IF EXISTS promo.promo_bundle_gifts_audit_timestamp() CASCADE;
