//! Shared test helpers: a live pool + seeders for the promo tables. Every seed keys on freshly
//! generated ids (item / customer / program / coupon), so rows never collide across tests running
//! in parallel against the same database. The module is tenant-agnostic (ADR-0029): no seed names
//! a tenant column — row scoping is the composing service's tenancy decorator, outside the
//! module's concern.

#![allow(dead_code)]

use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::sync::MutexGuard;
use uuid::Uuid;

/// Serializes the suites whose assertions read the wide-scan tables (rule/bundle candidates
/// match any cart; a coupon lookup keys on the bare code — with the company axis gone the
/// undecorated test DB has no row fence, ADR-0029). Holding the returned guard for the whole
/// test makes sweep + seed + assert one critical section; the sweep clears sibling tests'
/// rows (current run and stale rows from earlier suites — test binaries run sequentially, so
/// only this binary's own tests contend on the lock).
pub static WIDE_TABLE_SWEEP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Acquire the sweep lock and clear every wide-scan table. Call as the FIRST line of a test
/// (right after the pool bind) and bind the guard to `_` for the test's body.
pub async fn isolated_wide_tables(pool: &PgPool) -> MutexGuard<'static, ()> {
    let guard = WIDE_TABLE_SWEEP.lock().await;
    for table in [
        "promo.coupon_claims",
        "promo.coupon_redemptions",
        "promo.coupon_codes",
        "promo.promo_bundle_gifts",
        "promo.promo_bundle_components",
        "promo.promo_bundles",
        "promo.pricing_rules",
    ] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(pool)
            .await
            .expect("sweep wide-scan tables");
    }
    guard
}

pub fn dburl() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_promo".into())
}

pub async fn pool() -> PgPool {
    PgPool::connect(&dburl()).await.expect("connect")
}

pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

pub fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// A pricing rule with a percentage discount on one item.
pub async fn pct_rule(pool: &PgPool, item: Uuid, priority: i32, pct: &str) -> Uuid {
    rule(pool, RuleSpec {
        apply_on: "item",
        item: Some(item),
        priority,
        rate_or_discount: "discount_percentage",
        discount_percentage: Some(dec(pct)),
        ..RuleSpec::for_item(item)
    })
    .await
}

/// Full control over one pricing rule.
pub struct RuleSpec {
    pub apply_on: &'static str,
    pub item: Option<Uuid>,
    pub item_group: Option<Uuid>,
    pub brand: Option<Uuid>,
    pub customer: Option<Uuid>,
    pub customer_group: Option<Uuid>,
    pub priority: i32,
    pub min_qty: Decimal,
    pub max_qty: Option<Decimal>,
    pub min_amount: Decimal,
    pub rate_or_discount: &'static str,
    pub rate: Option<Decimal>,
    pub discount_percentage: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub coupon_required: bool,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub status: &'static str,
}

impl RuleSpec {
    pub fn for_item(item: Uuid) -> Self {
        Self {
            apply_on: "item",
            item: Some(item),
            item_group: None,
            brand: None,
            customer: None,
            customer_group: None,
            priority: 0,
            min_qty: Decimal::ZERO,
            max_qty: None,
            min_amount: Decimal::ZERO,
            rate_or_discount: "discount_percentage",
            rate: None,
            discount_percentage: None,
            discount_amount: None,
            coupon_required: false,
            valid_from: now() - chrono::Duration::days(1),
            valid_to: None,
            status: "active",
        }
    }
}

pub async fn rule(pool: &PgPool, s: RuleSpec) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO promo.pricing_rules
            (title, priority, apply_on, item_id, item_group_id, brand_id,
             customer_id, customer_group_id, min_qty, max_qty, min_amount,
             rate_or_discount, rate, discount_percentage, discount_amount,
             coupon_required, valid_from, valid_to, status)
        VALUES ('test',$1,$2::apply_on,$3,$4,$5,$6,$7,$8,$9,$10,
                $11::rate_or_discount,$12,$13,$14,$15,$16,$17,$18::pricing_rule_status)
        RETURNING id
        "#,
    )
    .bind(s.priority).bind(s.apply_on).bind(s.item).bind(s.item_group).bind(s.brand)
    .bind(s.customer).bind(s.customer_group).bind(s.min_qty).bind(s.max_qty).bind(s.min_amount)
    .bind(s.rate_or_discount).bind(s.rate).bind(s.discount_percentage).bind(s.discount_amount)
    .bind(s.coupon_required).bind(s.valid_from).bind(s.valid_to).bind(s.status)
    .fetch_one(pool)
    .await
    .expect("insert rule")
}

/// An order-scoped rule: fires once against the cart subtotal (≥ `min_order_amount`).
/// `rate_or_discount` is "discount_percentage" or "discount_amount".
#[allow(clippy::too_many_arguments)]
pub async fn order_rule(
    pool: &PgPool,
    priority: i32,
    min_order_amount: &str,
    rate_or_discount: &str,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    stackable: bool,
    customer_group: Option<Uuid>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO promo.pricing_rules
            (title, priority, scope, min_order_amount, stackable, apply_on,
             customer_group_id, rate_or_discount, discount_percentage, discount_amount,
             coupon_required, valid_from, status)
        VALUES ('test-order',$1,'order'::rule_scope,$2,$3,'all'::apply_on,$4,
                $5::rate_or_discount,$6,$7,false,$8,'active')
        RETURNING id
        "#,
    )
    .bind(priority)
    .bind(dec(min_order_amount))
    .bind(stackable)
    .bind(customer_group)
    .bind(rate_or_discount)
    .bind(discount_percentage)
    .bind(discount_amount)
    .bind(now() - chrono::Duration::days(1))
    .fetch_one(pool)
    .await
    .expect("insert order rule")
}

/// An order-scoped rule carrying the threshold-shape fields (ADR-003): a cart-wide item-count floor
/// (`min_order_qty`) and/or a discount Rp ceiling (`discount_upto`). Either may be `None`.
#[allow(clippy::too_many_arguments)]
pub async fn order_rule_threshold(
    pool: &PgPool,
    priority: i32,
    min_order_amount: &str,
    min_order_qty: Option<Decimal>,
    rate_or_discount: &str,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    discount_upto: Option<Decimal>,
    stackable: bool,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO promo.pricing_rules
            (title, priority, scope, min_order_amount, min_order_qty, stackable,
             apply_on, rate_or_discount, discount_percentage, discount_amount, discount_upto,
             coupon_required, valid_from, status)
        VALUES ('test-order-threshold',$1,'order'::rule_scope,$2,$3,$4,'all'::apply_on,
                $5::rate_or_discount,$6,$7,$8,false,$9,'active')
        RETURNING id
        "#,
    )
    .bind(priority)
    .bind(dec(min_order_amount))
    .bind(min_order_qty)
    .bind(stackable)
    .bind(rate_or_discount)
    .bind(discount_percentage)
    .bind(discount_amount)
    .bind(discount_upto)
    .bind(now() - chrono::Duration::days(1))
    .fetch_one(pool)
    .await
    .expect("insert threshold order rule")
}

/// A bundle with a reward effect. Add components with `bundle_component`.
#[allow(clippy::too_many_arguments)]
pub async fn bundle(
    pool: &PgPool,
    priority: i32,
    match_type: &str,
    required_distinct: Option<i32>,
    reward: &str,
    discount_percentage: Option<Decimal>,
    discount_amount: Option<Decimal>,
    min_order_amount: &str,
    stackable: bool,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO promo.promo_bundles
            (title, priority, match_type, required_distinct, reward,
             discount_percentage, discount_amount, min_order_amount, stackable,
             valid_from, status)
        VALUES ('test-bundle',$1,$2::bundle_match,$3,$4::rate_or_discount,
                $5,$6,$7,$8,$9,'active')
        RETURNING id
        "#,
    )
    .bind(priority)
    .bind(match_type)
    .bind(required_distinct)
    .bind(reward)
    .bind(discount_percentage)
    .bind(discount_amount)
    .bind(dec(min_order_amount))
    .bind(stackable)
    .bind(now() - chrono::Duration::days(1))
    .fetch_one(pool)
    .await
    .expect("insert bundle")
}

/// One item-selector component of a bundle (apply_on=item), needing `min_qty` per set.
pub async fn bundle_component(
    pool: &PgPool,
    bundle_id: Uuid,
    item: Uuid,
    min_qty: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO promo.promo_bundle_components
            (bundle_id, apply_on, item_id, min_qty)
        VALUES ($1,'item'::apply_on,$2,$3)
        "#,
    )
    .bind(bundle_id)
    .bind(item)
    .bind(dec(min_qty))
    .execute(pool)
    .await
    .expect("insert bundle component");
}

/// Attach a free gift to a bundle: `gift_qty` units of `gift_item` per satisfied set (ADR-005).
pub async fn gift(
    pool: &PgPool,
    bundle_id: Uuid,
    gift_item: Uuid,
    gift_qty: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO promo.promo_bundle_gifts
            (bundle_id, gift_item_id, gift_qty)
        VALUES ($1,$2,$3)
        "#,
    )
    .bind(bundle_id)
    .bind(gift_item)
    .bind(dec(gift_qty))
    .execute(pool)
    .await
    .expect("insert gift");
}

/// A coupon unlocking a rule, with a redemption cap.
pub async fn coupon(
    pool: &PgPool,
    code: &str,
    rule_id: Uuid,
    max_use: Option<i32>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO promo.coupon_codes
             (code, pricing_rule_id, max_use, valid_from, status)
           VALUES ($1,$2,$3,$4,'active') RETURNING id"#,
    )
    .bind(code.to_uppercase())
    .bind(rule_id)
    .bind(max_use)
    .bind(now() - chrono::Duration::days(1))
    .fetch_one(pool)
    .await
    .expect("insert coupon")
}

/// A loyalty program: earn `collection_factor` pts / currency, burn `conversion_factor` currency / pt.
pub async fn program(
    pool: &PgPool,
    collection_factor: &str,
    conversion_factor: &str,
    expiry_days: Option<i32>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO promo.loyalty_programs
             (program_name, program_type, collection_factor, conversion_factor,
              expiry_duration_days, from_date, status)
           VALUES ('test','single_tier'::loyalty_program_type,$1,$2,$3,$4,'active') RETURNING id"#,
    )
    .bind(dec(collection_factor))
    .bind(dec(conversion_factor))
    .bind(expiry_days)
    .bind(now() - chrono::Duration::days(1))
    .fetch_one(pool)
    .await
    .expect("insert program")
}

/// The member's current signed points balance.
pub async fn balance(pool: &PgPool, customer: Uuid, program_id: Uuid) -> Decimal {
    sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(points),0) FROM promo.loyalty_point_entries
           WHERE customer_id=$1 AND loyalty_program_id=$2
             AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(customer)
    .bind(program_id)
    .fetch_one(pool)
    .await
    .expect("balance")
}
