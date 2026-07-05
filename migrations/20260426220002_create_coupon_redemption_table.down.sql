-- Down: drop promo.coupon_redemptions table
DROP TABLE IF EXISTS promo.coupon_redemptions CASCADE;
DROP FUNCTION IF EXISTS promo.coupon_redemptions_audit_timestamp() CASCADE;
