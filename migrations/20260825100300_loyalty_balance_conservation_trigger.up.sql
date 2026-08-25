-- Loyalty balance conservation backstop.
--
-- A CONSTRAINT TRIGGER (not a CHECK on a materialized balance): a CHECK cannot aggregate, so a
-- materialized-balance design would need an app/trigger-maintained counter column plus a
-- balance >= 0 CHECK — the same trigger-class hot-path cost PLUS permanent drift risk. Recomputing
-- the true SUM has zero drift. Cost: one aggregate over the member's (company, customer, program)
-- partition — a member's few dozen rows — per entry WRITE; reads (the actual hot path) never touch
-- it.
--
-- The conserved quantity is the EXPIRY-AWARE available balance, agreeing with the redeem-time
-- semantics the service enforces (expiry_date filters at read): a clawback that lapsed points
-- cannot cover is refused here just as it is bounded in the verbs.
--
-- Known gaps (deliberate, defense-in-depth posture):
--   * soft-delete (a metadata-only UPDATE) bypasses the trigger — the ledger is append-only by
--     contract and soft-delete is not a write-path operation;
--   * concurrent UNSERIALIZED direct writes could race the check — the per-member anchor lock is
--     the primary mechanism; this trigger is the backstop, not an adversarial barrier.

CREATE OR REPLACE FUNCTION promo.assert_loyalty_balance_non_negative() RETURNS trigger AS $$
DECLARE
    bal NUMERIC;
BEGIN
    SELECT COALESCE(SUM(points) FILTER (WHERE expiry_date IS NULL OR expiry_date > now()), 0)
      INTO bal
      FROM promo.loyalty_point_entries
     WHERE company_id = NEW.company_id
       AND customer_id = NEW.customer_id
       AND loyalty_program_id = NEW.loyalty_program_id
       AND (metadata->>'deleted_at') IS NULL;

    IF bal < 0 THEN
        RAISE EXCEPTION 'loyalty balance would go negative for member % under program %',
            NEW.customer_id, NEW.loyalty_program_id
            USING ERRCODE = 'P0001';
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER loyalty_balance_conservation
AFTER INSERT OR UPDATE OF points, expiry_date ON promo.loyalty_point_entries
DEFERRABLE INITIALLY IMMEDIATE
FOR EACH ROW EXECUTE FUNCTION promo.assert_loyalty_balance_non_negative();
