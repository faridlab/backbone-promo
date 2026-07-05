//! The marquee seams (proven across modules, zero normal Cargo edge):
//!
//!  * **PRSEAM-1 (price resolution → selling):** promo resolves a discounted unit price and the REAL
//!    backbone-selling write path prices a live Sales Order from it. Selling takes `unit_price` as a
//!    given — this proves promo is what decides that number. Load-bearing: an item with no rule falls
//!    through to list, so the discounted order's subtotal is lower by exactly the promo discount.
//!  * **PRSEAM-2 (loyalty ← POS):** promo accrues points off backbone-pos's REAL `PosInvoicePaid`
//!    event (already emitted by the retail seam). Idempotent per source: replaying the paid event
//!    never double-earns.
//!
//! Both edges are dev-dependencies ONLY — the shipped promo library depends on neither selling nor POS.

mod common;

use backbone_promo::application::service::promo_events::LoggingSink;
use backbone_promo::application::service::promo_ports::{
    AccrualRequest, PriceQuery, PriceResolverPort,
};
use common::RuleSpec;
use backbone_promo::application::service::promo_write_service::{
    PromoPriceResolver, PromoWriteService,
};
use backbone_selling::application::service::selling_write_service::{
    NewLine, NewSalesOrder, SellingWriteService,
};
use backbone_pos::application::service::pos_events::PosInvoicePaid;
use common::*;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

async fn order_subtotal(pool: &sqlx::PgPool, id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT subtotal FROM selling.sales_orders WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// PRSEAM-1 — promo's resolved price is what selling actually charges.
#[tokio::test]
async fn prseam1_resolved_price_drives_selling_order() {
    let pool = pool().await;
    let promo = Arc::new(PromoWriteService::new(pool.clone()));
    let selling = SellingWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let discounted_item = Uuid::new_v4();
    let plain_item = Uuid::new_v4();

    // A 20%-off rule on ONE item.
    pct_rule(&pool, company, discounted_item, 0, "20").await;

    // The caller depends only on the PORT (zero normal edge).
    let resolver: Arc<dyn PriceResolverPort> = Arc::new(PromoPriceResolver { service: promo.clone() });

    let query = |item: Uuid| PriceQuery {
        company_id: company,
        list_price: dec("100000"),
        quantity: dec("2"),
        item_id: item,
        item_group_id: None,
        brand_id: None,
        customer_id: Some(customer),
        customer_group_id: None,
        coupon_code: None,
        at: now(),
    };

    // Resolve both lines through the port.
    let disc = resolver.resolve(&query(discounted_item)).await.unwrap();
    let plain = resolver.resolve(&query(plain_item)).await.unwrap();
    assert_eq!(disc.unit_price, dec("80000.00")); // 20% off
    assert_eq!(plain.unit_price, dec("100000")); // no rule → list

    // Build a REAL selling order priced from the resolved unit prices.
    let mk = |num: &str, item: Uuid, price: Decimal| NewSalesOrder {
        order_number: num.to_string(),
        quotation_id: None,
        company_id: company,
        branch_id: None,
        customer_id: customer,
        order_date: now().date_naive(),
        delivery_date: None,
        currency: Some("IDR".into()),
        tax_rate: Decimal::ZERO,
        notes: None,
        lines: vec![NewLine {
            item_id: item,
            revenue_account_id: None,
            description: None,
            quantity: dec("2"),
            unit_price: price,
            line_discount: Decimal::ZERO,
        }],
    };

    let disc_order = selling
        .create_sales_order(mk(&format!("SO-D-{}", &company.to_string()[..8]), discounted_item, disc.unit_price))
        .await
        .unwrap();
    let plain_order = selling
        .create_sales_order(mk(&format!("SO-P-{}", &company.to_string()[..8]), plain_item, plain.unit_price))
        .await
        .unwrap();

    // The promo discount is exactly the difference selling booked.
    assert_eq!(order_subtotal(&pool, disc_order).await, dec("160000.00")); // 2 × 80,000
    assert_eq!(order_subtotal(&pool, plain_order).await, dec("200000.00")); // 2 × 100,000
}

/// PRSEAM-3 — the coupon cap binds END-TO-END, not just in an isolated probe. `resolve` offers the
/// discount and hands back `applied_coupon_id`; the sale-commit consumes the use keyed by the real
/// selling order; once the cap is reached `resolve` REFUSES the coupon and the next customer pays list.
/// This is the wired burn a caller performs — the coupon analog of PRSEAM-2's loyalty accrual seam
/// (council 2026-07-06 completeness fix). Proven-by-revert: skip the commit and B still gets the
/// discount (the cap is inert), i.e. the commit is what makes "never past max_use" true in the seam.
#[tokio::test]
async fn prseam3_coupon_cap_binds_across_selling_commit() {
    let pool = pool().await;
    let promo = PromoWriteService::new(pool.clone());
    let selling = SellingWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();

    // A single-use, coupon-gated 30%-off rule.
    let rule_id = rule(&pool, RuleSpec {
        coupon_required: true,
        discount_percentage: Some(dec("30")),
        ..RuleSpec::for_item(company, item)
    })
    .await;
    let coupon_id = coupon(&pool, company, "FLASH", rule_id, Some(1)).await;

    let query = |cust: Uuid| PriceQuery {
        company_id: company,
        list_price: dec("100000"),
        quantity: Decimal::ONE,
        item_id: item,
        item_group_id: None,
        brand_id: None,
        customer_id: Some(cust),
        customer_group_id: None,
        coupon_code: Some("FLASH".into()),
        at: now(),
    };
    let mk_order = |num: &str, cust: Uuid, price: Decimal| NewSalesOrder {
        order_number: num.to_string(),
        quotation_id: None,
        company_id: company,
        branch_id: None,
        customer_id: cust,
        order_date: now().date_naive(),
        delivery_date: None,
        currency: Some("IDR".into()),
        tax_rate: Decimal::ZERO,
        notes: None,
        lines: vec![NewLine {
            item_id: item,
            revenue_account_id: None,
            description: None,
            quantity: Decimal::ONE,
            unit_price: price,
            line_discount: Decimal::ZERO,
        }],
    };
    let short = &company.to_string()[..8];

    // Customer A: resolve offers the discount + the coupon handoff token.
    let a = Uuid::new_v4();
    let r_a = promo.resolve(&query(a)).await.unwrap();
    assert_eq!(r_a.unit_price, dec("70000.00"));
    let cid = r_a.applied_coupon_id.expect("coupon offered");
    assert_eq!(cid, coupon_id);

    // Resolving again is still just an OFFER — the cap is intact until the sale commits.
    assert!(promo.resolve(&query(a)).await.unwrap().applied_coupon_id.is_some());

    // Build A's real selling order at the resolved price, then commit the coupon at order-confirm,
    // keyed by the order id — the wired burn.
    let order_a = selling
        .create_sales_order(mk_order(&format!("SO-A-{short}"), a, r_a.unit_price))
        .await
        .unwrap();
    let consumed_rule = promo
        .commit_coupon_redemption(company, cid, "sales_order", order_a, &sink)
        .await
        .unwrap();
    assert_eq!(consumed_rule, rule_id);

    // Customer B presents the SAME code — the cap is reached, so resolve REFUSES it: no discount.
    let b = Uuid::new_v4();
    let r_b = promo.resolve(&query(b)).await.unwrap();
    assert_eq!(r_b.applied_coupon_id, None, "an exhausted coupon is refused at resolve");
    assert_eq!(r_b.unit_price, dec("100000"), "B pays list — the cap protected the margin");
}

/// PRSEAM-2 — promo accrues loyalty points off POS's real paid event, idempotently.
#[tokio::test]
async fn prseam2_loyalty_accrues_from_pos_invoice_paid() {
    let pool = pool().await;
    let promo = PromoWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let program_id = program(&pool, company, "0.01", "100", Some(365)).await; // 1 pt / 100 spent

    // The REAL event POS emits when a counter sale is recognised.
    let paid = PosInvoicePaid {
        pos_invoice_id: Uuid::new_v4(),
        company_id: company,
        grand_total: dec("250000"),
        rounded_total: dec("250000"),
        billing_invoice_id: Uuid::new_v4(),
        payment_id: Uuid::new_v4(),
    };

    // A POS→promo adapter maps the paid event to an accrual request.
    let to_accrual = |e: &PosInvoicePaid| AccrualRequest {
        company_id: e.company_id,
        loyalty_program_id: program_id,
        customer_id: customer,
        purchase_amount: e.rounded_total,
        source_type: "pos_invoice".into(),
        source_id: e.pos_invoice_id,
        at: now(),
    };

    let a = promo.accrue(&to_accrual(&paid), &sink).await.unwrap();
    assert_eq!(a.points, dec("2500"));
    assert!(!a.already);

    // Redelivery of the same paid event earns nothing more.
    let b = promo.accrue(&to_accrual(&paid), &sink).await.unwrap();
    assert!(b.already);
    assert_eq!(balance(&pool, company, customer, program_id).await, dec("2500"));
}
