-- Down: drop the loyalty balance conservation backstop.

DROP TRIGGER IF EXISTS loyalty_balance_conservation ON promo.loyalty_point_entries;
DROP FUNCTION IF EXISTS promo.assert_loyalty_balance_non_negative() CASCADE;
