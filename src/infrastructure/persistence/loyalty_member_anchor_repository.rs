//! The per-member loyalty serialization anchor (hand-authored, user-owned).
//!
//! WHY THIS EXISTS: the member's balance is an aggregate SUM over the entry ledger, which cannot
//! take `FOR UPDATE`; locking only the member's EXISTING entry rows drops mutual exclusion for
//! row-less members and is phantom-open under concurrent earns. One anchor row per
//! (company, customer, program), taken `FOR UPDATE NOWAIT`, is the mutex that keeps exclusion for
//! every member from the first touch. It is lock machinery, not a domain entity — no CRUD surface
//! is mounted on it (see the migration that creates it).
//!
//! Per the module's 4-layer rule the SQL lives here; the write service orchestrates. The table is
//! RLS-fenced (ADR-0014 strict), so every method rides the caller's already-bound connection.

/// Repository for the promo.loyalty_member_anchors lock table.
///
/// Unit-shaped on purpose: both statements run on the CALLER'S transaction (`ensure_and_lock`
/// serializes the caller against every other balance writer for the member), so there is no pool to
/// hold.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoyaltyMemberAnchorRepository;

impl LoyaltyMemberAnchorRepository {
    pub fn new() -> Self {
        Self
    }
}

impl LoyaltyMemberAnchorRepository {
    /// Ensure the member's anchor row exists, then take it `FOR UPDATE NOWAIT`. Two statements, on
    /// the CALLER'S transaction (the company scope is already bound — don't re-bind here):
    ///
    ///   1. a speculative `INSERT ... ON CONFLICT DO NOTHING` — mints the anchor at first touch;
    ///   2. `SELECT ... FOR UPDATE NOWAIT` on the single row — the mutex itself.
    ///
    /// Mutual exclusion: two first-touch transactions racing on the same member serialize on the
    /// speculative insert's unique conflict (the loser waits for the winner's transaction to end —
    /// bounded by that transaction's lifetime, not a deadlock); afterwards the single anchor row is
    /// the NOWAIT mutex, so a loser fails fast with SQLSTATE 55P03 (the service maps that one code
    /// to [`crate::application::service::promo_ports::PricingError::LockBusy`]).
    pub async fn ensure_and_lock(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: uuid::Uuid,
        customer_id: uuid::Uuid,
        loyalty_program_id: uuid::Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO promo.loyalty_member_anchors (company_id, customer_id, loyalty_program_id)
               VALUES ($1, $2, $3)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(company_id)
        .bind(customer_id)
        .bind(loyalty_program_id)
        .execute(&mut *conn)
        .await?;
        sqlx::query(
            r#"SELECT id FROM promo.loyalty_member_anchors
               WHERE company_id = $1 AND customer_id = $2 AND loyalty_program_id = $3
               FOR UPDATE NOWAIT"#,
        )
        .bind(company_id)
        .bind(customer_id)
        .bind(loyalty_program_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(())
    }
}
