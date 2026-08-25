//! Integrity probes — the domain invariants that keep promo honest under retry/concurrency.
//! Mirrors docs/business-flows/golden-cases.md.

mod common;

use backbone_promo::application::service::promo_events::LoggingSink;
use backbone_promo::application::service::promo_ports::{AccrualRequest, PriceQuery, PricingError, RedemptionRequest};
use backbone_promo::application::service::promo_write_service::PromoWriteService;
use common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

/// IP-1 — coupon redemption is bounded by `max_use` AND idempotent per source: a retry of the same
/// sale never burns a second use (council 2026-07-06 maturity fix).
#[tokio::test]
async fn ip1_coupon_redemption_bounded_and_idempotent() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let sink = LoggingSink;
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let rule_id = rule(&pool, common::RuleSpec {
        coupon_required: true,
        discount_percentage: Some(dec("50")),
        ..common::RuleSpec::for_item(company, item)
    })
    .await;

    // --- Bound: max_use=1, two DIFFERENT sales → second is rejected, counter never exceeds cap.
    let capped = coupon(&pool, company, "ONCE", rule_id, Some(1)).await;
    svc.commit_coupon_redemption(company, capped, "sales_order", Uuid::new_v4(), &sink)
        .await
        .unwrap();
    let err = svc
        .commit_coupon_redemption(company, capped, "sales_order", Uuid::new_v4(), &sink)
        .await
        .unwrap_err();
    assert!(matches!(err, PricingError::CouponExhausted));
    let used: i32 = sqlx::query_scalar("SELECT used_count FROM promo.coupon_codes WHERE id=$1")
        .bind(capped)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(used, 1);

    // --- Idempotency: max_use=2, the SAME sale committed twice consumes exactly ONE use, and the
    // retry returns the same rule (not CouponExhausted, not a second burn).
    let budgeted = coupon(&pool, company, "TWICE", rule_id, Some(2)).await;
    let sale = Uuid::new_v4();
    let r1 = svc
        .commit_coupon_redemption(company, budgeted, "sales_order", sale, &sink)
        .await
        .unwrap();
    let r2 = svc
        .commit_coupon_redemption(company, budgeted, "sales_order", sale, &sink)
        .await
        .unwrap();
    assert_eq!(r1, r2, "a retry of the same sale returns the same rule");
    let used2: i32 = sqlx::query_scalar("SELECT used_count FROM promo.coupon_codes WHERE id=$1")
        .bind(budgeted)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(used2, 1, "the same source consumes exactly one use, not two");
}

/// IP-2 — loyalty accrual is idempotent per source: replaying the paid event never double-earns.
#[tokio::test]
async fn ip2_accrual_is_idempotent() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = program(&pool, company, "0.01", "100", None).await; // 1 pt / 100 spent
    let source = Uuid::new_v4();

    let req = AccrualRequest {
        company_id: company,
        loyalty_program_id: program_id,
        customer_id: customer,
        purchase_amount: dec("250000"),
        source_type: "pos_invoice".into(),
        source_id: source,
        at: now(),
    };

    let a = svc.accrue(&req, &sink).await.unwrap();
    assert_eq!(a.points, dec("2500")); // floor(250000 * 0.01)
    assert!(!a.already);

    // Replay the exact same source → no new points.
    let b = svc.accrue(&req, &sink).await.unwrap();
    assert!(b.already);
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("2500"));
}

/// IP-3 — redemption is balance-bounded and idempotent per source.
#[tokio::test]
async fn ip3_redemption_bounded_and_idempotent() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = program(&pool, company, "0.01", "100", None).await;

    // Earn 2500 points.
    svc.accrue(
        &AccrualRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            purchase_amount: dec("250000"),
            source_type: "pos_invoice".into(),
            source_id: Uuid::new_v4(),
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    // Over-redeem → rejected, balance untouched.
    let err = svc
        .redeem(
            &RedemptionRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                points: dec("3000"),
                source_type: "redemption".into(),
                source_id: Uuid::new_v4(),
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PricingError::InsufficientPoints { .. }));
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("2500"));

    // Redeem 1000 (worth 100000 IDR) → balance 1500.
    let redemption_src = Uuid::new_v4();
    let red = RedemptionRequest {
        company_id: company,
        loyalty_program_id: program_id,
        customer_id: customer,
        points: dec("1000"),
        source_type: "redemption".into(),
        source_id: redemption_src,
        at: now(),
    };
    let r = svc.redeem(&red, &sink).await.unwrap();
    assert_eq!(r.discount_value, dec("100000.00")); // 1000 * 100
    assert!(!r.already);
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("1500"));

    // Replay same redemption source → idempotent, balance stays 1500.
    let r2 = svc.redeem(&red, &sink).await.unwrap();
    assert!(r2.already);
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("1500"));
}

/// IP-4 — resolve is side-effect-free: previewing a price with a coupon does NOT consume a use.
#[tokio::test]
async fn ip4_resolve_does_not_consume_coupon() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let rule_id = rule(&pool, common::RuleSpec {
        coupon_required: true,
        discount_percentage: Some(dec("40")),
        ..common::RuleSpec::for_item(company, item)
    })
    .await;
    let coupon_id = coupon(&pool, company, "PREVIEW", rule_id, Some(1)).await;

    let query = PriceQuery {
        company_id: company,
        list_price: dec("100000"),
        quantity: Decimal::ONE,
        item_id: item,
        item_group_id: None,
        brand_id: None,
        customer_id: None,
        customer_group_id: None,
        coupon_code: Some("PREVIEW".into()),
        tax_key: None,
        at: now(),
    };
    // Resolve several times — each returns the discount, none consumes the coupon.
    for _ in 0..3 {
        let r = svc.resolve(&query).await.unwrap();
        assert_eq!(r.unit_price, dec("60000.00"));
    }
    let used: i32 = sqlx::query_scalar("SELECT used_count FROM promo.coupon_codes WHERE id=$1")
        .bind(coupon_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(used, 0, "resolve must not consume the coupon");
}

// ---- DB-level backstops (S-7 + D5) ----------------------------------------------------------------
//
// These probe the CONSTRAINTS, not the service: even a caller that bypasses every verb cannot
// break the bounds. The service-level behaviors are IP-1..IP-4 above.

/// IP-5 — the loyalty balance conservation CONSTRAINT TRIGGER refuses any write that would push a
/// member's expiry-aware balance negative, and it agrees with the expiry-aware semantics: lapsed
/// points do not back a redemption even though the RAW sum would stay non-negative.
#[tokio::test]
async fn ip5_balance_conservation_trigger() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = program(&pool, company, "0.01", "100", None).await;

    // 100 available (no expiry) — a −200 write is refused.
    sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date)
           VALUES ($1,$2,$3,'earned',100,0,'seed',$4,$5)"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(customer)
    .bind(Uuid::new_v4())
    .bind(now())
    .execute(&pool)
    .await
    .unwrap();
    let err: Result<_, sqlx::Error> = sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date)
           VALUES ($1,$2,$3,'redeemed',-200,0,'probe',$4,$5)"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(customer)
    .bind(Uuid::new_v4())
    .bind(now())
    .execute(&pool)
    .await;
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("23514") || msg.to_lowercase().contains("loyalty"), "trigger must refuse: {msg}");

    // Expiry-awareness: a member whose ONLY points lapsed cannot spend against them, even though
    // the raw signed sum would stay ≥ 0 (100 lapsed − 50 = +50 raw, but 0 available − 50 < 0).
    let customer2 = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date, expiry_date)
           VALUES ($1,$2,$3,'earned',100,0,'probe',$4,$5, now() - interval '1 hour')"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(customer2)
    .bind(Uuid::new_v4())
    .bind(now())
    .execute(&pool)
    .await
    .unwrap();
    let err2: Result<_, sqlx::Error> = sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date)
           VALUES ($1,$2,$3,'redeemed',-50,0,'probe',$4,$5)"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(customer2)
    .bind(Uuid::new_v4())
    .bind(now())
    .execute(&pool)
    .await;
    assert!(err2.is_err(), "lapsed points must not back a redemption");
}

/// IP-6 — `used_count` can never exceed `max_use` at the TABLE level, even for a caller that
/// bypasses the guarded increment entirely.
#[tokio::test]
async fn ip6_coupon_used_count_bounded_at_table() {
    let pool = pool().await;
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let rule_id = rule(&pool, common::RuleSpec {
        discount_percentage: Some(dec("10")),
        ..common::RuleSpec::for_item(company, item)
    })
    .await;
    let coupon_id = coupon(&pool, company, "CAPX", rule_id, Some(3)).await;

    let res = sqlx::query("UPDATE promo.coupon_codes SET used_count = 4 WHERE id = $1")
        .bind(coupon_id)
        .execute(&pool)
        .await;
    assert!(res.is_err(), "used_count > max_use must be refused by the CHECK");
    let res2 = sqlx::query("UPDATE promo.coupon_codes SET used_count = 3 WHERE id = $1")
        .bind(coupon_id)
        .execute(&pool)
        .await;
    assert!(res2.is_ok(), "used_count == max_use is legal (the cap itself)");
}

/// IP-7 — inverted validity windows (`to` before `from`) are refused on every windowed table:
/// pricing rules, loyalty programs, bundles, and coupon codes.
#[tokio::test]
async fn ip7_inverted_windows_refused_everywhere() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();

    let r1 = sqlx::query(
        r#"INSERT INTO promo.pricing_rules
             (company_id, title, priority, apply_on, item_id, rate_or_discount,
              valid_from, valid_to, status)
           VALUES ($1,'w',0,'item',$2,'discount_percentage', now(), now() - interval '1 hour', 'active')"#,
    )
    .bind(company)
    .bind(item)
    .execute(&pool)
    .await;
    assert!(r1.is_err(), "pricing rule inverted window must be refused");

    let r2 = sqlx::query(
        r#"INSERT INTO promo.loyalty_programs
             (company_id, program_name, program_type, collection_factor, conversion_factor,
              from_date, to_date, status)
           VALUES ($1,'w','single_tier',0.01,100, now(), now() - interval '1 hour', 'active')"#,
    )
    .bind(company)
    .execute(&pool)
    .await;
    assert!(r2.is_err(), "loyalty program inverted window must be refused");

    let r3 = sqlx::query(
        r#"INSERT INTO promo.promo_bundles
             (company_id, title, priority, match_type, reward, valid_from, valid_to, status)
           VALUES ($1,'w',0,'all_of','discount_percentage', now(), now() - interval '1 hour', 'active')"#,
    )
    .bind(company)
    .execute(&pool)
    .await;
    assert!(r3.is_err(), "bundle inverted window must be refused");

    let rule_id = rule(&pool, common::RuleSpec {
        discount_percentage: Some(dec("10")),
        ..common::RuleSpec::for_item(company, item)
    })
    .await;
    let r4 = sqlx::query(
        r#"INSERT INTO promo.coupon_codes
             (company_id, code, pricing_rule_id, valid_from, valid_to, status)
           VALUES ($1,'WINV',$2, now(), now() - interval '1 hour', 'active')"#,
    )
    .bind(company)
    .bind(rule_id)
    .execute(&pool)
    .await;
    assert!(r4.is_err(), "coupon inverted window must be refused");
}

/// IP-8 — the per-order reversal counters are bounded by their own legs at the TABLE level: a
/// caller cannot reverse more than was granted or spent.
#[tokio::test]
async fn ip8_order_points_reversal_bounds() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let program_id = program(&pool, company, "0.1", "100", None).await;
    let svc = backbone_promo::application::service::promo_write_service::PromoWriteService::new(pool.clone());
    let customer = Uuid::new_v4();
    let order = Uuid::new_v4();
    svc.grant_order_points(
        &backbone_promo::application::service::promo_ports::OrderPointsGrantRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            grant_base_amount: dec("10000"),
            coupon_code_id: None,
            at: now(),
        },
        &LoggingSink,
    )
    .await
    .unwrap();

    let over = sqlx::query(
        "UPDATE promo.loyalty_order_points SET granted_reversed_points = granted_points + 1 WHERE order_ref_id = $1",
    )
    .bind(order)
    .execute(&pool)
    .await;
    assert!(over.is_err(), "reversing more than was granted must be refused by the CHECK");

    let spend_over = sqlx::query(
        "UPDATE promo.loyalty_order_points SET spent_reversed_points = spent_points + 1 WHERE order_ref_id = $1",
    )
    .bind(order)
    .execute(&pool)
    .await;
    assert!(spend_over.is_err(), "restoring more than was spent must be refused by the CHECK");

    // A row that records no movement at all is refused (both legs zero).
    let empty = sqlx::query(
        r#"INSERT INTO promo.loyalty_order_points
             (company_id, loyalty_program_id, customer_id, order_ref_type, order_ref_id)
           VALUES ($1,$2,$3,'probe',$4)"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(customer)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await;
    assert!(empty.is_err(), "an order row recording no movement must be refused");
}

/// IP-9 — the fresh chain installs every backstop in its final form: the window CHECKs exist and
/// are VALIDATED (not left NOT VALID), and the conservation trigger is a DEFERRABLE
/// INITIALLY IMMEDIATE constraint trigger.
#[tokio::test]
async fn ip9_backstops_installed_and_validated() {
    let pool = pool().await;

    let checks: Vec<(String, bool)> = sqlx::query_as(
        r#"SELECT conname, convalidated FROM pg_constraint
           WHERE connamespace = 'promo'::regnamespace AND contype = 'c'
             AND conname IN ('coupon_codes_used_within_max', 'coupon_codes_window_ordered',
                             'loyalty_programs_window_ordered', 'pricing_rules_window_ordered',
                             'promo_bundles_window_ordered', 'loyalty_order_points_grant_reversal_bounded',
                             'loyalty_order_points_spend_reversal_bounded',
                             'loyalty_order_points_records_a_movement')"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(checks.len(), 8, "all eight backstop CHECKs must exist, found: {checks:?}");
    for (name, validated) in &checks {
        assert!(validated, "{name} must be VALIDATED, not left NOT VALID");
    }

    let trigger: (bool, bool) = sqlx::query_as(
        r#"SELECT t.tgdeferrable, t.tginitdeferred
           FROM pg_trigger t
           WHERE t.tgname = 'loyalty_balance_conservation' AND t.tgconstraint <> 0"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(trigger.0, "conservation trigger must be DEFERRABLE");
    assert!(!trigger.1, "conservation trigger must be INITIALLY IMMEDIATE");
}
