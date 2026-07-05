-- Down: drop promo.coupon_codes table
DROP TABLE IF EXISTS promo.coupon_codes CASCADE;
DROP FUNCTION IF EXISTS promo.coupon_codes_audit_timestamp() CASCADE;
