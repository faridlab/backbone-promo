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
        tax_key: None,
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
            tax_key: None,
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

/// Σ of every line's allocated share must equal the adjustment's headline discount, exactly — and
/// the per-tax-group fold must agree with the per-line shares (D4).
fn assert_shares_tie_out(cart_result: &backbone_promo::application::service::promo_ports::ResolvedCart) {
    for adj in &cart_result.order_adjustments {
        let sum: Decimal = adj.allocated.iter().map(|s| s.share).sum();
        assert_eq!(sum, adj.discount_amount, "allocation shares must sum to the discount exactly");
        let group_sum: Decimal = adj.by_tax_group.iter().map(|g| g.discount_amount).sum();
        assert_eq!(group_sum, adj.discount_amount, "by_tax_group must fold to the same total");
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
        tax_key: None,
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
              min_order_amount, stackable, valid_from, status)
           VALUES ($1,'free-bundle',0,'all_of'::bundle_match,'discount_percentage'::rate_or_discount,
                   $2,$3,'0',false,now() - interval '1 day', 'active')
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

/// CART-17 (ADR-003) — `min_order_qty`: a scope=order rule with a cart-wide item-count floor fires
/// only once the cart's total quantity clears it; below the floor it stays dormant (no discount).
#[tokio::test]
async fn cart17_order_min_qty_gate() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();

    // 10% off the cart, but only when the cart holds ≥ 5 units in total.
    order_rule_threshold(
        &pool, company, 0, "0", Some(dec("5")),
        "discount_percentage", Some(dec("10")), None, None, false,
    )
    .await;

    // Below the floor: 2 lines × 2 units = 4 < 5 → rule does not fire.
    let under = svc
        .resolve_cart(&cart(company, vec![
            line(company, item, "100000", "2"),
            line(company, item, "100000", "2"),
        ]))
        .await
        .unwrap();
    assert_eq!(under.subtotal, dec("400000.00"));
    assert_eq!(under.order_discount_total, Decimal::ZERO, "below min_order_qty: no discount");
    assert_eq!(under.total, dec("400000.00"));

    // At/above the floor: 2 lines × 3 units = 6 ≥ 5 → rule fires, 10% off.
    let over = svc
        .resolve_cart(&cart(company, vec![
            line(company, item, "100000", "3"),
            line(company, item, "100000", "3"),
        ]))
        .await
        .unwrap();
    assert_eq!(over.subtotal, dec("600000.00"));
    assert_eq!(over.order_discount_total, dec("60000.00"), "above min_order_qty: 10% off");
    assert_eq!(over.total, dec("540000.00"));
    assert_shares_tie_out(&over);
}

/// CART-18 (ADR-003) — `discount_upto`: a percentage order discount is clamped at its Rp ceiling.
/// 10% off a 1,000,000 subtotal would be 100,000, but the cap limits it to 50,000; conservation holds.
#[tokio::test]
async fn cart18_order_discount_upto_cap() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();

    // 10% off the cart, capped at 50,000.
    order_rule_threshold(
        &pool, company, 0, "0", None,
        "discount_percentage", Some(dec("10")), None, Some(dec("50000")), false,
    )
    .await;

    let r = svc
        .resolve_cart(&cart(company, vec![line(company, item, "1000000", "1")]))
        .await
        .unwrap();
    assert_eq!(r.subtotal, dec("1000000.00"));
    assert_eq!(r.order_discount_total, dec("50000.00"), "10% would be 100k; capped at 50k");
    assert_eq!(r.total, dec("950000.00"));
    assert_shares_tie_out(&r);
}

/// CART-19 (ADR-005) — multi-gift bundle: buying the qualifying product grants MULTIPLE distinct free
/// gifts (Shopee-style "buy A, get gift(s)"), each as its own zero-priced RewardLine. The basket total
/// is unchanged (gifts are extra goods, not a discount). The legacy single-gift path (reward_item_id,
/// covered by cart16) still works via the fallback when a bundle has no gift rows.
#[tokio::test]
async fn cart19_multi_gift_bundle() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (item_a, gift_b, gift_c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

    // Buy 1 A → get 1 free B and 2 free C (two distinct gifts on one bundle).
    let bid = bundle(&pool, company, 0, "all_of", None, "discount_percentage", None, None, "0", false).await;
    bundle_component(&pool, company, bid, item_a, "1").await;
    gift(&pool, company, bid, gift_b, "1").await;
    gift(&pool, company, bid, gift_c, "2").await;

    let r = svc
        .resolve_cart(&cart(company, vec![line(company, item_a, "100000", "1")]))
        .await
        .unwrap();

    assert_eq!(r.subtotal, dec("100000.00"));
    assert_eq!(r.order_discount_total, Decimal::ZERO, "gifts are free goods, not a discount");
    assert_eq!(r.total, dec("100000.00"), "basket total unchanged");
    assert_eq!(r.reward_lines.len(), 2, "one RewardLine per gift");
    let qty_of = |item: Uuid| {
        r.reward_lines.iter().filter(|rl| rl.item_id == item).map(|rl| rl.quantity).sum::<Decimal>()
    };
    assert_eq!(qty_of(gift_b), dec("1.0000"), "1 free B per satisfied set");
    assert_eq!(qty_of(gift_c), dec("2.0000"), "2 free C per satisfied set");
    assert!(r.reward_lines.iter().all(|rl| rl.bundle_id == bid), "all gifts attributed to the bundle");
    assert_shares_tie_out(&r);
}

// ---- Per-tax-group allocation (D4) ----------------------------------------------------------------
//
// Order-level discounts are partitioned by each line's effective tax-group key so a consumer can
// reduce each tax base without re-deriving anything. Every case still satisfies the conservation
// oracle (`assert_shares_tie_out`, which also checks the by-tax fold).

/// A cart line carrying an explicit tax-group key (None = the caller has no tax split).
fn line_tax(company: Uuid, item: Uuid, list: &str, qty: &str, tax_key: Option<&str>) -> CartLine {
    CartLine {
        line_id: Uuid::new_v4(),
        tax_key: tax_key.map(str::to_string),
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
            tax_key: None,
            at: now(),
        },
    }
}

/// The discount share carried on `order_adjustments` for one line, with its tax key.
fn share_of(r: &backbone_promo::application::service::promo_ports::ResolvedCart, line_id: Uuid) -> (Option<String>, Decimal) {
    let share = r.order_adjustments[0]
        .allocated
        .iter()
        .find(|s| s.line_id == line_id)
        .expect("line has a share");
    (share.tax_key.clone(), share.share)
}

/// The adjustment's per-tax-group total for one key.
fn group_total(
    r: &backbone_promo::application::service::promo_ports::ResolvedCart,
    key: Option<&str>,
) -> Decimal {
    r.order_adjustments[0]
        .by_tax_group
        .iter()
        .find(|g| g.tax_key.as_deref() == key)
        .map(|g| g.discount_amount)
        .unwrap_or(Decimal::ZERO)
}

/// TAX-1 — a 10% order rule over two tax groups splits exactly 10% per group; every share carries
/// its line's key and the ResolvedLine echoes it.
#[tokio::test]
async fn tax1_two_group_split() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    order_rule(&pool, company, 0, "0", "discount_percentage", Some(dec("10")), None, false, None).await;

    let la = line_tax(company, a, "100000", "1", Some("PPN"));
    let lb = line_tax(company, b, "50000", "1", Some("FREE"));
    let id_a = la.line_id;
    let id_b = lb.line_id;
    let r = svc.resolve_cart(&cart(company, vec![la, lb])).await.unwrap();

    assert_eq!(r.subtotal, dec("150000.00"));
    assert_eq!(r.order_discount_total, dec("15000.00"));
    assert_eq!(share_of(&r, id_a), (Some("PPN".into()), dec("10000.00")));
    assert_eq!(share_of(&r, id_b), (Some("FREE".into()), dec("5000.00")));
    assert_eq!(group_total(&r, Some("PPN")), dec("10000.00"));
    assert_eq!(group_total(&r, Some("FREE")), dec("5000.00"));
    // The resolved lines echo their keys so consumers can group nets without re-joining input.
    let echo = |id: Uuid| r.lines.iter().find(|l| l.line_id == id).unwrap().tax_key.clone();
    assert_eq!(echo(id_a), Some("PPN".into()));
    assert_eq!(echo(id_b), Some("FREE".into()));
    assert_shares_tie_out(&r);
}

/// TAX-2 — cross-group penny fold: a fixed 100 off three equal lines in three groups takes
/// 33.33/33.33/33.34 (the partition's one rounding step folded onto the most-slack line) and the
/// per-tax fold matches exactly.
#[tokio::test]
async fn tax2_three_group_penny_fold() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b, cc) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    order_rule(&pool, company, 0, "0", "discount_amount", None, Some(dec("100")), false, None).await;

    let lines = vec![
        line_tax(company, a, "100000", "1", Some("A")),
        line_tax(company, b, "100000", "1", Some("B")),
        line_tax(company, cc, "100000", "1", Some("C")),
    ];
    let r = svc.resolve_cart(&cart(company, lines)).await.unwrap();
    assert_eq!(r.order_discount_total, dec("100.00"));
    let mut shares: Vec<Decimal> =
        r.order_adjustments[0].allocated.iter().map(|s| s.share).collect();
    shares.sort();
    assert_eq!(shares, vec![dec("33.33"), dec("33.33"), dec("33.34")]);
    assert_eq!(
        group_total(&r, Some("A")) + group_total(&r, Some("B")) + group_total(&r, Some("C")),
        dec("100.00")
    );
    assert_shares_tie_out(&r);
}

/// TAX-3 — degenerate single group is byte-identical to the tax-agnostic kernel: the same cart with
/// NO keys and with one shared key produces the exact pre-D4 shares (CART-3's numbers).
#[tokio::test]
async fn tax3_single_group_byte_identical() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b, cc) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    order_rule(&pool, company, 0, "0", "discount_amount", None, Some(dec("10000")), false, None).await;

    let bare = svc
        .resolve_cart(&cart(company, vec![
            line(company, a, "100000", "1"),
            line(company, b, "100000", "1"),
            line(company, cc, "100000", "1"),
        ]))
        .await
        .unwrap();
    let keyed = svc
        .resolve_cart(&cart(company, vec![
            line_tax(company, a, "100000", "1", Some("PPN")),
            line_tax(company, b, "100000", "1", Some("PPN")),
            line_tax(company, cc, "100000", "1", Some("PPN")),
        ]))
        .await
        .unwrap();

    // The pre-D4 pinned numbers (CART-3).
    let mut shares: Vec<Decimal> = bare.lines.iter().map(|l| l.order_discount_share).collect();
    shares.sort();
    assert_eq!(shares, vec![dec("3333.33"), dec("3333.33"), dec("3333.34")]);
    // One shared key changes nothing about the money.
    let mut keyed_shares: Vec<Decimal> = keyed.lines.iter().map(|l| l.order_discount_share).collect();
    keyed_shares.sort();
    assert_eq!(shares, keyed_shares);
    assert_eq!(bare.order_discount_total, keyed.order_discount_total);
    assert_eq!(keyed.order_adjustments[0].by_tax_group.len(), 1);
    assert_eq!(keyed.order_adjustments[0].by_tax_group[0].tax_key.as_deref(), Some("PPN"));
    assert_eq!(keyed.order_adjustments[0].by_tax_group[0].discount_amount, dec("10000.00"));
}

/// TAX-4 — a keyless line among keyed lines is its own group (`None`), so mixed callers are safe.
#[tokio::test]
async fn tax4_keyless_line_is_its_own_group() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    order_rule(&pool, company, 0, "0", "discount_percentage", Some(dec("10")), None, false, None).await;

    let la = line_tax(company, a, "100000", "1", Some("PPN"));
    let lb = line_tax(company, b, "100000", "1", None);
    let id_b = lb.line_id;
    let r = svc.resolve_cart(&cart(company, vec![la, lb])).await.unwrap();

    assert_eq!(group_total(&r, Some("PPN")), dec("10000.00"));
    assert_eq!(group_total(&r, None), dec("10000.00"));
    assert_eq!(share_of(&r, id_b), (None, dec("10000.00")));
    assert_eq!(r.order_adjustments[0].by_tax_group.len(), 2);
    assert_shares_tie_out(&r);
}

/// TAX-5 — the cart-level `CartLine.tax_key` wins over a key buried in the embedded query.
#[tokio::test]
async fn tax5_cart_level_key_wins_over_query_key() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    order_rule(&pool, company, 0, "0", "discount_percentage", Some(dec("10")), None, false, None).await;

    let mut l = line_tax(company, item, "100000", "1", Some("CART"));
    l.query.tax_key = Some("QUERY".into());
    let id = l.line_id;
    let r = svc.resolve_cart(&cart(company, vec![l])).await.unwrap();

    assert_eq!(share_of(&r, id), (Some("CART".into()), dec("10000.00")));
    assert_eq!(r.lines[0].tax_key.as_deref(), Some("CART"));
    assert_eq!(group_total(&r, Some("QUERY")), Decimal::ZERO, "query-buried key must not win");
    assert_shares_tie_out(&r);
}

/// TAX-6 — a group whose lines are already fully consumed by a prior adjustment receives nothing:
/// the follow-on order rule finds capacity only in the other group, exactly there.
#[tokio::test]
async fn tax6_exhausted_group_gets_no_share() {
    let pool = pool().await;
    let svc = PromoWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    // A 100%-off stackable bundle on A alone (exhausts A's capacity), plus a stackable fixed 5000
    // order rule that can only find capacity on B.
    let bid = bundle(&pool, company, 0, "all_of", None, "discount_percentage", Some(dec("100")), None, "0", true).await;
    bundle_component(&pool, company, bid, a, "1").await;
    order_rule(&pool, company, 0, "0", "discount_amount", None, Some(dec("5000")), true, None).await;

    let la = line_tax(company, a, "10000", "1", Some("PPN"));
    let lb = line_tax(company, b, "100000", "1", Some("FREE"));
    let id_a = la.line_id;
    let id_b = lb.line_id;
    let r = svc.resolve_cart(&cart(company, vec![la, lb])).await.unwrap();

    assert_eq!(r.order_discount_total, dec("15000.00")); // 10000 (bundle) + 5000 (rule)
    // The bundle took all of A; the order rule's 5000 could only land on B's group.
    assert_eq!(r.order_adjustments.len(), 2);
    let (bundle_adj, rule_adj) = (&r.order_adjustments[0], &r.order_adjustments[1]);
    assert_eq!(bundle_adj.discount_amount, dec("10000.00"));
    assert_eq!(bundle_adj.allocated.len(), 1);
    assert_eq!(bundle_adj.allocated[0].line_id, id_a);
    assert_eq!(bundle_adj.allocated[0].tax_key.as_deref(), Some("PPN"));
    assert_eq!(bundle_adj.allocated[0].share, dec("10000.00"));
    assert_eq!(rule_adj.discount_amount, dec("5000.00"));
    assert_eq!(rule_adj.allocated.len(), 1);
    assert_eq!(rule_adj.allocated[0].line_id, id_b);
    assert_eq!(rule_adj.allocated[0].tax_key.as_deref(), Some("FREE"));
    assert_eq!(rule_adj.allocated[0].share, dec("5000.00"));
    assert_eq!(rule_adj.by_tax_group.len(), 1);
    assert_eq!(rule_adj.by_tax_group[0].tax_key.as_deref(), Some("FREE"));
    assert_shares_tie_out(&r);
}

/// TAX-7 — serde back-compat: JSON from before the tax split (no `tax_key` anywhere) still
/// deserializes, and the new shapes round-trip.
#[test]
fn tax7_serde_back_compat() {
    // A pre-D4 PriceQuery (no tax_key field).
    let old_price_query = serde_json::json!({
        "company_id": "00000000-0000-0000-0000-000000000001",
        "list_price": "100000",
        "quantity": "1",
        "item_id": "00000000-0000-0000-0000-000000000002",
        "item_group_id": null,
        "brand_id": null,
        "customer_id": null,
        "customer_group_id": null,
        "coupon_code": null,
        "at": "2026-01-01T00:00:00Z"
    });
    let q: PriceQuery = serde_json::from_value(old_price_query).expect("old PriceQuery JSON parses");
    assert_eq!(q.tax_key, None);

    // A pre-D4 CartLine (no tax_key) around that query.
    let old_cart_line = serde_json::json!({
        "line_id": "00000000-0000-0000-0000-000000000003",
        "query": {
            "company_id": "00000000-0000-0000-0000-000000000001",
            "list_price": "100000",
            "quantity": "1",
            "item_id": "00000000-0000-0000-0000-000000000002",
            "item_group_id": null,
            "brand_id": null,
            "customer_id": null,
            "customer_group_id": null,
            "coupon_code": null,
            "at": "2026-01-01T00:00:00Z"
        }
    });
    let l: CartLine = serde_json::from_value(old_cart_line).expect("old CartLine JSON parses");
    assert_eq!(l.tax_key, None);

    // The new adjustment shape round-trips through JSON.
    let adj = backbone_promo::application::service::promo_ports::OrderAdjustment {
        source: AdjustmentSource::OrderRule("00000000-0000-0000-0000-000000000004".parse().unwrap()),
        discount_amount: dec("100.00"),
        allocated: vec![backbone_promo::application::service::promo_ports::AllocationShare {
            line_id: "00000000-0000-0000-0000-000000000003".parse().unwrap(),
            tax_key: Some("PPN".into()),
            share: dec("100.00"),
        }],
        by_tax_group: vec![backbone_promo::application::service::promo_ports::TaxGroupTotal {
            tax_key: Some("PPN".into()),
            discount_amount: dec("100.00"),
        }],
    };
    let round: backbone_promo::application::service::promo_ports::OrderAdjustment =
        serde_json::from_value(serde_json::to_value(&adj).unwrap()).unwrap();
    assert_eq!(round, adj);
}
