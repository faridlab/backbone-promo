-- Migration: Add threshold-shape fields to pricing_rules
--
-- Two additive, nullable columns (null = feature off, existing behavior unchanged):
--   min_order_qty  — cart-wide item-count floor for scope=order rules (today only the
--                    min_order_amount Rp floor exists). "buy >= N items cart-wide".
--   discount_upto  — optional Rp ceiling on the discount a rule grants ("10% off, max 50k").
--
-- See docs/adr/ADR-003-promo-type-is-structural-not-stored.md (capability-gap closure).

ALTER TABLE promo.pricing_rules
  ADD COLUMN IF NOT EXISTS min_order_qty NUMERIC(18, 4) CHECK (min_order_qty >= 0),
  ADD COLUMN IF NOT EXISTS discount_upto NUMERIC(18, 2) CHECK (discount_upto >= 0);
