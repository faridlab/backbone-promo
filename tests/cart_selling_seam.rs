//! The cart-pricing seam, end-to-end: **backbone-selling → promo `resolve_cart`** (ADR-002).
//!
//! Selling prices a whole basket by calling promo through its own `CartPricingPort`; order-total
//! discounts and bundles (which span lines) come back as per-line nets, and selling persists them on a
//! REAL Sales Order. This proves the deferred cross-repo half of ADR-002 — a live order actually
//! consumes `resolve_cart` — and that the conserved cart total lands on the order subtotal exactly.
//! Zero normal Cargo edge: selling depends on its `CartPricingPort`, not on promo (dev-dep only here).

mod common;

use backbone_promo::application::service::promo_ports::{CartLine, CartQuery, PriceQuery};
use backbone_promo::application::service::promo_write_service::PromoWriteService;
use backbone_selling::application::service::selling_cart_pricing::{
    CartPriceRequest, CartPricingError, CartPricingPort, PricedCart, PricedCartLine, PricedRewardLine,
};
use backbone_selling::application::service::selling_write_service::{
    CartOrderLine, NewCartSalesOrder, SellingWriteService,
};
use common::*;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

/// ACL: selling's `CartPriceRequest` → promo `CartQuery` → `resolve_cart` → selling's `PricedCart`.
struct PromoCartAdapter {
    svc: Arc<PromoWriteService>,
}
#[async_trait::async_trait]
impl CartPricingPort for PromoCartAdapter {
    async fn price_cart(&self, req: &CartPriceRequest) -> Result<PricedCart, CartPricingError> {
        let q = CartQuery {
            company_id: req.company_id,
            customer_id: req.customer_id,
            customer_group_id: req.customer_group_id,
            coupon_code: req.coupon_code.clone(),
            at: now(),
            lines: req
                .lines
                .iter()
                .map(|l| CartLine {
                    line_id: l.line_ref,
                    query: PriceQuery {
                        company_id: req.company_id,
                        list_price: l.list_price,
                        quantity: l.quantity,
                        item_id: l.item_id,
                        item_group_id: l.item_group_id,
                        brand_id: l.brand_id,
                        customer_id: req.customer_id,
                        customer_group_id: req.customer_group_id,
                        coupon_code: req.coupon_code.clone(),
                        at: now(),
                    },
                })
                .collect(),
        };
        let resolved = self
            .svc
            .resolve_cart(&q)
            .await
            .map_err(|e| CartPricingError { code: "pricing_failed".into(), message: e.to_string() })?;
        Ok(PricedCart {
            total: resolved.total,
            lines: resolved
                .lines
                .iter()
                .map(|l| PricedCartLine {
                    line_ref: l.line_id,
                    unit_price: l.unit_price,
                    net_line_total: l.net_line_total,
                })
                .collect(),
            reward_lines: resolved
                .reward_lines
                .iter()
                .map(|r| PricedRewardLine { item_id: r.item_id, quantity: r.quantity })
                .collect(),
        })
    }
}

async fn order_totals(pool: &sqlx::PgPool, id: Uuid) -> (Decimal, Decimal) {
    sqlx::query_as("SELECT subtotal, total FROM selling.sales_orders WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// CSSEAM-1 — an order-total discount + a bundle, priced by promo, land on a REAL Sales Order whose
/// subtotal equals the conserved cart total.
#[tokio::test]
async fn csseam1_cart_discounts_land_on_a_real_sales_order() {
    let pool = pool().await;
    let promo = Arc::new(PromoWriteService::new(pool.clone()));
    let selling = SellingWriteService::new(pool.clone());
    let adapter = PromoCartAdapter { svc: promo.clone() };
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let (item_a, item_b) = (Uuid::new_v4(), Uuid::new_v4());

    // Promo: buy A+B → 10% off the matched set, and spend ≥ 250k → 5% off the order (stackable).
    let bid = bundle(&pool, company, 10, "all_of", None, "discount_percentage", Some(dec("10")), None, "0", true).await;
    bundle_component(&pool, company, bid, item_a, "1").await;
    bundle_component(&pool, company, bid, item_b, "1").await;
    order_rule(&pool, company, 0, "250000", "discount_percentage", Some(dec("5")), None, true, None).await;

    // Selling prices the basket through promo and persists a real order.
    let order = NewCartSalesOrder {
        order_number: format!("SO-CART-{}", &Uuid::new_v4().to_string()[..8]),
        company_id: company,
        branch_id: None,
        customer_id: customer,
        customer_group_id: None,
        coupon_code: None,
        order_date: now().date_naive(),
        delivery_date: None,
        currency: Some("IDR".into()),
        tax_rate: Decimal::ZERO,
        notes: None,
        lines: vec![
            CartOrderLine { item_id: item_a, item_group_id: None, brand_id: None, revenue_account_id: None, description: None, list_price: dec("200000"), quantity: dec("1") },
            CartOrderLine { item_id: item_b, item_group_id: None, brand_id: None, revenue_account_id: None, description: None, list_price: dec("100000"), quantity: dec("1") },
        ],
    };
    let order_id = selling.create_sales_order_priced(order, &adapter).await.unwrap();

    // Subtotal 300k → bundle 10% of 300k = 30k, then order 5% of remaining 270k = 13.5k → total 256.5k.
    let (subtotal, total) = order_totals(&pool, order_id).await;
    assert_eq!(subtotal, dec("256500.00"), "the conserved cart total lands on the order subtotal");
    assert_eq!(total, dec("256500.00"));
}

/// CSSEAM-2 — with no promo configured, the cart pricer passes list prices straight through.
#[tokio::test]
async fn csseam2_no_promo_is_passthrough() {
    let pool = pool().await;
    let promo = Arc::new(PromoWriteService::new(pool.clone()));
    let selling = SellingWriteService::new(pool.clone());
    let adapter = PromoCartAdapter { svc: promo.clone() };
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();

    let order = NewCartSalesOrder {
        order_number: format!("SO-CART-{}", &Uuid::new_v4().to_string()[..8]),
        company_id: company,
        branch_id: None,
        customer_id: customer,
        customer_group_id: None,
        coupon_code: None,
        order_date: now().date_naive(),
        delivery_date: None,
        currency: Some("IDR".into()),
        tax_rate: Decimal::ZERO,
        notes: None,
        lines: vec![CartOrderLine {
            item_id: Uuid::new_v4(),
            item_group_id: None,
            brand_id: None,
            revenue_account_id: None,
            description: None,
            list_price: dec("100000"),
            quantity: dec("2"),
        }],
    };
    let order_id = selling.create_sales_order_priced(order, &adapter).await.unwrap();
    let (subtotal, _) = order_totals(&pool, order_id).await;
    assert_eq!(subtotal, dec("200000.00")); // 2 × 100,000, no discount
}

/// CSSEAM-3 — a buy-X-get-Y bundle's free item lands on the REAL Sales Order as a zero-priced line,
/// without changing the subtotal.
#[tokio::test]
async fn csseam3_free_item_lands_on_the_order() {
    let pool = pool().await;
    let promo = Arc::new(PromoWriteService::new(pool.clone()));
    let selling = SellingWriteService::new(pool.clone());
    let adapter = PromoCartAdapter { svc: promo.clone() };
    let company = Uuid::new_v4();
    let (item_a, free_b) = (Uuid::new_v4(), Uuid::new_v4());

    // buy A → get 1 free B.
    let bid = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO promo.promo_bundles
             (company_id, title, priority, match_type, reward, reward_item_id, reward_qty,
              min_order_amount, stackable, valid_from, is_active)
           VALUES ($1,'free',0,'all_of'::bundle_match,'discount_percentage'::rate_or_discount,
                   $2,'1','0',false,now() - interval '1 day', true) RETURNING id"#,
    ).bind(company).bind(free_b).fetch_one(&pool).await.unwrap();
    bundle_component(&pool, company, bid, item_a, "1").await;

    let order = NewCartSalesOrder {
        order_number: format!("SO-FREE-{}", &Uuid::new_v4().to_string()[..8]),
        company_id: company, branch_id: None, customer_id: Uuid::new_v4(), customer_group_id: None,
        coupon_code: None, order_date: now().date_naive(), delivery_date: None,
        currency: Some("IDR".into()), tax_rate: Decimal::ZERO, notes: None,
        lines: vec![CartOrderLine { item_id: item_a, item_group_id: None, brand_id: None, revenue_account_id: None, description: None, list_price: dec("100000"), quantity: dec("1") }],
    };
    let order_id = selling.create_sales_order_priced(order, &adapter).await.unwrap();

    let (subtotal, _) = order_totals(&pool, order_id).await;
    assert_eq!(subtotal, dec("100000.00"), "A charged in full; the free B is zero-priced");
    // Two order lines: the paid A and the free B at unit_price 0.
    let (n_lines, free_qty): (i64, Decimal) = sqlx::query_as(
        "SELECT count(*), COALESCE(SUM(quantity) FILTER (WHERE item_id=$2 AND unit_price=0),0) FROM selling.sales_order_items WHERE order_id=$1",
    ).bind(order_id).bind(free_b).fetch_one(&pool).await.unwrap();
    assert_eq!(n_lines, 2);
    assert_eq!(free_qty, dec("1.0000"), "the free B line was added at qty 1, price 0");
}
