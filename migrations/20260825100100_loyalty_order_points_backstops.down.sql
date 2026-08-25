-- Down: remove the reversal invariants and the RLS fence from promo.loyalty_order_points.

DROP POLICY IF EXISTS loyalty_order_points_company_isolation ON promo.loyalty_order_points;
ALTER TABLE promo.loyalty_order_points NO FORCE ROW LEVEL SECURITY;
ALTER TABLE promo.loyalty_order_points DISABLE ROW LEVEL SECURITY;

ALTER TABLE promo.loyalty_order_points DROP CONSTRAINT IF EXISTS loyalty_order_points_records_a_movement;
ALTER TABLE promo.loyalty_order_points DROP CONSTRAINT IF EXISTS loyalty_order_points_spend_reversal_bounded;
ALTER TABLE promo.loyalty_order_points DROP CONSTRAINT IF EXISTS loyalty_order_points_grant_reversal_bounded;
