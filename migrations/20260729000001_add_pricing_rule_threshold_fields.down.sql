-- Revert: drop the threshold-shape fields added in the companion up migration.

ALTER TABLE promo.pricing_rules
  DROP COLUMN IF EXISTS min_order_qty,
  DROP COLUMN IF EXISTS discount_upto;
