-- Down: drop promo.loyalty_order_points table
DROP TABLE IF EXISTS promo.loyalty_order_points CASCADE;
DROP FUNCTION IF EXISTS promo.loyalty_order_points_audit_timestamp() CASCADE;
