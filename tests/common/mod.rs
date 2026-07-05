//! Shared test helpers: a live pool + seeders for the promo tables. Every test uses a fresh random
//! `company_id` so rows never collide across tests running in parallel against the same database.

#![allow(dead_code)]

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

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
pub async fn pct_rule(
    pool: &PgPool,
    company: Uuid,
    item: Uuid,
    priority: i32,
    pct: &str,
) -> Uuid {
    rule(pool, RuleSpec {
        company,
        apply_on: "item",
        item: Some(item),
        priority,
        rate_or_discount: "discount_percentage",
        discount_percentage: Some(dec(pct)),
        ..RuleSpec::for_item(company, item)
    })
    .await
}

/// Full control over one pricing rule.
pub struct RuleSpec {
    pub company: Uuid,
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
    pub is_active: bool,
}

impl RuleSpec {
    pub fn for_item(company: Uuid, item: Uuid) -> Self {
        Self {
            company,
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
            is_active: true,
        }
    }
}

pub async fn rule(pool: &PgPool, s: RuleSpec) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO promo.pricing_rules
            (company_id, title, priority, apply_on, item_id, item_group_id, brand_id,
             customer_id, customer_group_id, min_qty, max_qty, min_amount,
             rate_or_discount, rate, discount_percentage, discount_amount,
             coupon_required, valid_from, valid_to, is_active)
        VALUES ($1,'test',$2,$3::apply_on,$4,$5,$6,$7,$8,$9,$10,$11,
                $12::rate_or_discount,$13,$14,$15,$16,$17,$18,$19)
        RETURNING id
        "#,
    )
    .bind(s.company).bind(s.priority).bind(s.apply_on).bind(s.item).bind(s.item_group).bind(s.brand)
    .bind(s.customer).bind(s.customer_group).bind(s.min_qty).bind(s.max_qty).bind(s.min_amount)
    .bind(s.rate_or_discount).bind(s.rate).bind(s.discount_percentage).bind(s.discount_amount)
    .bind(s.coupon_required).bind(s.valid_from).bind(s.valid_to).bind(s.is_active)
    .fetch_one(pool)
    .await
    .expect("insert rule")
}

/// A coupon unlocking a rule, with a redemption cap.
pub async fn coupon(
    pool: &PgPool,
    company: Uuid,
    code: &str,
    rule_id: Uuid,
    max_use: Option<i32>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO promo.coupon_codes
             (company_id, code, pricing_rule_id, max_use, valid_from, is_active)
           VALUES ($1,$2,$3,$4,$5,true) RETURNING id"#,
    )
    .bind(company)
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
    company: Uuid,
    collection_factor: &str,
    conversion_factor: &str,
    expiry_days: Option<i32>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO promo.loyalty_programs
             (company_id, program_name, program_type, collection_factor, conversion_factor,
              expiry_duration_days, from_date, is_active)
           VALUES ($1,'test','single_tier'::loyalty_program_type,$2,$3,$4,$5,true) RETURNING id"#,
    )
    .bind(company)
    .bind(dec(collection_factor))
    .bind(dec(conversion_factor))
    .bind(expiry_days)
    .bind(now() - chrono::Duration::days(1))
    .fetch_one(pool)
    .await
    .expect("insert program")
}

/// The member's current signed points balance.
pub async fn balance(pool: &PgPool, company: Uuid, customer: Uuid, program_id: Uuid) -> Decimal {
    sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(points),0) FROM promo.loyalty_point_entries
           WHERE company_id=$1 AND customer_id=$2 AND loyalty_program_id=$3
             AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company)
    .bind(customer)
    .bind(program_id)
    .fetch_one(pool)
    .await
    .expect("balance")
}
