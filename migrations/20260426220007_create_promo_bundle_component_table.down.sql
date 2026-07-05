-- Down: drop promo.promo_bundle_components table
DROP TABLE IF EXISTS promo.promo_bundle_components CASCADE;
DROP FUNCTION IF EXISTS promo.promo_bundle_components_audit_timestamp() CASCADE;
