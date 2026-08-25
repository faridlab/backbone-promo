//! Loyalty RLS fence probes — the loyalty tables under a NON-BYPASSRLS session.
//!
//! Row-Level Security only binds for a non-superuser, non-BYPASSRLS session (superusers always
//! bypass it), so these probes run against the restricted `promo_fence_probe` role minted and
//! granted by the admin pool — the same pattern backbone-selling's fence suite uses. Every probe
//! scopes the session with a TRANSACTION-LOCAL `set_config('app.company_id', …, true)`, so nothing
//! leaks across pooled connections.
//!
//! Tables probed: the entry ledger, the per-order points rows, and the member anchor lock table —
//! the three tables the loyalty write path touches — plus read fencing on the programs/coupons
//! masters. The deployment contract this enforces: the app must connect as a non-superuser role,
//! and every loyalty statement then fences to one company no matter what its WHERE clause says.

mod common;

use common::pool;
use sqlx::PgPool;
use uuid::Uuid;

const PROBE_ROLE: &str = "promo_fence_probe";
const PROBE_PASSWORD: &str = "probe";

/// Rebuild DATABASE_URL aimed at the probe role, keeping its host/port/database.
fn restricted_url(admin_url: &str) -> String {
    let rest = admin_url
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://");
    let (authority, path) = rest.split_once('/').expect("DATABASE_URL must name a database");
    let hostport = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let db = path.split('?').next().unwrap_or("backbone_promo");
    format!("postgresql://{PROBE_ROLE}:{PROBE_PASSWORD}@{hostport}/{db}")
}

/// A pool connected as the restricted probe role, minted and granted by the admin pool.
async fn restricted_pool(admin: &PgPool) -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_promo".to_string());
    let db = url
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://")
        .split_once('/')
        .and_then(|(_, path)| path.split('?').next())
        .unwrap_or("backbone_promo")
        .to_string();

    // Serialize mint + grants across parallel tests (shared-catalog DDL does not tolerate
    // concurrent GRANTs), then tolerate losing the race — the winner made the same role.
    sqlx::query("SELECT pg_advisory_lock(hashtext('promo_fence_probe'))")
        .execute(admin)
        .await
        .expect("take probe mint lock");
    let _ = sqlx::query(&format!(
        "CREATE ROLE {PROBE_ROLE} LOGIN PASSWORD '{PROBE_PASSWORD}' \
           NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE"
    ))
    .execute(admin)
    .await;
    // One statement per execute (a multi-command string is not a legal prepared statement). The
    // grants cover exactly the loyalty tables the write path touches — no more.
    for grant in [
        format!(r#"GRANT CONNECT ON DATABASE "{db}" TO {PROBE_ROLE}"#),
        format!("GRANT USAGE ON SCHEMA promo TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE promo.loyalty_point_entries TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE promo.loyalty_order_points TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE promo.loyalty_member_anchors TO {PROBE_ROLE}"),
        format!("GRANT SELECT ON TABLE promo.loyalty_programs TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE promo.coupon_codes TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE promo.coupon_redemptions TO {PROBE_ROLE}"),
    ] {
        sqlx::query(&grant).execute(admin).await.expect("grant probe role");
    }
    sqlx::query("SELECT pg_advisory_unlock(hashtext('promo_fence_probe'))")
        .execute(admin)
        .await
        .expect("release probe mint lock");

    PgPool::connect(&restricted_url(&url)).await.expect("connect as restricted probe")
}

/// Scope a fresh transaction on `pool` to `company`, transaction-local.
async fn scoped_tx(pool: &PgPool, company: Uuid) -> sqlx::Transaction<'_, sqlx::Postgres> {
    let mut tx = pool.begin().await.expect("begin scoped tx");
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(company.to_string())
        .execute(&mut *tx)
        .await
        .expect("scope tx");
    tx
}

/// LFP-1 — the entry ledger is read-fenced: a victim's entries are invisible to a foreign tenant's
/// scope even when the query names the victim's company explicitly.
#[tokio::test]
async fn lfp1_entry_ledger_reads_are_fenced() {
    let admin = pool().await;
    let restricted = restricted_pool(&admin).await;
    let victim = Uuid::new_v4();
    let attacker = Uuid::new_v4();

    // The victim's ledger, seeded through the admin (superuser) pool.
    sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date)
           VALUES ($1,$2,$3,'earned',100,0,'seed',$4, now())"#,
    )
    .bind(victim)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&admin)
    .await
    .unwrap();

    let mut tx = scoped_tx(&restricted, attacker).await;
    let visible: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM promo.loyalty_point_entries WHERE company_id = $1",
    )
    .bind(victim)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(visible, 0, "a foreign scope must not see the victim's ledger");

    // The victim's own scope sees exactly its row.
    let mut own = scoped_tx(&restricted, victim).await;
    let own_visible: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM promo.loyalty_point_entries WHERE company_id = $1",
    )
    .bind(victim)
    .fetch_one(&mut *own)
    .await
    .unwrap();
    own.commit().await.unwrap();
    assert_eq!(own_visible, 1);
}

/// LFP-2 — the entry ledger is write-fenced: writing another company's id into the ledger under a
/// foreign scope is refused by the policy's WITH CHECK, and writing under one's OWN scope passes it.
#[tokio::test]
async fn lfp2_entry_ledger_writes_are_fenced() {
    let admin = pool().await;
    let restricted = restricted_pool(&admin).await;
    let victim = Uuid::new_v4();
    let attacker = Uuid::new_v4();

    // Attacker tries to mint points ON THE VICTIM'S COMPANY under its own scope.
    let mut tx = scoped_tx(&restricted, attacker).await;
    let refused = sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date)
           VALUES ($1,$2,$3,'earned',100,0,'probe',$4, now())"#,
    )
    .bind(victim)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&mut *tx)
    .await;
    let msg = refused.err().map(|e| e.to_string()).unwrap_or_default();
    tx.rollback().await.unwrap();
    assert!(
        msg.contains("row-level security"),
        "cross-tenant ledger write must be refused by RLS, got: {msg}"
    );

    // Positive control: the same shape under the writer's OWN scope passes the WITH CHECK.
    let mut own = scoped_tx(&restricted, attacker).await;
    sqlx::query(
        r#"INSERT INTO promo.loyalty_point_entries
             (company_id, loyalty_program_id, customer_id, entry_type, points, purchase_amount,
              source_type, source_id, posting_date)
           VALUES ($1,$2,$3,'earned',50,0,'probe',$4, now())"#,
    )
    .bind(attacker)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&mut *own)
    .await
    .expect("own-company ledger write passes the fence");
    own.commit().await.unwrap();
}

/// LFP-3 — the per-order points rows are fenced both ways: invisible across tenants, unwritable
/// with a foreign company_id.
#[tokio::test]
async fn lfp3_order_points_rows_are_fenced() {
    let admin = pool().await;
    let restricted = restricted_pool(&admin).await;
    let victim = Uuid::new_v4();
    let attacker = Uuid::new_v4();

    // Seed a victim row through the admin pool.
    sqlx::query(
        r#"INSERT INTO promo.loyalty_order_points
             (company_id, loyalty_program_id, customer_id, order_ref_type, order_ref_id,
              grant_base_amount, granted_points)
           VALUES ($1,$2,$3,'pos_order',$4,10000,1000)"#,
    )
    .bind(victim)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&admin)
    .await
    .unwrap();

    let mut tx = scoped_tx(&restricted, attacker).await;
    let visible: i64 =
        sqlx::query_scalar("SELECT count(*) FROM promo.loyalty_order_points WHERE company_id = $1")
            .bind(victim)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(visible, 0, "a foreign scope must not see the victim's order rows");
    let refused = sqlx::query(
        r#"INSERT INTO promo.loyalty_order_points
             (company_id, loyalty_program_id, customer_id, order_ref_type, order_ref_id,
              grant_base_amount, granted_points)
           VALUES ($1,$2,$3,'pos_order',$4,1,1)"#,
    )
    .bind(victim)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&mut *tx)
    .await;
    let msg = refused.err().map(|e| e.to_string()).unwrap_or_default();
    tx.rollback().await.unwrap();
    assert!(
        msg.contains("row-level security"),
        "cross-tenant order-points write must be refused by RLS, got: {msg}"
    );
}

/// LFP-4 — the member anchor lock table is fenced: a foreign scope cannot mint an anchor on the
/// victim's company (the lock a write would take is itself tenant-bound).
#[tokio::test]
async fn lfp4_member_anchor_table_is_fenced() {
    let admin = pool().await;
    let restricted = restricted_pool(&admin).await;
    let victim = Uuid::new_v4();
    let attacker = Uuid::new_v4();

    // Attacker mints an anchor FOR THE VICTIM's member under the attacker's scope.
    let mut tx = scoped_tx(&restricted, attacker).await;
    let refused = sqlx::query(
        r#"INSERT INTO promo.loyalty_member_anchors (company_id, customer_id, loyalty_program_id)
           VALUES ($1,$2,$3)"#,
    )
    .bind(victim)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&mut *tx)
    .await;
    let msg = refused.err().map(|e| e.to_string()).unwrap_or_default();
    tx.rollback().await.unwrap();
    assert!(
        msg.contains("row-level security"),
        "minting a foreign member's anchor must be refused by RLS, got: {msg}"
    );

    // Positive control: minting one's own anchor works, and the NOWAIT lock the write path takes
    // succeeds under the owner's scope.
    let mut own = scoped_tx(&restricted, attacker).await;
    sqlx::query(
        r#"INSERT INTO promo.loyalty_member_anchors (company_id, customer_id, loyalty_program_id)
           VALUES ($1,$2,$3)"#,
    )
    .bind(attacker)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&mut *own)
    .await
    .expect("own anchor mint passes the fence");
    let locked: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM promo.loyalty_member_anchors
           WHERE company_id = $1 FOR UPDATE NOWAIT"#,
    )
    .bind(attacker)
    .fetch_optional(&mut *own)
    .await
    .unwrap();
    assert!(locked.is_some(), "the owner can take its own anchor lock");
    own.commit().await.unwrap();
}

/// LFP-5 — the read-side masters (loyalty programs, coupon codes) are read-fenced: program factors
/// and coupon state never leak across tenants even to a session that names them by id.
#[tokio::test]
async fn lfp5_masters_reads_are_fenced() {
    let admin = pool().await;
    let restricted = restricted_pool(&admin).await;
    let victim = Uuid::new_v4();
    let attacker = Uuid::new_v4();

    // A victim program and a victim coupon (admin-seeded).
    let program_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO promo.loyalty_programs
             (company_id, program_name, program_type, collection_factor, conversion_factor,
              from_date, status)
           VALUES ($1,'fence','single_tier',0.1,100, now() - interval '1 day', 'active')
           RETURNING id"#,
    )
    .bind(victim)
    .fetch_one(&admin)
    .await
    .unwrap();
    let item = Uuid::new_v4();
    let rule_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO promo.pricing_rules
             (company_id, title, priority, apply_on, item_id, rate_or_discount, valid_from, status)
           VALUES ($1,'fence',0,'item',$2,'discount_percentage', now() - interval '1 day', 'active')
           RETURNING id"#,
    )
    .bind(victim)
    .bind(item)
    .fetch_one(&admin)
    .await
    .unwrap();
    let coupon_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO promo.coupon_codes
             (company_id, code, pricing_rule_id, valid_from, status)
           VALUES ($1,'FENCE',$2, now() - interval '1 day', 'active') RETURNING id"#,
    )
    .bind(victim)
    .bind(rule_id)
    .fetch_one(&admin)
    .await
    .unwrap();

    let mut tx = scoped_tx(&restricted, attacker).await;
    let program: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM promo.loyalty_programs WHERE id = $1",
    )
    .bind(program_id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap();
    let coupon: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM promo.coupon_codes WHERE id = $1")
            .bind(coupon_id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap();
    tx.commit().await.unwrap();
    assert!(program.is_none(), "a foreign scope must not read the victim's program factors");
    assert!(coupon.is_none(), "a foreign scope must not read the victim's coupon state");
}
