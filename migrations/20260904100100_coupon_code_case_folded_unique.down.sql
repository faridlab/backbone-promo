-- Down: drop the case-folded coupon-code uniqueness (the exact-case partial
-- unique index (company_id, code) remains).

DROP INDEX IF EXISTS promo.idx_coupon_codes_company_id_code_casefold;
