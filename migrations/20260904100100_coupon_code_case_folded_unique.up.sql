-- Case-folded uniqueness for promo coupon codes (DB-backed, replacing the
-- case-handling-by-convention posture).
--
-- The exact-case partial unique index (company_id, code) already exists. Lookup
-- normalizes the typed code to UPPER before matching, and writers are expected
-- to store codes upper-cased — but until now nothing in the DATABASE pinned
-- that convention, so 'SAVE10' and 'save10' could coexist in one company: the
-- lower-case twin is unreachable through the normalized lookup path and a
-- second, differently-cased copy of the same shopper-visible code can be minted
-- by any writer that skips the convention. This expression index makes
-- case-folded uniqueness unrepresentable-to-violate for ANY writer.
--
-- Postgres cannot create a UNIQUE index NOT VALID, so on a database that already
-- holds mixed-case twins this fails AT DEPLOY TIME — the intended surfacing.
-- Operator remediation (judgment call, NOT auto-applied here): pick the
-- canonical (upper-cased) row per (company, UPPER(code)), retire the twins.
--
-- Concurrent-create safety: two simultaneous CREATEs of the same code now race
-- on this index instead of racing a check-then-act search — the losing insert
-- gets a 23505 at COMMIT, exactly the DB-backed shape the module's other
-- uniqueness keys (redemption ledger, loyalty entries, claim table) already
-- have.

CREATE UNIQUE INDEX IF NOT EXISTS idx_coupon_codes_company_id_code_casefold
    ON promo.coupon_codes (company_id, UPPER(code))
    WHERE (metadata->>'deleted_at') IS NULL;
