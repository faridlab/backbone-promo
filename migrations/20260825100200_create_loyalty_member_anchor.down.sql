-- Down: drop the per-member loyalty lock anchor.

DROP POLICY IF EXISTS loyalty_member_anchors_company_isolation ON promo.loyalty_member_anchors;
DROP TABLE IF EXISTS promo.loyalty_member_anchors;
