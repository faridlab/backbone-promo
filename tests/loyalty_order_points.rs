//! Per-order loyalty accounting tests — the order-flow verbs (grant / spend / reverse) and the
//! NOWAIT serialization contract.
//!
//! Every test uses a fresh random company (and member) so parallel tests never collide. Program
//! factors are fixed for readability: collection 0.1 pts per currency unit, conversion 100
//! currency per point — a 10,000 base grants 1,000 pts worth 100,000 on burn.

mod common;

use backbone_promo::application::service::promo_ports::{
    LockResource, OrderPointsGrantRequest, OrderPointsReversalRequest, OrderPointsSpendRequest,
    PricingError,
};
use backbone_promo::application::service::promo_write_service::PromoWriteService;
use backbone_promo::application::service::{
    AccrualRequest, LoggingSink, PromoEvent, PromoEventSink,
};
use common::*;
use rust_decimal::Decimal;
use sqlx::Row;
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

/// Seed a program with the standard test factors (0.1 pts/unit, 100/pt).
async fn std_program(pool: &sqlx::PgPool, company: Uuid, expiry_days: Option<i32>) -> Uuid {
    program(pool, company, "0.1", "100", expiry_days).await
}

/// Seed the member's anchor row WITHOUT going through a verb (concurrency tests lock it directly).
async fn seed_anchor(pool: &sqlx::PgPool, company: Uuid, customer: Uuid, program_id: Uuid) {
    sqlx::query(
        "INSERT INTO promo.loyalty_member_anchors (company_id, customer_id, loyalty_program_id)
         VALUES ($1,$2,$3) ON CONFLICT DO NOTHING",
    )
    .bind(company)
    .bind(customer)
    .bind(program_id)
    .execute(pool)
    .await
    .expect("seed anchor");
}

/// The member's expiry-aware position at `at`: (available, lapsed).
async fn balances_at(
    pool: &sqlx::PgPool,
    company: Uuid,
    customer: Uuid,
    program_id: Uuid,
    at: chrono::DateTime<chrono::Utc>,
) -> (Decimal, Decimal) {
    let row = sqlx::query(
        r#"SELECT
               COALESCE(SUM(points) FILTER (WHERE expiry_date IS NULL OR expiry_date > $4), 0) AS available,
               COALESCE(SUM(points) FILTER (WHERE expiry_date IS NOT NULL AND expiry_date <= $4), 0) AS lapsed
           FROM promo.loyalty_point_entries
           WHERE company_id=$1 AND customer_id=$2 AND loyalty_program_id=$3
             AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company)
    .bind(customer)
    .bind(program_id)
    .bind(at)
    .fetch_one(pool)
    .await
    .expect("balances");
    let available: Decimal = row.get("available");
    let lapsed: Decimal = row.get("lapsed");
    (available, lapsed)
}

/// Grant: the server derives floor(base · collection_factor), writes the order row born with its
/// grant leg, claims the ledger entry, and publishes exactly one event.
#[tokio::test]
async fn grant_derives_points_and_writes_row() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    let out = svc
        .grant_order_points(
            &OrderPointsGrantRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                grant_base_amount: dec("10000"),
                coupon_code_id: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();

    assert_eq!(out.points, dec("1000")); // floor(10,000 × 0.1)
    assert!(!out.already);
    assert!(out.order_points_id.is_some());
    assert!(out.entry_id.is_some());
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("1000"));

    // The order row carries the grant leg.
    let row = sqlx::query_as::<_, (Decimal, Decimal, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"SELECT granted_points, grant_base_amount, granted_at FROM promo.loyalty_order_points
           WHERE company_id=$1 AND loyalty_program_id=$2 AND order_ref_type='pos_order' AND order_ref_id=$3"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(order)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, dec("1000"));
    assert_eq!(row.1, dec("10000.00"));
    assert!(row.2.is_some());

    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::LoyaltyOrderPointsGranted(_))), 1);
}

/// A replayed confirm is a no-op: same outcome values, `already`, no second row or entry.
#[tokio::test]
async fn grant_replay_is_idempotent() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();
    let req = OrderPointsGrantRequest {
        company_id: company,
        loyalty_program_id: program_id,
        customer_id: customer,
        order_ref_type: "pos_order".into(),
        order_ref_id: order,
        grant_base_amount: dec("10000"),
        coupon_code_id: None,
        at: now(),
    };

    let first = svc.grant_order_points(&req, &sink).await.unwrap();
    let second = svc.grant_order_points(&req, &sink).await.unwrap();

    assert!(!first.already);
    assert!(second.already);
    assert_eq!(second.points, first.points);
    assert_eq!(second.order_points_id, first.order_points_id);
    assert_eq!(second.entry_id, None); // nothing new claimed
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("1000"));
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM promo.loyalty_order_points WHERE company_id=$1 AND order_ref_id=$2",
    )
    .bind(company)
    .bind(order)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::LoyaltyOrderPointsGranted(_))), 1);
}

/// A base whose derived points floor to zero writes NOTHING (mirrors `accrue`'s zero behavior), and
/// a negative base is refused before any lock is taken.
#[tokio::test]
async fn grant_below_factor_floor_writes_nothing() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    let out = svc
        .grant_order_points(
            &OrderPointsGrantRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                grant_base_amount: dec("5"), // 5 × 0.1 = 0.5 → floor 0
                coupon_code_id: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();
    assert_eq!(out.points, Decimal::ZERO);
    assert!(out.order_points_id.is_none());
    assert!(out.entry_id.is_none());
    assert_eq!(balance(&pool, company, customer, program_id).await, Decimal::ZERO);

    let err = svc
        .grant_order_points(
            &OrderPointsGrantRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                grant_base_amount: dec("-1"),
                coupon_code_id: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PricingError::Invalid(_)));
}

/// Spend: the member's balance comes from an earn elsewhere; the order spends against it and the
/// money value is DERIVED server-side (points × conversion_factor).
#[tokio::test]
async fn spend_happy_path_derives_discount() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let settle = Uuid::new_v4();
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    // Earn 1000 pts on an unrelated settlement document.
    svc.accrue(
        &AccrualRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            purchase_amount: dec("10000"),
            source_type: "settlement".into(),
            source_id: settle,
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    let out = svc
        .spend_order_points(
            &OrderPointsSpendRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                points: dec("400"),
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();

    assert!(!out.already);
    assert_eq!(out.points, dec("400"));
    assert_eq!(out.discount_value, dec("40000")); // 400 × 100, derived
    assert_eq!(out.available_after, dec("600"));
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("600"));

    // The order row exists with only its spend leg.
    let row = sqlx::query_as::<_, (Decimal, Decimal)>(
        r#"SELECT spent_points, granted_points FROM promo.loyalty_order_points
           WHERE company_id=$1 AND loyalty_program_id=$2 AND order_ref_type='pos_order' AND order_ref_id=$3"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(order)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, dec("400"));
    assert_eq!(row.1, Decimal::ZERO);
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::LoyaltyOrderPointsSpent(_))), 1);
}

/// A replayed payment is a no-op: the stored spend is returned, nothing burns twice.
#[tokio::test]
async fn spend_replay_returns_stored() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    svc.accrue(
        &AccrualRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            purchase_amount: dec("10000"),
            source_type: "settlement".into(),
            source_id: Uuid::new_v4(),
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    let req = OrderPointsSpendRequest {
        company_id: company,
        loyalty_program_id: program_id,
        customer_id: customer,
        order_ref_type: "pos_order".into(),
        order_ref_id: order,
        points: dec("400"),
        at: now(),
    };
    let first = svc.spend_order_points(&req, &sink).await.unwrap();
    let second = svc.spend_order_points(&req, &sink).await.unwrap();

    assert!(second.already);
    assert_eq!(second.entry_id, first.entry_id);
    assert_eq!(second.points, first.points);
    assert_eq!(second.discount_value, first.discount_value);
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("600"));
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::LoyaltyOrderPointsSpent(_))), 1);
}

/// A spend the member's LAPSED points would have covered refuses as PointsExpired, not
/// InsufficientPoints — the customer had them; they expired.
#[tokio::test]
async fn spend_refuses_with_points_expired() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();

    // 500 pts earned with an expiry that has already passed.
    sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date, expiry_date)
           VALUES ($1,$2,$3,'earned',500,0,'seed',$4,$5,$6)"#,
    )
    .bind(company)
    .bind(program_id)
    .bind(customer)
    .bind(Uuid::new_v4())
    .bind(now())
    .bind(now() - chrono::Duration::days(1))
    .execute(&pool)
    .await
    .unwrap();

    let err = svc
        .spend_order_points(
            &OrderPointsSpendRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                points: dec("300"),
                at: now(),
            },
            &LoggingSink,
        )
        .await
        .unwrap_err();
    match err {
        PricingError::PointsExpired { lapsed, available } => {
            assert_eq!(lapsed, dec("500"));
            assert_eq!(available, Decimal::ZERO);
        }
        other => panic!("expected PointsExpired, got {other:?}"),
    }
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("500")); // untouched
}

/// A spend neither available nor lapsed points cover refuses as InsufficientPoints.
#[tokio::test]
async fn spend_refuses_insufficient() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;

    let err = svc
        .spend_order_points(
            &OrderPointsSpendRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: Uuid::new_v4(),
                points: dec("100"),
                at: now(),
            },
            &LoggingSink,
        )
        .await
        .unwrap_err();
    match err {
        PricingError::InsufficientPoints { available, requested } => {
            assert_eq!(available, Decimal::ZERO);
            assert_eq!(requested, dec("100"));
        }
        other => panic!("expected InsufficientPoints, got {other:?}"),
    }
}

/// Full reversal of a grant-only order claws the whole grant back; a SECOND return document on the
/// same order reverses nothing (the counters are at their bounds), and the replay of the first
/// return returns its stored legs.
#[tokio::test]
async fn full_reversal_grant_only_then_nothing_left() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let ret1 = Uuid::new_v4();
    let ret2 = Uuid::new_v4();
    let sink = RecSink::default();

    svc.grant_order_points(
        &OrderPointsGrantRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            grant_base_amount: dec("10000"),
            coupon_code_id: None,
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    let mk = |ret: Uuid| OrderPointsReversalRequest {
        company_id: company,
        loyalty_program_id: program_id,
        customer_id: customer,
        order_ref_type: "pos_order".into(),
        order_ref_id: order,
        reversal_ref_type: "pos_return".into(),
        reversal_ref_id: ret,
        return_amount: None,
        at: now(),
    };

    let first = svc.reverse_order_points(&mk(ret1), &sink).await.unwrap();
    assert!(!first.already);
    assert_eq!(first.grant_reversed, dec("1000"));
    assert_eq!(first.spend_restored, Decimal::ZERO);
    assert_eq!(balance(&pool, company, customer, program_id).await, Decimal::ZERO);

    // Replaying the SAME return returns the stored legs.
    let replay = svc.reverse_order_points(&mk(ret1), &sink).await.unwrap();
    assert!(replay.already);
    assert_eq!(replay.grant_reversed, dec("1000"));

    // A DIFFERENT return on the fully-reversed order writes nothing.
    let second = svc.reverse_order_points(&mk(ret2), &sink).await.unwrap();
    assert!(!second.already);
    assert_eq!(second.grant_reversed, Decimal::ZERO);
    assert_eq!(second.spend_restored, Decimal::ZERO);
    assert_eq!(balance(&pool, company, customer, program_id).await, Decimal::ZERO);
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::LoyaltyOrderPointsReversed(_))), 1);
}

/// A partial return claws back proportionally: floor(return_amount · collection_factor), bounded
/// by what remains un-reversed.
#[tokio::test]
async fn partial_reversal_proportional() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    svc.grant_order_points(
        &OrderPointsGrantRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            grant_base_amount: dec("10000"),
            coupon_code_id: None,
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    let out = svc
        .reverse_order_points(
            &OrderPointsReversalRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                reversal_ref_type: "pos_return".into(),
                reversal_ref_id: Uuid::new_v4(),
                return_amount: Some(dec("2500")), // floor(2,500 × 0.1) = 250
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();
    assert_eq!(out.grant_reversed, dec("250"));
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("750"));

    let counters = sqlx::query_as::<_, (Decimal, Decimal)>(
        "SELECT granted_reversed_points, spent_reversed_points FROM promo.loyalty_order_points WHERE order_ref_id=$1",
    )
    .bind(order)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counters.0, dec("250"));
    assert_eq!(counters.1, Decimal::ZERO);
}

/// Full reversal after the member SPENT some of the grant: the clawback is bounded by what the
/// member still holds (never negative), and the spend is restored fully.
#[tokio::test]
async fn full_reversal_after_spend_bounded_by_available() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    // Grant 1000 and spend 400 on the SAME order.
    svc.grant_order_points(
        &OrderPointsGrantRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            grant_base_amount: dec("10000"),
            coupon_code_id: None,
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();
    svc.spend_order_points(
        &OrderPointsSpendRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            points: dec("400"),
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("600"));

    let out = svc
        .reverse_order_points(
            &OrderPointsReversalRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                reversal_ref_type: "pos_return".into(),
                reversal_ref_id: Uuid::new_v4(),
                return_amount: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();
    // Clawback bounded by the 600 still held; the 400 spent is restored.
    assert_eq!(out.grant_reversed, dec("600"));
    assert_eq!(out.spend_restored, dec("400"));
    // 1000 − 400 − 600 + 400 = 400 stays with the member.
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("400"));
}

/// An order that SPENT before it granted (points earned elsewhere, confirm arriving late): the
/// late grant claims its ledger entry and sets the grant leg on the existing row.
#[tokio::test]
async fn spend_then_late_grant_sets_grant_leg() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    // Earn 2000 elsewhere, spend 400 on this order first.
    svc.accrue(
        &AccrualRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            purchase_amount: dec("20000"),
            source_type: "settlement".into(),
            source_id: Uuid::new_v4(),
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();
    svc.spend_order_points(
        &OrderPointsSpendRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            points: dec("400"),
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    // The confirm arrives late.
    let out = svc
        .grant_order_points(
            &OrderPointsGrantRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                grant_base_amount: dec("5000"),
                coupon_code_id: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();
    assert!(!out.already); // a fresh ledger entry was claimed — this is a real grant
    assert_eq!(out.points, dec("500"));
    assert!(out.entry_id.is_some());

    let row = sqlx::query_as::<_, (Decimal, Decimal)>(
        "SELECT granted_points, spent_points FROM promo.loyalty_order_points WHERE order_ref_id=$1",
    )
    .bind(order)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, dec("500"));
    assert_eq!(row.1, dec("400"));
    // 2000 − 400 + 500 = 2100.
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("2100"));
}

/// Cross-legacy dedupe: a bare `accrue` that already used this order's source key means the order
/// verb records the grant on the row but writes NO second earn and publishes nothing.
#[tokio::test]
async fn legacy_accrue_dedupes_grant() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    // The legacy event-driven path already earned for this exact source.
    svc.accrue(
        &AccrualRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            purchase_amount: dec("10000"),
            source_type: "pos_order".into(),
            source_id: order,
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    let out = svc
        .grant_order_points(
            &OrderPointsGrantRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                grant_base_amount: dec("10000"),
                coupon_code_id: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();
    assert!(out.already);
    assert_eq!(out.entry_id, None); // no second earn
    assert_eq!(out.points, dec("1000"));
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("1000")); // not doubled
    assert_eq!(sink.count_matching(|e| matches!(e, PromoEvent::LoyaltyOrderPointsGranted(_))), 0);
}

/// Reversing an order with no loyalty accounting is a typed refusal, not a silent no-op.
#[tokio::test]
async fn reversal_unknown_order_refused() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;

    let err = svc
        .reverse_order_points(
            &OrderPointsReversalRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: Uuid::new_v4(),
                order_ref_type: "pos_order".into(),
                order_ref_id: Uuid::new_v4(),
                reversal_ref_type: "pos_return".into(),
                reversal_ref_id: Uuid::new_v4(),
                return_amount: None,
                at: now(),
            },
            &LoggingSink,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PricingError::Invalid(_)));
}

/// A spend restoration copies the order's original earned expiry, so a restored point lapses when
/// the point it restores would have.
#[tokio::test]
async fn spend_restoration_copies_earned_expiry() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, Some(30)).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    svc.grant_order_points(
        &OrderPointsGrantRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            grant_base_amount: dec("10000"),
            coupon_code_id: None,
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();
    svc.spend_order_points(
        &OrderPointsSpendRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            points: dec("100"),
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();

    let ret = Uuid::new_v4();
    let out = svc
        .reverse_order_points(
            &OrderPointsReversalRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                reversal_ref_type: "pos_return".into(),
                reversal_ref_id: ret,
                return_amount: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();
    assert_eq!(out.grant_reversed, dec("900"));
    assert_eq!(out.spend_restored, dec("100"));

    // The restored leg carries the earned entry's expiry (≈30 days out), and the restored points
    // count as available until then.
    let restored_expiry: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        r#"SELECT expiry_date FROM promo.loyalty_point_entries
           WHERE company_id=$1 AND source_type='pos_return' AND source_id=$2
             AND entry_type='spend_reversed'"#,
    )
    .bind(company)
    .bind(ret)
    .fetch_one(&pool)
    .await
    .unwrap();
    let exp = restored_expiry.expect("restored leg must carry the earned expiry");
    assert!(exp > now() + chrono::Duration::days(29));

    let (available, _) = balances_at(&pool, company, customer, program_id, now()).await;
    assert_eq!(available, dec("100")); // 1000 − 100 − 900 + 100, all unexpired
}

/// A grant reversal copies the order's original earned expiry too, so the negation lapses exactly
/// when the grant it negates would. A NULL-expiry reversal row outlives the grant, drags the
/// expiry-aware balance negative once the grant lapses, and the conservation backstop then turns
/// every later write on the member into a trigger failure.
#[tokio::test]
async fn grant_reversal_copies_earned_expiry() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, Some(30)).await;
    let order = Uuid::new_v4();
    let sink = RecSink::default();

    svc.grant_order_points(
        &OrderPointsGrantRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order,
            grant_base_amount: dec("10000"),
            coupon_code_id: None,
            at: now(),
        },
        &sink,
    )
    .await
    .unwrap();
    let ret = Uuid::new_v4();
    let out = svc
        .reverse_order_points(
            &OrderPointsReversalRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: order,
                reversal_ref_type: "pos_return".into(),
                reversal_ref_id: ret,
                return_amount: None,
                at: now(),
            },
            &sink,
        )
        .await
        .unwrap();
    assert_eq!(out.grant_reversed, dec("1000"));
    assert_eq!(out.spend_restored, dec("0"));

    // The negation leg carries the earned entry's expiry — not NULL.
    let reversal_expiry: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        r#"SELECT expiry_date FROM promo.loyalty_point_entries
           WHERE company_id=$1 AND source_type='pos_return' AND source_id=$2
             AND entry_type='grant_reversed'"#,
    )
    .bind(company)
    .bind(ret)
    .fetch_one(&pool)
    .await
    .unwrap();
    let exp = reversal_expiry.expect("grant_reversed leg must carry the earned expiry");
    assert!(exp > now() + chrono::Duration::days(29));

    // Past the expiry the member nets to exactly zero — available never goes negative.
    let past = now() + chrono::Duration::days(31);
    let (available, lapsed) = balances_at(&pool, company, customer, program_id, past).await;
    assert_eq!(available, dec("0"));
    assert_eq!(lapsed, dec("0"));

    // Conservation stays neutral: later writes on the same member succeed (no trigger failure).
    let order2 = Uuid::new_v4();
    svc.grant_order_points(
        &OrderPointsGrantRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order2,
            grant_base_amount: dec("10000"),
            coupon_code_id: None,
            at: past,
        },
        &sink,
    )
    .await
    .unwrap();
    svc.spend_order_points(
        &OrderPointsSpendRequest {
            company_id: company,
            loyalty_program_id: program_id,
            customer_id: customer,
            order_ref_type: "pos_order".into(),
            order_ref_id: order2,
            points: dec("100"),
            at: past,
        },
        &sink,
    )
    .await
    .unwrap();
}

// ---- NOWAIT serialization contract --------------------------------------------------------------
//
// Each probe holds the real lock in a plain `FOR UPDATE` transaction of its own, then drives the
// verb and asserts the typed refusal — proving the verb actually takes that lock, fail-fast.

/// A held member anchor refuses a concurrent spend as LockBusy{MemberBalance}.
#[tokio::test]
async fn nowait_held_anchor_refuses_spend() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    seed_anchor(&pool, company, customer, program_id).await;

    let mut holder = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT id FROM promo.loyalty_member_anchors
         WHERE company_id=$1 AND customer_id=$2 AND loyalty_program_id=$3 FOR UPDATE",
    )
    .bind(company)
    .bind(customer)
    .bind(program_id)
    .execute(&mut *holder)
    .await
    .unwrap();

    let err = svc
        .spend_order_points(
            &OrderPointsSpendRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: Uuid::new_v4(),
                points: dec("100"),
                at: now(),
            },
            &LoggingSink,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        PricingError::LockBusy { resource: LockResource::MemberBalance }
    ));
    holder.rollback().await.unwrap();
}

/// A held program row refuses a concurrent spend as LockBusy{LoyaltyProgram} — the FIRST lock the
/// verb takes, proving the program→anchor order.
#[tokio::test]
async fn nowait_held_program_row_refuses_spend() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;

    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM promo.loyalty_programs WHERE id=$1 FOR UPDATE")
        .bind(program_id)
        .execute(&mut *holder)
        .await
        .unwrap();

    let err = svc
        .spend_order_points(
            &OrderPointsSpendRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: Uuid::new_v4(),
                points: dec("100"),
                at: now(),
            },
            &LoggingSink,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        PricingError::LockBusy { resource: LockResource::LoyaltyProgram }
    ));
    holder.rollback().await.unwrap();
}

/// A held member anchor refuses a concurrent grant the same way.
#[tokio::test]
async fn nowait_held_anchor_refuses_grant() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = std_program(&pool, company, None).await;
    seed_anchor(&pool, company, customer, program_id).await;

    let mut holder = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT id FROM promo.loyalty_member_anchors
         WHERE company_id=$1 AND customer_id=$2 AND loyalty_program_id=$3 FOR UPDATE",
    )
    .bind(company)
    .bind(customer)
    .bind(program_id)
    .execute(&mut *holder)
    .await
    .unwrap();

    let err = svc
        .grant_order_points(
            &OrderPointsGrantRequest {
                company_id: company,
                loyalty_program_id: program_id,
                customer_id: customer,
                order_ref_type: "pos_order".into(),
                order_ref_id: Uuid::new_v4(),
                grant_base_amount: dec("10000"),
                coupon_code_id: None,
                at: now(),
            },
            &LoggingSink,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        PricingError::LockBusy { resource: LockResource::MemberBalance }
    ));
    holder.rollback().await.unwrap();
}

/// A held coupon row refuses a concurrent burn as LockBusy{CouponCode}.
#[tokio::test]
async fn nowait_held_coupon_refuses_burn() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let rule_id = pct_rule(&pool, company, item, 0, "10").await;
    let coupon_id = coupon(&pool, company, "LOCKED", rule_id, Some(5)).await;

    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM promo.coupon_codes WHERE id=$1 FOR UPDATE")
        .bind(coupon_id)
        .execute(&mut *holder)
        .await
        .unwrap();

    let err = svc
        .commit_coupon_redemption(company, coupon_id, "sale", Uuid::new_v4(), &LoggingSink)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        PricingError::LockBusy { resource: LockResource::CouponCode }
    ));
    holder.rollback().await.unwrap();
}

/// The retry-storm bound: 8 concurrent DISTINCT-source burns against a max_use=4 coupon — exactly 4
/// commit, the rest refuse (LockBusy or CouponExhausted), `used_count` lands on 4, and the ledger
/// holds exactly 4 rows. The lock makes losers fail fast instead of queueing behind the winner.
#[tokio::test]
async fn coupon_retry_storm_never_over_burns() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let rule_id = pct_rule(&pool, company, item, 0, "10").await;
    let coupon_id = coupon(&pool, company, "STORM", rule_id, Some(4)).await;

    let svc = std::sync::Arc::new(PromoWriteService::new(pool.clone()));
    // The sink is not Sync, so the verb futures are !Send — run the storm on a LocalSet (the tasks
    // still interleave at every await point, so the pool-level lock contention is real). Each
    // contender plays the HOST contract: a LockBusy is retried (same source, fresh transaction)
    // until it gets a terminal answer, so the counts assert what the burn BOUND is, not what one
    // unlucky interleaving dropped.
    let set = tokio::task::LocalSet::new();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let svc = svc.clone();
        handles.push(set.spawn_local(async move {
            let source = Uuid::new_v4();
            // Retry LockBusy (same source — idempotent — fresh transaction each time). The backoff
            // grows per attempt so the contenders stagger instead of stampeding the row together
            // every wake; a terminal answer is returned as-is.
            for attempt in 1..=10u64 {
                match svc
                    .commit_coupon_redemption(company, coupon_id, "sale", source, &LoggingSink)
                    .await
                {
                    Ok(rule) => return Ok(rule),
                    Err(PricingError::LockBusy { .. }) => {
                        tokio::time::sleep(std::time::Duration::from_millis(15 * attempt)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            unreachable!("staggered retries always produce a terminal answer")
        }));
    }
    set.await;
    let mut ok = 0usize;
    let mut refused = 0usize;
    for h in handles {
        match h.await.unwrap() {
            Ok(_) => ok += 1,
            Err(PricingError::CouponExhausted) => refused += 1,
            Err(PricingError::LockBusy { .. }) => refused += 1,
            Err(e) => panic!("unexpected error in storm: {e}"),
        }
    }
    assert_eq!(ok, 4, "exactly max_use burns may commit");
    assert_eq!(refused, 4);

    let used: i32 = sqlx::query_scalar("SELECT used_count FROM promo.coupon_codes WHERE id=$1")
        .bind(coupon_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(used, 4);
    let ledger: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM promo.coupon_redemptions WHERE coupon_id=$1",
    )
    .bind(coupon_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger, 4);
}
