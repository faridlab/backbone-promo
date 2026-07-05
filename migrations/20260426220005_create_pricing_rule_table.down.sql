-- Down: drop promo.pricing_rules table
DROP TABLE IF EXISTS promo.pricing_rules CASCADE;
DROP FUNCTION IF EXISTS promo.pricing_rules_audit_timestamp() CASCADE;
