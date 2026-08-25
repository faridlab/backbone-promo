//! Golden cases — the numeric oracle for price resolution. Money is IDR (2dp, half-away-from-zero).
//! Mirrors docs/business-flows/golden-cases.md.

mod common;

use backbone_promo::application::service::promo_ports::{CartLine, CartQuery, PriceQuery};
use backbone_promo::application::service::promo_write_service::PromoWriteService;
use common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

fn q(company: Uuid, item: Uuid, list: &str, qty: &str) -> PriceQuery {
    PriceQuery {
        company_id: company,
        list_price: dec(list),
        quantity: dec(qty),
        item_id: item,
        item_group_id: None,
        brand_id: None,
        customer_id: None,
        customer_group_id: None,
        coupon_code: None,
        tax_key: None,
        at: now(),
    }
}

/// PGC-1 — a 10% discount rule resolves the unit price down 10%.
#[tokio::test]
async fn pgc1_percentage_discount() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let rule_id = pct_rule(&pool, company, item, 0, "10").await;

    let r = svc.resolve(&q(company, item, "100000", "2")).await.unwrap();
    assert_eq!(r.unit_price, dec("90000.00"));
    assert_eq!(r.discount_amount, dec("10000.00"));
    assert_eq!(r.applied_rule_id, Some(rule_id));
}

/// PGC-2 — a rate override replaces the list price outright.
#[tokio::test]
async fn pgc2_rate_override() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    rule(&pool, RuleSpec {
        rate_or_discount: "rate",
        rate: Some(dec("75000")),
        ..RuleSpec::for_item(company, item)
    })
    .await;

    let r = svc.resolve(&q(company, item, "100000", "1")).await.unwrap();
    assert_eq!(r.unit_price, dec("75000.00"));
    assert_eq!(r.discount_amount, dec("25000.00"));
}

/// PGC-3 — a fixed per-unit discount, floored at zero (never a negative price).
#[tokio::test]
async fn pgc3_amount_discount_floors_at_zero() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    rule(&pool, RuleSpec {
        rate_or_discount: "discount_amount",
        discount_amount: Some(dec("150000")), // exceeds the 100k list
        ..RuleSpec::for_item(company, item)
    })
    .await;

    let r = svc.resolve(&q(company, item, "100000", "1")).await.unwrap();
    assert_eq!(r.unit_price, Decimal::ZERO);
    assert_eq!(r.discount_amount, dec("100000.00"));
}

/// PGC-4 — on a priority tie the more specific rule wins (item beats storewide `all`).
#[tokio::test]
async fn pgc4_specificity_breaks_priority_tie() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    // Both priority 5; the storewide rule gives 5%, the item rule gives 20%.
    rule(&pool, RuleSpec {
        apply_on: "all",
        item: None,
        priority: 5,
        discount_percentage: Some(dec("5")),
        ..RuleSpec::for_item(company, item)
    })
    .await;
    let item_rule = rule(&pool, RuleSpec {
        priority: 5,
        discount_percentage: Some(dec("20")),
        ..RuleSpec::for_item(company, item)
    })
    .await;

    let r = svc.resolve(&q(company, item, "100000", "1")).await.unwrap();
    assert_eq!(r.applied_rule_id, Some(item_rule));
    assert_eq!(r.unit_price, dec("80000.00"));
}

/// PGC-5 — conditions gate the rule: below min_qty / outside the window → passthrough (list price).
#[tokio::test]
async fn pgc5_conditions_gate_the_rule() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    // Rule needs qty >= 10.
    rule(&pool, RuleSpec {
        min_qty: dec("10"),
        discount_percentage: Some(dec("30")),
        ..RuleSpec::for_item(company, item)
    })
    .await;

    // qty 3 < 10 → no rule applies → charge list.
    let r = svc.resolve(&q(company, item, "100000", "3")).await.unwrap();
    assert_eq!(r.unit_price, dec("100000"));
    assert_eq!(r.applied_rule_id, None);

    // qty 10 → rule applies.
    let r2 = svc.resolve(&q(company, item, "100000", "10")).await.unwrap();
    assert_eq!(r2.unit_price, dec("70000.00"));
}

/// PGC-6 — a coupon-gated rule stays dormant until its coupon is presented.
#[tokio::test]
async fn pgc6_coupon_gate() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let rule_id = rule(&pool, RuleSpec {
        coupon_required: true,
        discount_percentage: Some(dec("25")),
        ..RuleSpec::for_item(company, item)
    })
    .await;
    coupon(&pool, company, "SAVE25", rule_id, Some(100)).await;

    // No coupon → gated rule ignored → list price.
    let mut query = q(company, item, "100000", "1");
    let r = svc.resolve(&query).await.unwrap();
    assert_eq!(r.applied_rule_id, None);

    // With the coupon → the gated rule applies and the coupon is reported.
    query.coupon_code = Some("save25".into()); // case-insensitive
    let r2 = svc.resolve(&query).await.unwrap();
    assert_eq!(r2.applied_rule_id, Some(rule_id));
    assert_eq!(r2.unit_price, dec("75000.00"));
    assert!(r2.applied_coupon_id.is_some());
}

/// PGC-7 — the tax-split golden: a pinned cart whose order discount crosses THREE tax groups.
/// These exact numbers (shares, per-group totals, line nets) are the regression baseline for the
/// per-tax-group allocation kernel; any change in the partition math must come with an explicit
/// decision, not drift.
#[tokio::test]
async fn pgc7_tax_split_golden() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b, cc) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    // 10% off the whole order, no cap.
    order_rule(&pool, company, 0, "0", "discount_percentage", Some(dec("10")), None, false, None).await;

    let mk = |item: Uuid, list: &str, key: Option<&str>| CartLine {
        line_id: Uuid::new_v4(),
        tax_key: key.map(str::to_string),
        query: PriceQuery {
            company_id: company,
            list_price: dec(list),
            quantity: Decimal::ONE,
            item_id: item,
            item_group_id: None,
            brand_id: None,
            customer_id: None,
            customer_group_id: None,
            coupon_code: None,
            tax_key: None,
            at: now(),
        },
    };
    let la = mk(a, "100000", Some("PPN"));
    let lb = mk(b, "33333", Some("PPN11"));
    let lc = mk(cc, "66667", None);
    let (ia, ib, ic) = (la.line_id, lb.line_id, lc.line_id);
    let r = svc
        .resolve_cart(&CartQuery {
            company_id: company,
            customer_id: None,
            customer_group_id: None,
            coupon_code: None,
            lines: vec![la, lb, lc],
            at: now(),
        })
        .await
        .unwrap();

    // Subtotal 200000 → 10% = 20000 exactly. Group slices ∝ capacity:
    //   PPN 100000 → 10000.00; PPN11 33333 → 3333.30; None 66667 → 6666.70. Σ = 20000.00.
    assert_eq!(r.subtotal, dec("200000.00"));
    assert_eq!(r.order_discount_total, dec("20000.00"));
    assert_eq!(r.total, dec("180000.00"));
    let share = |id: Uuid| {
        r.order_adjustments[0].allocated.iter().find(|s| s.line_id == id).unwrap().clone()
    };
    assert_eq!(share(ia).tax_key.as_deref(), Some("PPN"));
    assert_eq!(share(ia).share, dec("10000.00"));
    assert_eq!(share(ib).tax_key.as_deref(), Some("PPN11"));
    assert_eq!(share(ib).share, dec("3333.30"));
    assert_eq!(share(ic).tax_key, None);
    assert_eq!(share(ic).share, dec("6666.70"));
    let group = |key: Option<&str>| {
        r.order_adjustments[0]
            .by_tax_group
            .iter()
            .find(|g| g.tax_key.as_deref() == key)
            .unwrap()
            .discount_amount
    };
    assert_eq!(group(Some("PPN")), dec("10000.00"));
    assert_eq!(group(Some("PPN11")), dec("3333.30"));
    assert_eq!(group(None), dec("6666.70"));
    // Line nets conserve to the total.
    let net = |id: Uuid| r.lines.iter().find(|l| l.line_id == id).unwrap().net_line_total;
    assert_eq!(net(ia) + net(ib) + net(ic), dec("180000.00"));
}
