//! The cart-pricing seam at the counter: **backbone-pos → promo `resolve_cart`** (ADR-002).
//!
//! POS rings a ticket by calling promo through its own `CartPricingPort`; order-total discounts and
//! bundles come back as per-line nets and land on a REAL POS ticket whose net total equals the
//! conserved cart total. Proves the retail half of ADR-002's deferred cross-repo wiring. Zero normal
//! Cargo edge: POS depends on its `CartPricingPort`, not on promo (dev-dep only here).

mod common;

use backbone_promo::application::service::promo_ports::{CartLine, CartQuery, PriceQuery};
use backbone_promo::application::service::promo_write_service::PromoWriteService;
use backbone_pos::application::service::pos_cart_pricing::{
    CartPriceRequest, CartPricingError, CartPricingPort, PricedCart, PricedCartLine, PricedRewardLine,
};
use backbone_pos::application::service::pos_write_service::{
    CartSaleLine, NewCartSale, PosWriteService,
};
use common::*;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

/// ACL: POS's `CartPriceRequest` → promo `CartQuery` → `resolve_cart` → POS's `PricedCart`.
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
        let r = self
            .svc
            .resolve_cart(&q)
            .await
            .map_err(|e| CartPricingError { code: "pricing_failed".into(), message: e.to_string() })?;
        Ok(PricedCart {
            total: r.total,
            lines: r
                .lines
                .iter()
                .map(|l| PricedCartLine {
                    line_ref: l.line_id,
                    unit_price: l.unit_price,
                    net_line_total: l.net_line_total,
                })
                .collect(),
            reward_lines: r
                .reward_lines
                .iter()
                .map(|rl| PricedRewardLine { item_id: rl.item_id, quantity: rl.quantity })
                .collect(),
        })
    }
}

async fn open_session(pool: &sqlx::PgPool, company: Uuid, profile: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO pos.pos_opening_entries (company_id, pos_profile_id, cashier_party_id, opened_at, status)
           VALUES ($1,$2,$3, now(), 'open'::pos_session_status) RETURNING id"#,
    )
    .bind(company)
    .bind(profile)
    .bind(Uuid::new_v4())
    .fetch_one(pool)
    .await
    .unwrap()
}

/// CPSEAM-1 — a bundle + an order-total discount, priced by promo, land on a REAL POS ticket whose
/// net total equals the conserved cart total.
#[tokio::test]
async fn cpseam1_cart_discounts_land_on_a_real_ticket() {
    let pool = pool().await;
    let promo = Arc::new(PromoWriteService::new(pool.clone()));
    let pos = PosWriteService::new(pool.clone());
    let adapter = PromoCartAdapter { svc: promo.clone() };
    let company = Uuid::new_v4();
    let profile = Uuid::new_v4();
    let (item_a, item_b) = (Uuid::new_v4(), Uuid::new_v4());
    let session = open_session(&pool, company, profile).await;

    // buy A+B → 10% off the set; spend ≥ 250k → 5% off the order (stackable).
    let bid = bundle(&pool, company, 10, "all_of", None, "discount_percentage", Some(dec("10")), None, "0", true).await;
    bundle_component(&pool, company, bid, item_a, "1").await;
    bundle_component(&pool, company, bid, item_b, "1").await;
    order_rule(&pool, company, 0, "250000", "discount_percentage", Some(dec("5")), None, true, None).await;

    let ticket = NewCartSale {
        company_id: company,
        pos_profile_id: profile,
        opening_entry_id: session,
        branch_id: None,
        customer_id: None,
        customer_group_id: None,
        coupon_code: None,
        receipt_number: format!("R-{}", &Uuid::new_v4().to_string()[..8]),
        posting_at: now().naive_utc(),
        tax_total: Decimal::ZERO,
        round_to: None,
        lines: vec![
            CartSaleLine { item_id: item_a, item_group_id: None, brand_id: None, revenue_account_id: None, description: None, list_price: dec("200000"), quantity: dec("1") },
            CartSaleLine { item_id: item_b, item_group_id: None, brand_id: None, revenue_account_id: None, description: None, list_price: dec("100000"), quantity: dec("1") },
        ],
    };
    let ticket_id = pos.ring_sale_priced(ticket, &adapter).await.unwrap();

    // Subtotal 300k → bundle 10% (30k) → order 5% of 270k (13.5k) → net 256.5k.
    let net_total: Decimal = sqlx::query_scalar("SELECT net_total FROM pos.pos_invoices WHERE id=$1")
        .bind(ticket_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(net_total, dec("256500.00"), "the conserved cart total lands on the ticket net");
}

/// CPSEAM-2 — a buy-X-get-Y bundle's free item is rung on the REAL ticket as a zero-priced line.
#[tokio::test]
async fn cpseam2_free_item_lands_on_the_ticket() {
    let pool = pool().await;
    let promo = Arc::new(PromoWriteService::new(pool.clone()));
    let pos = PosWriteService::new(pool.clone());
    let adapter = PromoCartAdapter { svc: promo.clone() };
    let company = Uuid::new_v4();
    let profile = Uuid::new_v4();
    let (item_a, free_b) = (Uuid::new_v4(), Uuid::new_v4());
    let session = open_session(&pool, company, profile).await;

    let bid = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO promo.promo_bundles
             (company_id, title, priority, match_type, reward, reward_item_id, reward_qty,
              min_order_amount, stackable, valid_from, is_active)
           VALUES ($1,'free',0,'all_of'::bundle_match,'discount_percentage'::rate_or_discount,
                   $2,'1','0',false,now() - interval '1 day', true) RETURNING id"#,
    ).bind(company).bind(free_b).fetch_one(&pool).await.unwrap();
    bundle_component(&pool, company, bid, item_a, "1").await;

    let ticket = NewCartSale {
        company_id: company, pos_profile_id: profile, opening_entry_id: session, branch_id: None,
        customer_id: None, customer_group_id: None, coupon_code: None,
        receipt_number: format!("R-{}", &Uuid::new_v4().to_string()[..8]),
        posting_at: now().naive_utc(), tax_total: Decimal::ZERO, round_to: None,
        lines: vec![CartSaleLine { item_id: item_a, item_group_id: None, brand_id: None, revenue_account_id: None, description: None, list_price: dec("100000"), quantity: dec("1") }],
    };
    let ticket_id = pos.ring_sale_priced(ticket, &adapter).await.unwrap();

    let net_total: Decimal = sqlx::query_scalar("SELECT net_total FROM pos.pos_invoices WHERE id=$1")
        .bind(ticket_id).fetch_one(&pool).await.unwrap();
    assert_eq!(net_total, dec("100000.00"), "A charged in full; free B is zero-priced");
    let (n, free_qty): (i64, Decimal) = sqlx::query_as(
        "SELECT count(*), COALESCE(SUM(quantity) FILTER (WHERE item_id=$2 AND unit_price=0),0) FROM pos.pos_invoice_items WHERE pos_invoice_id=$1",
    ).bind(ticket_id).bind(free_b).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 2);
    assert_eq!(free_qty, dec("1.0000"));
}
