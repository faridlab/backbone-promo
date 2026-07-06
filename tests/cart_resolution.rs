//! Cart-scoped resolution (ADR-002) — the numeric oracle for `resolve_cart`: order-total minimums,
//! bundling, allocation with penny reconciliation, stacking policy, and the scope isolation that
//! keeps order rules out of the single-line seam. Money is IDR (2dp, half-away-from-zero).

mod common;

use backbone_promo::application::service::promo_ports::{
    AdjustmentSource, CartLine, CartQuery, PriceQuery,
};
use backbone_promo::application::service::promo_write_service::PromoWriteService;
use common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

/// A cart line with just the dimensions the resolver matches on.
fn line(company: Uuid, item: Uuid, list: &str, qty: &str) -> CartLine {
    CartLine {
        line_id: Uuid::new_v4(),
        query: PriceQuery {
            company_id: company,
            list_price: dec(list),
            quantity: dec(qty),
            item_id: item,
            item_group_id: None,
            brand_id: None,
            customer_id: None,
            customer_group_id: None,
            coupon_code: None,
            at: now(),
        },
    }
}

fn cart(company: Uuid, lines: Vec<CartLine>) -> CartQuery {
    CartQuery {
        company_id: company,
        customer_id: None,
        customer_group_id: None,
        coupon_code: None,
        lines,
        at: now(),
    }
}

/// Σ of every line's allocated share must equal the adjustment's headline discount, exactly.
fn assert_shares_tie_out(cart_result: &backbone_promo::application::service::promo_ports::ResolvedCart) {
    for adj in &cart_result.order_adjustments {
        let sum: Decimal = adj.allocated.iter().map(|(_, s)| *s).sum();
        assert_eq!(sum, adj.discount_amount, "allocation shares must sum to the discount exactly");
    }
    // Line shares must also sum to the order discount total.
    let line_sum: Decimal = cart_result.lines.iter().map(|l| l.order_discount_share).sum();
    assert_eq!(line_sum, cart_result.order_discount_total);
    assert_eq!(cart_result.total, cart_result.subtotal - cart_result.order_discount_total);
    // CONSERVATION (council 2026-07-06): the per-line NETs selling/POS actually post must sum to the
    // cart total. Capacity-aware allocation guarantees no line's share exceeds its gross, so nothing
    // is lost to a clamp.
    let net_sum: Decimal = cart_result.lines.iter().map(|l| l.net_line_total).sum();
    assert_eq!(net_sum, cart_result.total, "Σ net_line_total must equal the cart total");
}

/// CART-1 — an order-total-minimum rule fires on the subtotal and is allocated ∝ line gross.
#[tokio::test]
async fn cart1_order_total_minimum_fires_and_allocates() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    // Spend ≥ 250k → 10% off the whole order.
    order_rule(&pool, company, 0, "250000", "discount_percentage", Some(dec("10")), None, false, None).await;

    let c = cart(company, vec![
        line(company, a, "100000", "1"),
        line(company, b, "200000", "1"),
    ]);
    let r = svc.resolve_cart(&c).await.unwrap();

    assert_eq!(r.subtotal, dec("300000.00"));
    assert_eq!(r.order_discount_total, dec("30000.00")); // 10% of 300k
    assert_eq!(r.total, dec("270000.00"));
    assert_eq!(r.order_adjustments.len(), 1);
    assert!(matches!(r.order_adjustments[0].source, AdjustmentSource::OrderRule(_)));
    // A:B gross = 100k:200k → shares 10k:20k.
    let share = |item: Uuid| r.lines.iter().find(|l| l.item_id == item).unwrap().order_discount_share;
    assert_eq!(share(a), dec("10000.00"));
    assert_eq!(share(b), dec("20000.00"));
    assert_shares_tie_out(&r);
}

/// CART-2 — below the threshold, an order rule does not fire (cart is pass-through).
#[tokio::test]
async fn cart2_order_total_minimum_not_met() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let a = Uuid::new_v4();
    order_rule(&pool, company, 0, "250000", "discount_percentage", Some(dec("10")), None, false, None).await;

    let c = cart(company, vec![line(company, a, "100000", "1")]); // subtotal 100k < 250k
    let r = svc.resolve_cart(&c).await.unwrap();

    assert_eq!(r.subtotal, dec("100000.00"));
    assert_eq!(r.order_discount_total, Decimal::ZERO);
    assert_eq!(r.total, dec("100000.00"));
    assert!(r.order_adjustments.is_empty());
}

/// CART-3 — allocation penny reconciliation: a fixed amount that doesn't divide evenly still ties out.
#[tokio::test]
async fn cart3_allocation_penny_reconciliation() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b, cc) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    // 10000 fixed off the order, spread over three EQUAL lines → 3333.33 each, 0.01 remainder.
    order_rule(&pool, company, 0, "0", "discount_amount", None, Some(dec("10000")), false, None).await;

    let c = cart(company, vec![
        line(company, a, "100000", "1"),
        line(company, b, "100000", "1"),
        line(company, cc, "100000", "1"),
    ]);
    let r = svc.resolve_cart(&c).await.unwrap();

    assert_eq!(r.order_discount_total, dec("10000.00"));
    // Shares sum EXACTLY to 10000 despite the indivisible split.
    assert_shares_tie_out(&r);
    // Two lines at 3333.33, one carries the +0.01 remainder at 3333.34.
    let mut shares: Vec<Decimal> = r.lines.iter().map(|l| l.order_discount_share).collect();
    shares.sort();
    assert_eq!(shares, vec![dec("3333.33"), dec("3333.33"), dec("3333.34")]);
}

/// CART-4 — an all_of bundle (buy A + B) discounts the matched lines' value.
#[tokio::test]
async fn cart4_bundle_all_of() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let bid = bundle(&pool, company, 0, "all_of", None, "discount_percentage", Some(dec("10")), None, "0", false).await;
    bundle_component(&pool, company, bid, a, "1").await;
    bundle_component(&pool, company, bid, b, "1").await;

    let c = cart(company, vec![
        line(company, a, "100000", "1"),
        line(company, b, "50000", "1"),
    ]);
    let r = svc.resolve_cart(&c).await.unwrap();

    assert_eq!(r.subtotal, dec("150000.00"));
    assert_eq!(r.order_discount_total, dec("15000.00")); // 10% of 150k matched
    assert_eq!(r.total, dec("135000.00"));
    assert_eq!(r.order_adjustments.len(), 1);
    assert_eq!(r.order_adjustments[0].source, AdjustmentSource::Bundle(bid));
    assert_shares_tie_out(&r);
}

/// CART-5 — an all_of bundle with a missing component does not fire.
#[tokio::test]
async fn cart5_bundle_all_of_not_satisfied() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let bid = bundle(&pool, company, 0, "all_of", None, "discount_percentage", Some(dec("10")), None, "0", false).await;
    bundle_component(&pool, company, bid, a, "1").await;
    bundle_component(&pool, company, bid, b, "1").await;

    // Only A in the cart — B is absent.
    let c = cart(company, vec![line(company, a, "100000", "1")]);
    let r = svc.resolve_cart(&c).await.unwrap();

    assert_eq!(r.order_discount_total, Decimal::ZERO);
    assert!(r.order_adjustments.is_empty());
}

/// CART-6 — an any_n bundle fires when `required_distinct` components are present.
#[tokio::test]
async fn cart6_bundle_any_n() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b, cc) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    // Any 2 of {A,B,C} → 20000 off.
    let bid = bundle(&pool, company, 0, "any_n", Some(2), "discount_amount", None, Some(dec("20000")), "0", false).await;
    bundle_component(&pool, company, bid, a, "1").await;
    bundle_component(&pool, company, bid, b, "1").await;
    bundle_component(&pool, company, bid, cc, "1").await;

    // A and B present, C absent → 2 distinct → fires.
    let c = cart(company, vec![
        line(company, a, "100000", "1"),
        line(company, b, "100000", "1"),
    ]);
    let r = svc.resolve_cart(&c).await.unwrap();

    assert_eq!(r.order_discount_total, dec("20000.00"));
    assert_eq!(r.total, dec("180000.00"));
    assert_shares_tie_out(&r);
}

/// CART-7 — a per-line rule still applies per line inside a cart; unit price drops, no order adj.
#[tokio::test]
async fn cart7_line_rule_still_applies_in_cart() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let a = Uuid::new_v4();
    pct_rule(&pool, company, a, 0, "20").await; // 20% off item A (scope=line by default)

    let c = cart(company, vec![line(company, a, "100000", "2")]);
    let r = svc.resolve_cart(&c).await.unwrap();

    assert_eq!(r.lines[0].unit_price, dec("80000.00")); // 20% off applied per line
    assert!(r.lines[0].applied_rule_id.is_some());
    assert_eq!(r.subtotal, dec("160000.00")); // 80k · 2
    assert!(r.order_adjustments.is_empty());
    assert_eq!(r.total, dec("160000.00"));
}

/// CART-8a — a non-stackable order rule is exclusive: the highest-priority one wins alone.
#[tokio::test]
async fn cart8a_non_stackable_is_exclusive() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let a = Uuid::new_v4();
    // R1 priority 10 non-stackable 10%; R2 priority 5 stackable 5%.
    order_rule(&pool, company, 10, "0", "discount_percentage", Some(dec("10")), None, false, None).await;
    order_rule(&pool, company, 5, "0", "discount_percentage", Some(dec("5")), None, true, None).await;

    let c = cart(company, vec![line(company, a, "100000", "1")]);
    let r = svc.resolve_cart(&c).await.unwrap();

    // Only R1 (10%) applies; R2 cannot stack onto an exclusive winner.
    assert_eq!(r.order_adjustments.len(), 1);
    assert_eq!(r.order_discount_total, dec("10000.00"));
    assert_eq!(r.total, dec("90000.00"));
}

/// CART-8b — two stackable order rules combine, each computed on the running remainder.
#[tokio::test]
async fn cart8b_stackable_rules_combine() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let a = Uuid::new_v4();
    order_rule(&pool, company, 10, "0", "discount_percentage", Some(dec("10")), None, true, None).await;
    order_rule(&pool, company, 5, "0", "discount_percentage", Some(dec("5")), None, true, None).await;

    let c = cart(company, vec![line(company, a, "100000", "1")]);
    let r = svc.resolve_cart(&c).await.unwrap();

    // 10% of 100k = 10k; then 5% of the remaining 90k = 4.5k → 14.5k total.
    assert_eq!(r.order_adjustments.len(), 2);
    assert_eq!(r.order_discount_total, dec("14500.00"));
    assert_eq!(r.total, dec("85500.00"));
    assert_shares_tie_out(&r);
}

/// CART-9 — an order rule scoped to a customer group only fires for that group.
#[tokio::test]
async fn cart9_order_rule_customer_group_scoped() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let a = Uuid::new_v4();
    let vip = Uuid::new_v4();
    order_rule(&pool, company, 0, "0", "discount_percentage", Some(dec("10")), None, false, Some(vip)).await;

    // Non-VIP cart: rule does not apply.
    let mut c = cart(company, vec![line(company, a, "100000", "1")]);
    let r = svc.resolve_cart(&c).await.unwrap();
    assert_eq!(r.order_discount_total, Decimal::ZERO);

    // VIP cart: rule applies.
    c.customer_group_id = Some(vip);
    let r = svc.resolve_cart(&c).await.unwrap();
    assert_eq!(r.order_discount_total, dec("10000.00"));
}

/// CART-10 — scope isolation: an order rule NEVER leaks into the single-line `resolve` seam.
#[tokio::test]
async fn cart10_order_rule_absent_from_single_line_resolve() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let a = Uuid::new_v4();
    order_rule(&pool, company, 0, "0", "discount_percentage", Some(dec("50")), None, false, None).await;

    // Single-line resolve must ignore scope=order rules → pass-through to list price.
    let q = PriceQuery {
        company_id: company,
        list_price: dec("100000"),
        quantity: dec("1"),
        item_id: a,
        item_group_id: None,
        brand_id: None,
        customer_id: None,
        customer_group_id: None,
        coupon_code: None,
        at: now(),
    };
    let r = svc.resolve(&q).await.unwrap();
    assert_eq!(r.unit_price, dec("100000")); // untouched by the order rule
    assert_eq!(r.applied_rule_id, None);
}

/// CART-15 (council 2026-07-06) — a 100%-off stackable bundle on ONE of two lines PLUS a stackable
/// order rule. Before the capacity-aware allocation fix the order rule spread ∝ gross over both lines,
/// over-allocating the bundle-zeroed line; reconcile clamped it and lost 25,000, so the per-line nets
/// summed to 75,000 while the cart total was 50,000. Now allocation weights by REMAINING capacity, so
/// the order discount lands entirely on the line that can hold it and Σ net == total.
#[tokio::test]
async fn cart15_bundle_plus_stackable_order_rule_conserves() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());

    // 100%-off stackable bundle on item A alone.
    let bid = bundle(&pool, company, 10, "all_of", None, "discount_percentage", Some(dec("100")), None, "0", true).await;
    bundle_component(&pool, company, bid, a, "1").await;
    // A stackable 50%-off order rule.
    order_rule(&pool, company, 0, "0", "discount_percentage", Some(dec("50")), None, true, None).await;

    let line_a = line(company, a, "100000", "1");
    let line_b = line(company, b, "100000", "1");
    let (id_a, id_b) = (line_a.line_id, line_b.line_id);
    let r = svc.resolve_cart(&cart(company, vec![line_a, line_b])).await.unwrap();

    assert_eq!(r.subtotal, dec("200000.00"));
    assert_eq!(r.order_discount_total, dec("150000.00")); // 100k bundle + 50k order rule
    assert_eq!(r.total, dec("50000.00"));
    let net = |id| r.lines.iter().find(|l| l.line_id == id).unwrap().net_line_total;
    assert_eq!(net(id_a), dec("0.00"), "A: fully discounted by the bundle");
    assert_eq!(net(id_b), dec("50000.00"), "B: absorbs the whole order-rule share");
    assert_shares_tie_out(&r); // includes Σ net == total
}

/// Seed a buy-X-get-Y bundle: satisfying its components grants `reward_qty × sets` free `reward_item`.
async fn free_bundle(
    pool: &sqlx::PgPool,
    company: Uuid,
    reward_item: Uuid,
    reward_qty: &str,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO promo.promo_bundles
             (company_id, title, priority, match_type, reward, reward_item_id, reward_qty,
              min_order_amount, stackable, valid_from, is_active)
           VALUES ($1,'free-bundle',0,'all_of'::bundle_match,'discount_percentage'::rate_or_discount,
                   $2,$3,'0',false,now() - interval '1 day', true)
           RETURNING id"#,
    )
    .bind(company)
    .bind(reward_item)
    .bind(dec(reward_qty))
    .fetch_one(pool)
    .await
    .unwrap()
}

/// CART-16 (ADR-002 free-line, 2026-07-06) — buy A, get 1 free B: a satisfied buy-X-get-Y bundle emits
/// a zero-priced reward line for the free item WITHOUT discounting the basket (the total is unchanged).
#[tokio::test]
async fn cart16_buy_x_get_y_free_line() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (item_a, free_b) = (Uuid::new_v4(), Uuid::new_v4());
    let bid = free_bundle(&pool, company, free_b, "1").await;
    bundle_component(&pool, company, bid, item_a, "1").await; // buy 1 A

    let r = svc.resolve_cart(&cart(company, vec![line(company, item_a, "100000", "1")])).await.unwrap();

    // A is charged in full; the bundle adds a free B, not a discount.
    assert_eq!(r.subtotal, dec("100000.00"));
    assert_eq!(r.order_discount_total, Decimal::ZERO);
    assert_eq!(r.total, dec("100000.00"));
    assert_eq!(r.reward_lines.len(), 1);
    assert_eq!(r.reward_lines[0].item_id, free_b);
    assert_eq!(r.reward_lines[0].quantity, dec("1.0000"));
    assert_eq!(r.reward_lines[0].bundle_id, bid);
    assert_shares_tie_out(&r); // conservation still holds (free line is separate)
}
