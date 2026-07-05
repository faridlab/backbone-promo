-- Down: drop promo.promo_bundles table
DROP TABLE IF EXISTS promo.promo_bundles CASCADE;
DROP FUNCTION IF EXISTS promo.promo_bundles_audit_timestamp() CASCADE;
