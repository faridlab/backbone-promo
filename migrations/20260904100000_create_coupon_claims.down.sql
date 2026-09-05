-- Down: drop the cart-stage coupon claim state (claim-lifecycle machinery; no
-- schema model, no CRUD surface depends on it).

DROP POLICY IF EXISTS coupon_claims_company_isolation ON promo.coupon_claims;
DROP TABLE IF EXISTS promo.coupon_claims;
DROP TYPE IF EXISTS promo.coupon_claim_status;
