-- Down: drop promo.loyalty_point_entries table
DROP TABLE IF EXISTS promo.loyalty_point_entries CASCADE;
DROP FUNCTION IF EXISTS promo.loyalty_point_entries_audit_timestamp() CASCADE;
