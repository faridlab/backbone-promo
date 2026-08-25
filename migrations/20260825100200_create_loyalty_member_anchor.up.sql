-- Per-member serialization anchor for the loyalty ledger.
--
-- WHY A TABLE: the member's balance is an aggregate SUM over promo.loyalty_point_entries, which
-- cannot take FOR UPDATE; locking only the member's EXISTING entry rows drops mutual exclusion for
-- row-less members and is phantom-open under concurrent earns. One anchor row per
-- (company, customer, program), locked FOR UPDATE NOWAIT, is the mutex that keeps exclusion for
-- members with no entries yet. This is lock machinery, not a domain entity — deliberately NOT a
-- schema model, so no CRUD surface is ever mounted on it.
--
-- DDL posture mirrors promo.loyalty_point_entries: no FK (the referenced parties are logical),
-- RLS ENABLE + FORCE with the ADR-0014 strict company predicate (same shape as the 20260819 fence
-- migration). The write service reaches it through
-- LoyaltyMemberAnchorRepository::ensure_and_lock: a speculative INSERT ... ON CONFLICT DO NOTHING
-- followed by SELECT ... FOR UPDATE NOWAIT on the caller's transaction.

CREATE TABLE IF NOT EXISTS promo.loyalty_member_anchors (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL,
    customer_id UUID NOT NULL,
    loyalty_program_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id),
    UNIQUE (company_id, customer_id, loyalty_program_id)
);

ALTER TABLE promo.loyalty_member_anchors ENABLE ROW LEVEL SECURITY;
ALTER TABLE promo.loyalty_member_anchors FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS loyalty_member_anchors_company_isolation ON promo.loyalty_member_anchors;
CREATE POLICY loyalty_member_anchors_company_isolation ON promo.loyalty_member_anchors
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
