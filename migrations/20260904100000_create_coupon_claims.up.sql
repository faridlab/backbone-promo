-- Cart-stage claim state for promo codes (the coupon-claim serialization substrate).
--
-- WHY THIS TABLE: the coupon write surface until now had exactly one verb — the
-- commit-time burn (commit_coupon_redemption). A shopper presenting a code on a
-- cart needs a RESERVATION adjudicated server-side at claim time (usage headroom
-- counted under the coupon row lock, one active claim per cart), so two carts
-- racing the last use of a capped code learn the outcome at CLAIM time, not at
-- checkout. The claim does NOT burn: used_count still advances only at commit.
-- Headroom = used_count + every 'claimed' row for the coupon, so a released or
-- settled claim stops consuming capacity the moment its status flips.
--
-- It is claim-lifecycle machinery driven exclusively by the claim/release verbs
-- and the burn's same-ref settle — deliberately NOT a schema model, so no CRUD
-- surface is ever mounted on it (the loyalty_member_anchors posture). DDL
-- mirrors that table: no hard FKs (the referenced parties are logical), RLS
-- ENABLE + FORCE with the strict company predicate (ADR-0014).
--
-- Backstops (the s7 posture — the verbs are the runtime bound, the schema makes
-- violations unrepresentable for ANY writer):
--   * one ACTIVE claim per cart: partial UNIQUE on (company, cart_ref) WHERE
--     status = 'claimed' — a second concurrent claim by the same cart is a
--     23505, not a silent overwrite.
--   * settled_at consistency: 'claimed' rows carry NULL settled_at; every other
--     status stamps it.

CREATE TYPE promo.coupon_claim_status AS ENUM ('claimed', 'released', 'redeemed');

CREATE TABLE IF NOT EXISTS promo.coupon_claims (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL,
    cart_ref_type TEXT NOT NULL,
    cart_ref_id UUID NOT NULL,
    coupon_id UUID NOT NULL,
    code TEXT NOT NULL,
    pricing_rule_id UUID NOT NULL,
    status promo.coupon_claim_status NOT NULL DEFAULT 'claimed',
    claimed_at TIMESTAMPTZ NOT NULL,
    settled_at TIMESTAMPTZ,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_coupon_claims_one_active_per_cart
    ON promo.coupon_claims (company_id, cart_ref_type, cart_ref_id)
    WHERE status = 'claimed';

CREATE INDEX IF NOT EXISTS idx_coupon_claims_coupon_status
    ON promo.coupon_claims (company_id, coupon_id, status);

ALTER TABLE promo.coupon_claims
    ADD CONSTRAINT coupon_claims_settled_at_matches_status
    CHECK ((status = 'claimed' AND settled_at IS NULL)
        OR (status <> 'claimed' AND settled_at IS NOT NULL));

ALTER TABLE promo.coupon_claims ENABLE ROW LEVEL SECURITY;
ALTER TABLE promo.coupon_claims FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS coupon_claims_company_isolation ON promo.coupon_claims;
CREATE POLICY coupon_claims_company_isolation ON promo.coupon_claims
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
