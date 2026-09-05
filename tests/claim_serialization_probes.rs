//! Claim-serialization probes — the cart-stage promo code claim surface.
//!
//! What these prove, per the claim contract:
//!   * adjudication is SERVER-SIDE (the typed string resolves to coupon/rule ids the request
//!     never carried; normalization — trim + upper-case — happens in the verb);
//!   * every adjudication refusal is the ONE uniform `ClaimRefused` — unknown, inactive,
//!     out-of-window, exhausted-at-rest, and cart-occupied read identically on the wire, so a
//!     public claim route is not a code-enumeration oracle;
//!   * racing claims of a capped coupon's last unit SERIALIZE on the coupon row lock: N racers,
//!     exactly one wins, and the losers are refused after the standing retry-with-jitter host
//!     contract;
//!   * the cart holding the LAST unit of a capped code replays its OWN claim — the same-cart
//!     branch is decided before the headroom count, so the holder's re-typed code is an
//!     idempotent replay (`already: true`, no second row) while every fresh cart stays refused;
//!   * the claim RESERVES (headroom = used_count + active claims) but never burns — used_count
//!     advances only at commit, and the burn settles the same-ref claim atomically;
//!   * one ACTIVE claim per cart and case-folded code uniqueness are DB-backed (direct SQL
//!     violations surface as 23505, not service-side checks);
//!   * the claim rides the SAME `FOR UPDATE NOWAIT` coupon-row lock the burn takes (a held row
//!     refuses the claim as `LockBusy{CouponCode}`);
//!   * the claims table is RLS-fenced (a foreign tenant can neither read nor write claims).
//!
//! Every test seeds a fresh random company so parallel tests never collide.

mod common;

use backbone_promo::application::service::promo_ports::{
    CouponClaimStatus, LockResource, PricingError, PromoCodeClaimRequest,
};
use backbone_promo::application::service::promo_write_service::PromoWriteService;
use backbone_promo::application::service::{LoggingSink, PromoEvent, PromoEventSink};
use common::*;
use uuid::Uuid;

/// A sink that records what the verbs published.
#[derive(Default)]
struct RecSink(std::sync::Mutex<Vec<PromoEvent>>);

impl PromoEventSink for RecSink {
    fn publish(&self, event: &PromoEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

impl RecSink {
    fn count_matching(&self, f: impl Fn(&PromoEvent) -> bool) -> usize {
        self.0.lock().unwrap().iter().filter(|e| f(e)).count()
    }
}

/// A claim request for `code` on `cart`, at the test's clock.
fn claim_req(company: Uuid, cart: Uuid, code: &str) -> PromoCodeClaimRequest {
    PromoCodeClaimRequest {
        company_id: company,
        cart_ref_type: "storefront_cart".into(),
        cart_ref_id: cart,
        code: code.into(),
        at: now(),
    }
}

/// The coupon's `used_count` as stored.
async fn used_count(pool: &sqlx::PgPool, coupon_id: Uuid) -> i32 {
    sqlx::query_scalar::<_, i32>("SELECT used_count FROM promo.coupon_codes WHERE id = $1")
        .bind(coupon_id)
        .fetch_one(pool)
        .await
        .expect("read used_count")
}

/// The cart's claim rows as stored (status, in claim order).
async fn claim_rows(
    pool: &sqlx::PgPool,
    company: Uuid,
    cart: Uuid,
) -> Vec<(Uuid, String)> {
    let rows = sqlx::query(
        r#"SELECT id, status::text FROM promo.coupon_claims
           WHERE company_id = $1 AND cart_ref_type = 'storefront_cart' AND cart_ref_id = $2
           ORDER BY claimed_at"#,
    )
    .bind(company)
    .bind(cart)
    .fetch_all(pool)
    .await
    .expect("read claim rows");
    rows.iter().map(|r| (r.get("id"), r.get("status"))).collect()
}

use sqlx::Row;

/// A seeded coupon-claim fixture: a pct rule + an active coupon `CODE` with the given cap.
async fn fixture(pool: &sqlx::PgPool, company: Uuid, max_use: Option<i32>) -> Uuid {
    let item = Uuid::new_v4();
    let rule_id = pct_rule(pool, company, item, 0, "10").await;
    coupon(pool, company, "CLAIMABLE", rule_id, max_use).await
}

// ---- adjudication + durability -------------------------------------------------------------------

/// The claim resolves the typed string SERVER-SIDE: a lower-case, padded input mints a claim on
/// the upper-cased stored code, the outcome carries only server-derived ids, the row lands
/// `claimed`, and exactly one PromoCodeClaimed event publishes.
#[tokio::test]
async fn claim_adjudicates_server_side_and_persists() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let coupon_id = fixture(&pool, company, Some(5)).await;
    let cart = Uuid::new_v4();
    let sink = RecSink::default();

    let out = svc
        .claim_promo_code(&claim_req(company, cart, "  claimable  "), &sink)
        .await
        .expect("claim");

    // Server-derived facts only: the stored (canonical) code, the coupon/rule the request never
    // named, and the minted claim id.
    assert_eq!(out.code, "CLAIMABLE");
    assert_eq!(out.coupon_id, coupon_id);
    assert!(!out.already);
    assert!(!out.claim_id.is_nil());

    // Durable + inspectable: one row in `claimed`, readable back through the verb.
    let rows = svc.claims_for_cart(company, "storefront_cart", cart).await.expect("history");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].claim_id, out.claim_id);
    assert_eq!(rows[0].status, CouponClaimStatus::Claimed);
    assert_eq!(rows[0].code, "CLAIMABLE");
    assert_eq!(rows[0].settled_at, None);
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::PromoCodeClaimed(_))), 1);

    // The claim RESERVES, never burns.
    assert_eq!(used_count(&pool, coupon_id).await, 0);
}

/// Re-claiming the code the cart already holds is an idempotent replay: the SAME claim id, no
/// second row, no second event, no second unit of headroom.
#[tokio::test]
async fn claim_is_idempotent_per_cart_and_code() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    fixture(&pool, company, Some(2)).await;
    let cart = Uuid::new_v4();
    let sink = RecSink::default();

    let first = svc.claim_promo_code(&claim_req(company, cart, "CLAIMABLE"), &sink).await.unwrap();
    // The same code typed differently is still the same claim.
    let second = svc.claim_promo_code(&claim_req(company, cart, "claimable"), &sink).await.unwrap();
    assert_eq!(second.claim_id, first.claim_id);
    assert!(second.already);
    assert_eq!(claim_rows(&pool, company, cart).await.len(), 1);
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::PromoCodeClaimed(_))), 1);
}

/// A cart holding the LAST unit of a capped code replays ITS OWN claim: the same-cart branch
/// is decided before the headroom count, so the holder's re-typed code is the idempotent
/// replay (`already: true`, same claim id, no second row, `used_count` untouched) — not a
/// refusal by its own reservation. A fresh cart is still refused: the cap is genuinely
/// consumed by the holder's claim alone.
#[tokio::test]
async fn capped_last_unit_holder_replays_its_own_claim() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let coupon_id = fixture(&pool, company, Some(1)).await;
    let holder = Uuid::new_v4();
    let sink = RecSink::default();

    let first = svc
        .claim_promo_code(&claim_req(company, holder, "CLAIMABLE"), &sink)
        .await
        .expect("the holder wins the single unit");
    assert!(!first.already);

    // The holder re-types its own code: a replay, never a headroom contender — this cart's
    // own reservation is the unit that fills the cap.
    let replay = svc
        .claim_promo_code(&claim_req(company, holder, "claimable"), &sink)
        .await
        .expect("the capped holder replays its own claim");
    assert!(replay.already);
    assert_eq!(replay.claim_id, first.claim_id);
    assert_eq!(replay.code, "CLAIMABLE");

    // No second row, no second event, and the burn counter untouched.
    let rows = claim_rows(&pool, company, holder).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, first.claim_id);
    assert_eq!(rows[0].1, "claimed");
    assert_eq!(used_count(&pool, coupon_id).await, 0);
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::PromoCodeClaimed(_))), 1);

    // The cap stays shut for everyone else: a fresh cart is uniformly refused.
    let other = svc
        .claim_promo_code(&claim_req(company, Uuid::new_v4(), "CLAIMABLE"), &LoggingSink)
        .await
        .unwrap_err();
    assert!(matches!(other, PricingError::ClaimRefused));
}

// ---- the uniform refusal (no code-enumeration oracle) ---------------------------------------------

/// Every adjudication refusal is the ONE variant with the ONE display string: unknown code,
/// inactive coupon, out-of-window coupon, exhausted-at-rest coupon, and a cart already holding
/// a different code are indistinguishable on the wire.
#[tokio::test]
async fn claim_refusals_are_uniform() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let rule_id = pct_rule(&pool, company, item, 0, "10").await;

    // Unknown code.
    let unknown = svc
        .claim_promo_code(&claim_req(company, Uuid::new_v4(), "NOSUCHCODE"), &LoggingSink)
        .await
        .unwrap_err();

    // Inactive coupon.
    let inactive_id = coupon(&pool, company, "SLEEPY", rule_id, None).await;
    sqlx::query("UPDATE promo.coupon_codes SET status = 'inactive' WHERE id = $1")
        .bind(inactive_id)
        .execute(&pool)
        .await
        .unwrap();
    let inactive = svc
        .claim_promo_code(&claim_req(company, Uuid::new_v4(), "SLEEPY"), &LoggingSink)
        .await
        .unwrap_err();

    // Out-of-window coupon (window already closed).
    let expired_id = coupon(&pool, company, "PASTDUE", rule_id, None).await;
    sqlx::query("UPDATE promo.coupon_codes SET valid_upto = $1 WHERE id = $2")
        .bind(now() - chrono::Duration::hours(1))
        .bind(expired_id)
        .execute(&pool)
        .await
        .unwrap();
    let expired = svc
        .claim_promo_code(&claim_req(company, Uuid::new_v4(), "PASTDUE"), &LoggingSink)
        .await
        .unwrap_err();

    // Exhausted at rest: max_use fully burned already.
    let burned_id = coupon(&pool, company, "ALLGONE", rule_id, Some(1)).await;
    sqlx::query("UPDATE promo.coupon_codes SET used_count = 1 WHERE id = $1")
        .bind(burned_id)
        .execute(&pool)
        .await
        .unwrap();
    let exhausted = svc
        .claim_promo_code(&claim_req(company, Uuid::new_v4(), "ALLGONE"), &LoggingSink)
        .await
        .unwrap_err();

    // A cart already holding a different code: both codes are valid and unexhausted — the
    // occupied slot alone must produce the refusal.
    coupon(&pool, company, "FIRSTPICK", rule_id, None).await;
    coupon(&pool, company, "SECONDPICK", rule_id, None).await;
    let occupied_cart = Uuid::new_v4();
    svc.claim_promo_code(&claim_req(company, occupied_cart, "FIRSTPICK"), &LoggingSink)
        .await
        .unwrap();
    let occupied = svc
        .claim_promo_code(&claim_req(company, occupied_cart, "SECONDPICK"), &LoggingSink)
        .await
        .unwrap_err();

    for err in [unknown, inactive, expired, exhausted, occupied] {
        assert!(matches!(err, PricingError::ClaimRefused), "expected ClaimRefused, got {err:?}");
        assert_eq!(err.to_string(), "claim refused");
    }
}

// ---- serialization: N racers, exactly one wins ----------------------------------------------------

/// Eight distinct carts race the LAST unit of a max_use=1 coupon. Playing the standing host
/// contract (a LockBusy is retried once-ish with jitter until terminal), EXACTLY ONE claim
/// wins: one `claimed` row, seven uniform refusals, and `used_count` untouched — the claim
/// reserves, it never burns.
#[tokio::test]
async fn concurrent_claims_of_last_coupon_exactly_one_wins() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let coupon_id = fixture(&pool, company, Some(1)).await;

    let svc = std::sync::Arc::new(PromoWriteService::new(pool.clone()));
    // The sink is not Sync, so the claim futures are !Send — run the storm on a LocalSet (the
    // tasks still interleave at every await point, so the pool-level lock contention is real).
    let set = tokio::task::LocalSet::new();
    let mut handles = Vec::new();
    for racer in 0..8u64 {
        let svc = svc.clone();
        handles.push(set.spawn_local(async move {
            let cart = Uuid::new_v4();
            // Host contract: retry a LockBusy (fresh transaction each attempt) with growing
            // backoff until the answer is terminal — a refusal after contention is still a
            // refusal, which is exactly what the probe measures.
            for attempt in 1..=10u64 {
                match svc
                    .claim_promo_code(&claim_req(company, cart, "CLAIMABLE"), &LoggingSink)
                    .await
                {
                    Ok(out) => return (racer, Ok(out)),
                    Err(PricingError::LockBusy { .. }) => {
                        tokio::time::sleep(std::time::Duration::from_millis(15 * attempt)).await;
                        continue;
                    }
                    Err(e) => return (racer, Err(e)),
                }
            }
            unreachable!("staggered retries always produce a terminal answer")
        }));
    }
    set.await;

    let mut winners = 0usize;
    let mut refused = 0usize;
    for h in handles {
        let (_racer, result) = h.await.unwrap();
        match result {
            Ok(out) => {
                winners += 1;
                assert!(!out.already);
                assert_eq!(out.code, "CLAIMABLE");
            }
            Err(PricingError::ClaimRefused) => refused += 1,
            Err(e) => panic!("unexpected error in the storm: {e}"),
        }
    }
    assert_eq!(winners, 1, "exactly one racer must win the last unit");
    assert_eq!(refused, 7, "every other racer must be uniformly refused");

    // One claimed row; the burn counter untouched.
    let claimed = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM promo.coupon_claims
           WHERE company_id = $1 AND coupon_id = $2 AND status = 'claimed'"#,
    )
    .bind(company)
    .bind(coupon_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(claimed, 1);
    assert_eq!(used_count(&pool, coupon_id).await, 0);
}

/// A held coupon row (a plain FOR UPDATE in a foreign transaction) refuses the claim as
/// `LockBusy{CouponCode}` — proof the claim rides the SAME row lock the burn takes, and takes
/// it FIRST, before any write.
#[tokio::test]
async fn nowait_held_coupon_row_refuses_claim() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let coupon_id = fixture(&pool, company, Some(5)).await;

    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM promo.coupon_codes WHERE id = $1 FOR UPDATE")
        .bind(coupon_id)
        .execute(&mut *holder)
        .await
        .unwrap();

    let err = svc
        .claim_promo_code(&claim_req(company, Uuid::new_v4(), "CLAIMABLE"), &LoggingSink)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        PricingError::LockBusy { resource: LockResource::CouponCode }
    ));
    holder.rollback().await.unwrap();
}

// ---- headroom, release, settle -------------------------------------------------------------------

/// Headroom counts CLAIMS: on a max_use=2 coupon the third cart is refused while two claims
/// hold; releasing one frees the unit and the slot immediately (the explicit, now-shaped
/// answer to the abandoned-code lockout — no delayed sweep).
#[tokio::test]
async fn release_frees_headroom_and_the_cart_slot() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    fixture(&pool, company, Some(2)).await;
    let cart1 = Uuid::new_v4();
    let cart2 = Uuid::new_v4();
    let cart3 = Uuid::new_v4();

    svc.claim_promo_code(&claim_req(company, cart1, "CLAIMABLE"), &LoggingSink).await.unwrap();
    svc.claim_promo_code(&claim_req(company, cart2, "CLAIMABLE"), &LoggingSink).await.unwrap();
    let refused = svc
        .claim_promo_code(&claim_req(company, cart3, "CLAIMABLE"), &LoggingSink)
        .await
        .unwrap_err();
    assert!(matches!(refused, PricingError::ClaimRefused));

    // Release cart1: idempotent, frees the unit, publishes one event.
    let sink = RecSink::default();
    let released = svc
        .release_promo_claim(company, "storefront_cart", cart1, now(), &sink)
        .await
        .unwrap();
    assert!(released.is_some());
    let again = svc
        .release_promo_claim(company, "storefront_cart", cart1, now(), &sink)
        .await
        .unwrap();
    assert_eq!(again, None);
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::PromoCodeClaimReleased(_))), 1);

    // The freed unit is cart3's now.
    svc.claim_promo_code(&claim_req(company, cart3, "CLAIMABLE"), &LoggingSink).await.unwrap();

    // And the freed SLOT lets cart1 claim a different code outright.
    let item = Uuid::new_v4();
    let rule_id = pct_rule(&pool, company, item, 0, "10").await;
    coupon(&pool, company, "OTHERCODE", rule_id, None).await;
    svc.claim_promo_code(&claim_req(company, cart1, "OTHERCODE"), &LoggingSink).await.unwrap();

    let rows1 = claim_rows(&pool, company, cart1).await;
    assert_eq!(rows1.len(), 2, "released history stays inspectable");
    assert!(rows1.iter().any(|(_, s)| s == "released"));
    assert!(rows1.iter().any(|(_, s)| s == "claimed"));
}

/// The burn settles the same-ref claim atomically: `commit_coupon_redemption` under the SAME
/// document ref flips the claim to `redeemed` (stamped), advances `used_count`, and a replayed
/// burn neither double-burns nor re-settles.
#[tokio::test]
async fn burn_settles_the_same_ref_claim() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let coupon_id = fixture(&pool, company, Some(3)).await;
    let cart = Uuid::new_v4();

    let out = svc.claim_promo_code(&claim_req(company, cart, "CLAIMABLE"), &LoggingSink).await.unwrap();

    let rule = svc
        .commit_coupon_redemption(company, coupon_id, "storefront_cart", cart, &LoggingSink)
        .await
        .expect("burn");
    assert_eq!(rule, out.pricing_rule_id);
    assert_eq!(used_count(&pool, coupon_id).await, 1);

    let rows = svc.claims_for_cart(company, "storefront_cart", cart).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, CouponClaimStatus::Redeemed);
    assert!(rows[0].settled_at.is_some());

    // Replayed burn: idempotent (no second burn), the settled claim stays settled.
    svc.commit_coupon_redemption(company, coupon_id, "storefront_cart", cart, &LoggingSink)
        .await
        .expect("replay");
    assert_eq!(used_count(&pool, coupon_id).await, 1);
    let rows = claim_rows(&pool, company, cart).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "redeemed");
}

/// The plain selling path is unchanged: a burn with no prior claim commits and touches zero
/// claim rows (claiming is an additive storefront-stage surface, not a gate on the burn).
#[tokio::test]
async fn burn_without_claim_is_unchanged() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let coupon_id = fixture(&pool, company, Some(3)).await;
    let order = Uuid::new_v4();

    let rule = svc
        .commit_coupon_redemption(company, coupon_id, "sales_order", order, &LoggingSink)
        .await
        .expect("burn");
    assert!(!rule.is_nil());
    assert_eq!(used_count(&pool, coupon_id).await, 1);

    let claims = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM promo.coupon_claims WHERE company_id = $1",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(claims, 0);
}

// ---- DB-backed uniqueness (no uniqueness-by-convention) -------------------------------------------

/// Case-folded code uniqueness is enforced by the DATABASE: within one company a differently-
/// cased twin of an existing code is a 23505 at insert — no check-then-act search can race it.
#[tokio::test]
async fn case_folded_code_uniqueness_is_db_backed() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let rule_id = pct_rule(&pool, company, item, 0, "10").await;

    // A mixed-case stored code (bypassing the upper-casing helpers, as any raw writer could).
    sqlx::query(
        r#"INSERT INTO promo.coupon_codes (company_id, code, pricing_rule_id, valid_from, status)
           VALUES ($1, 'MiXeD10', $2, $3, 'active')"#,
    )
    .bind(company)
    .bind(rule_id)
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await
    .unwrap();

    // The case-folded twin is refused by the index, not by a service-side search.
    let twin = sqlx::query(
        r#"INSERT INTO promo.coupon_codes (company_id, code, pricing_rule_id, valid_from, status)
           VALUES ($1, 'mixed10', $2, $3, 'active')"#,
    )
    .bind(company)
    .bind(rule_id)
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        twin.as_database_error().and_then(|d| d.code()).map(|c| c.to_string()),
        Some("23505".to_string())
    );

    // A different company's same code is fine — uniqueness is per company.
    let company2 = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO promo.coupon_codes (company_id, code, pricing_rule_id, valid_from, status)
           VALUES ($1, 'MIXED10', $2, $3, 'active')"#,
    )
    .bind(company2)
    .bind(rule_id)
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await
    .expect("cross-company code is not a conflict");
}

/// One ACTIVE claim per cart is a partial unique index: a direct second `claimed` row for the
/// same cart is a 23505 — the bound is the database, not a service check a racer could slip past.
#[tokio::test]
async fn one_active_claim_per_cart_is_db_backed() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let rule_id = pct_rule(&pool, company, item, 0, "10").await;
    let coupon_id = coupon(&pool, company, "ONEPERTROLLEY", rule_id, None).await;
    let cart = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO promo.coupon_claims
               (company_id, cart_ref_type, cart_ref_id, coupon_id, code, pricing_rule_id, claimed_at)
           VALUES ($1, 'storefront_cart', $2, $3, 'ONEPERTROLLEY', $4, $5)"#,
    )
    .bind(company)
    .bind(cart)
    .bind(coupon_id)
    .bind(rule_id)
    .bind(now())
    .execute(&pool)
    .await
    .unwrap();

    let dup = sqlx::query(
        r#"INSERT INTO promo.coupon_claims
               (company_id, cart_ref_type, cart_ref_id, coupon_id, code, pricing_rule_id, claimed_at)
           VALUES ($1, 'storefront_cart', $2, $3, 'ONEPERTROLLEY', $4, $5)"#,
    )
    .bind(company)
    .bind(cart)
    .bind(coupon_id)
    .bind(rule_id)
    .bind(now())
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        dup.as_database_error().and_then(|d| d.code()).map(|c| c.to_string()),
        Some("23505".to_string())
    );
}

// ---- the claims table's RLS fence ------------------------------------------------------------------

const PROBE_ROLE: &str = "promo_claim_fence_probe";
const PROBE_PASSWORD: &str = "probe";

/// The claims table under a NON-BYPASSRLS session: a foreign tenant neither reads the victim's
/// claims nor can write a claim into the victim's company (WITH CHECK). Same pattern as the
/// loyalty fence suite; its own probe role with grants on exactly the two tables the claim
/// path touches.
#[tokio::test]
async fn claims_table_is_company_fenced() {
    let admin = pool().await;
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_promo".into());

    sqlx::query("SELECT pg_advisory_lock(hashtext('promo_claim_fence_probe'))")
        .execute(&admin)
        .await
        .unwrap();
    let _ = sqlx::query(&format!(
        "CREATE ROLE {PROBE_ROLE} LOGIN PASSWORD '{PROBE_PASSWORD}' \
           NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE"
    ))
    .execute(&admin)
    .await;
    let db = url
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://")
        .split_once('/')
        .and_then(|(_, path)| path.split('?').next())
        .unwrap_or("backbone_promo");
    for grant in [
        format!(r#"GRANT CONNECT ON DATABASE "{db}" TO {PROBE_ROLE}"#),
        format!("GRANT USAGE ON SCHEMA promo TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE promo.coupon_claims TO {PROBE_ROLE}"),
    ] {
        sqlx::query(&grant).execute(&admin).await.unwrap();
    }
    sqlx::query("SELECT pg_advisory_unlock(hashtext('promo_claim_fence_probe'))")
        .execute(&admin)
        .await
        .unwrap();

    let authority = url
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://");
    let (auth, _) = authority.split_once('/').unwrap();
    let hostport = auth.rsplit_once('@').map(|(_, h)| h).unwrap_or(auth);
    let restricted =
        PgPool::connect(&format!("postgresql://{PROBE_ROLE}:{PROBE_PASSWORD}@{hostport}/{db}"))
            .await
            .expect("connect as restricted probe");

    let victim = Uuid::new_v4();
    let attacker = Uuid::new_v4();
    let item = Uuid::new_v4();
    let rule_id = pct_rule(&admin, victim, item, 0, "10").await;
    let coupon_id = coupon(&admin, victim, "FENCED1", rule_id, None).await;
    let cart = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO promo.coupon_claims
               (company_id, cart_ref_type, cart_ref_id, coupon_id, code, pricing_rule_id, claimed_at)
           VALUES ($1, 'storefront_cart', $2, $3, 'FENCED1', $4, $5)"#,
    )
    .bind(victim)
    .bind(cart)
    .bind(coupon_id)
    .bind(rule_id)
    .bind(now())
    .execute(&admin)
    .await
    .unwrap();

    // Read-fenced: the victim's claim is invisible even when the query names the victim.
    let mut scoped = restricted.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(attacker.to_string())
        .execute(&mut *scoped)
        .await
        .unwrap();
    let seen = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM promo.coupon_claims WHERE company_id = $1",
    )
    .bind(victim)
    .fetch_one(&mut *scoped)
    .await
    .unwrap();
    assert_eq!(seen, 0, "a foreign tenant must not see the victim's claims");

    // Write-fenced: inserting INTO the victim's company fails the WITH CHECK clause.
    let write = sqlx::query(
        r#"INSERT INTO promo.coupon_claims
               (company_id, cart_ref_type, cart_ref_id, coupon_id, code, pricing_rule_id, claimed_at)
           VALUES ($1, 'storefront_cart', $2, $3, 'FENCED1', $4, $5)"#,
    )
    .bind(victim)
    .bind(Uuid::new_v4())
    .bind(coupon_id)
    .bind(rule_id)
    .bind(now())
    .execute(&mut *scoped)
    .await
    .unwrap_err();
    assert_eq!(
        write.as_database_error().and_then(|d| d.code()).map(|c| c.to_string()),
        Some("42501".to_string()),
        "a foreign tenant's claim insert must fail the RLS WITH CHECK"
    );
    scoped.rollback().await.unwrap();
}

use sqlx::PgPool;

/// Caller-contract shape checks: a malformed document ref is a loud Invalid (a programmer
/// error, not a shopper refusal); an empty code is the uniform ClaimRefused.
#[tokio::test]
async fn malformed_refs_and_empty_code_are_refused_loudly_or_uniformly() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    fixture(&pool, company, None).await;

    let mut req = claim_req(company, Uuid::new_v4(), "CLAIMABLE");
    req.cart_ref_type = String::new();
    let err = svc.claim_promo_code(&req, &LoggingSink).await.unwrap_err();
    assert!(matches!(err, PricingError::Invalid(_)));

    let mut req = claim_req(company, Uuid::new_v4(), "CLAIMABLE");
    req.cart_ref_type = "x".repeat(41);
    let err = svc.claim_promo_code(&req, &LoggingSink).await.unwrap_err();
    assert!(matches!(err, PricingError::Invalid(_)));

    let req = claim_req(company, Uuid::new_v4(), "   ");
    let err = svc.claim_promo_code(&req, &LoggingSink).await.unwrap_err();
    assert!(matches!(err, PricingError::ClaimRefused));
}
