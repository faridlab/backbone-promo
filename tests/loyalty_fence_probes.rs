//! The tenancy posture probe (ADR-0029) — the module ships NO tenancy of its own: no tenant
//! column, no tenant predicate, and no RLS policy. What it ships instead is the HALF-FENCE the
//! composing service's tenancy decorator completes: every promo base table carries ENABLE + FORCE
//! ROW LEVEL SECURITY with zero policies. This probe pins that posture from below, the family
//! pattern (proven on backbone-accounting, backbone-billing, backbone-selling, then backbone-pos).
//!
//! - the flags are armed on every base table and the policy set is empty (schema pin);
//! - a plain NOSUPERUSER NOBYPASSRLS role is default-DENIED — zero rows, writes refused — no
//!   matter what legacy variable is set (no policy reads `app.company_id` anymore; the
//!   decorator's org-scoped policies will, once composed);
//! - the scratch owner is a superuser and BYPASSES row-level security, so it still sees its own
//!   seeded rows plainly: the denial is the missing policy, not an empty database.
//!
//! The pre-rewrite fence suite's cross-company isolation legs retire here: they pinned the
//! module's own company policy, whose claim has moved to the composing service's tenancy
//! decorator (ADR-0029).
//!
//! Requires DATABASE_URL (:5433/backbone_promo) reachable as a superuser (to mint and tear down
//! the probe role).

mod common;

use common::{dburl, pool};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const ROLE: &str = "promo_tenancy_probe";
const PWD: &str = "probe";

/// Role/catalog DDL serializes — two tests minting roles concurrently hit
/// "tuple concurrently updated" in the system catalogs.
static ROLE_DDL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Shed the role's grants, then drop it. Leftover grants (from a run whose teardown never
/// reached the drop, or whose drop was swallowed) make plain DROP ROLE fail with 2BP01 —
/// DROP OWNED BY first keeps both bootstrap and teardown idempotent across runs.
async fn drop_role(admin: &PgPool) {
    let _ = sqlx::query(&format!("DROP OWNED BY {ROLE}"))
        .execute(admin)
        .await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {ROLE}"))
        .execute(admin)
        .await;
}

async fn bootstrap_role(admin: &PgPool, table: &str) {
    drop_role(admin).await;
    let db = dburl()
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://")
        .split_once('/')
        .and_then(|(_, path)| path.split('?').next())
        .unwrap_or("backbone_promo")
        .to_string();
    for stmt in [
        format!("CREATE ROLE {ROLE} LOGIN PASSWORD '{PWD}' NOSUPERUSER NOBYPASSRLS"),
        format!(r#"GRANT CONNECT ON DATABASE "{db}" TO {ROLE}"#),
        format!("GRANT USAGE ON SCHEMA promo TO {ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE promo.{table} TO {ROLE}"),
    ] {
        sqlx::query(&stmt).execute(admin).await.unwrap();
    }
}

/// A pool connected as the plain probe role, aimed at the same host/port/database as the admin
/// URL (the role was minted by [`bootstrap_role`] on the admin pool).
async fn restricted_pool() -> PgPool {
    let url = dburl();
    let rest = url
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://");
    let (authority, path) = rest.split_once('/').expect("DATABASE_URL must name a database");
    let hostport = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let db = path.split('?').next().unwrap_or("backbone_promo");
    PgPool::connect(&format!("postgresql://{ROLE}:{PWD}@{hostport}/{db}"))
        .await
        .expect("connect probe role")
}

// ── The schema pin: armed flags, empty policy set ─────────────────────────────

/// The promo base tables the strip migration freed of their company axis — every one must stay
/// behind the armed half-fence.
const BASE_TABLES: [&str; 11] = [
    "coupon_claims",
    "coupon_codes",
    "coupon_redemptions",
    "loyalty_member_anchors",
    "loyalty_order_points",
    "loyalty_point_entries",
    "loyalty_programs",
    "pricing_rules",
    "promo_bundle_components",
    "promo_bundle_gifts",
    "promo_bundles",
];

/// Every promo base table carries ENABLE + FORCE ROW LEVEL SECURITY and the module ships ZERO
/// policies — the decorator's half-fence. If a strip or regen ever drops the flags, an
/// undecorated deployment would silently become readable by any role the host grants; if a
/// policy ever reappears module-side, the decorator's org-scoped policies would fight it.
#[tokio::test]
async fn tables_carry_rls_flags_and_the_module_ships_no_policy() {
    let owner = pool().await;

    let armed: Vec<String> = sqlx::query(
        "SELECT c.relname FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'promo' AND c.relkind = 'r' \
           AND c.relrowsecurity AND c.relforcerowsecurity \
         ORDER BY c.relname",
    )
    .fetch_all(&owner)
    .await
    .unwrap()
    .iter()
    .map(|r| r.get::<String, _>("relname"))
    .collect();
    for table in BASE_TABLES {
        assert!(
            armed.iter().any(|t| t == table),
            "{table} must carry ENABLE + FORCE ROW LEVEL SECURITY"
        );
    }

    let policies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_policy WHERE polrelid::regnamespace::text = 'promo'",
    )
    .fetch_one(&owner)
    .await
    .unwrap();
    assert_eq!(
        policies, 0,
        "the module ships no RLS policy — isolation belongs to the composing service's decorator"
    );
}

// ── Default-deny until composed: the plain probe role ─────────────────────────

/// A plain non-superuser, NOBYPASSRLS role with bare grants sees NOTHING and cannot
/// write — with or without the legacy company variable set. No policy admits it (there
/// are none), and none reads `app.company_id` anymore. The owner pool still sees its
/// seeded row: the denial is the missing policy, not an empty database.
#[tokio::test]
async fn plain_role_is_default_denied_until_the_decorator_composes() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let owner = pool().await;
    bootstrap_role(&owner, "loyalty_point_entries").await;
    let restricted = restricted_pool().await;

    // The owner seeds a ledger row as the superuser (whom RLS can never bind). No tenant column
    // exists to set — a ledger entry is just a row (ADR-0029).
    let customer = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO promo.loyalty_point_entries \
           (loyalty_program_id, customer_id, entry_type, points, purchase_amount, \
            source_type, source_id, posting_date) \
         VALUES ($1, $2, 'earned', 100, 0, 'probe', $3, now())",
    )
    .bind(Uuid::new_v4())
    .bind(customer)
    .bind(Uuid::new_v4())
    .execute(&owner)
    .await
    .unwrap();

    // Bare read: zero rows — default-deny with no policy admitting the role.
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM promo.loyalty_point_entries WHERE customer_id=$1",
    )
    .bind(customer)
    .fetch_one(&restricted)
    .await
    .unwrap();
    assert_eq!(n, 0, "a role no policy admits sees zero rows");

    // The legacy company variable resurrects nothing: no policy reads it anymore
    // (the decorator's org-scoped policies will, once composed). Transaction-local, so
    // nothing leaks across pooled connections.
    let mut tx = restricted.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(Uuid::new_v4().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM promo.loyalty_point_entries WHERE customer_id=$1",
    )
    .bind(customer)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(n, 0, "the legacy variable must not bypass the absent policy set");
    tx.rollback().await.unwrap();

    // A write is refused outright (no WITH CHECK policy admits the new row).
    let err = sqlx::query(
        "INSERT INTO promo.loyalty_point_entries \
           (loyalty_program_id, customer_id, entry_type, points, purchase_amount, \
            source_type, source_id, posting_date) \
         VALUES ($1, $2, 'earned', 1, 0, 'probe write', $3, now())",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&restricted)
    .await
    .expect_err("a write with no admitting policy must be refused");
    let code = err
        .as_database_error()
        .and_then(|db| db.code())
        .map(|c| c.to_string())
        .unwrap_or_default();
    assert_eq!(code, "42501", "the default-denied write hits row-level security, got {err}");
    assert!(
        err.to_string().to_lowercase().contains("row-level security"),
        "the refusal names row-level security, got {err}"
    );

    // The owner pool still sees its row.
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM promo.loyalty_point_entries WHERE customer_id=$1",
    )
    .bind(customer)
    .fetch_one(&owner)
    .await
    .unwrap();
    assert_eq!(n, 1, "the owner pool must still see the seeded row");

    drop(restricted);
    drop_role(&owner).await;
}
