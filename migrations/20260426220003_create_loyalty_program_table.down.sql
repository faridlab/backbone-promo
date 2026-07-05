-- Down: drop promo.loyalty_programs table
DROP TABLE IF EXISTS promo.loyalty_programs CASCADE;
DROP FUNCTION IF EXISTS promo.loyalty_programs_audit_timestamp() CASCADE;
