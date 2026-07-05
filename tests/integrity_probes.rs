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
