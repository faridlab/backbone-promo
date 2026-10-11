// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

mod coupon_claim_repository;
mod loyalty_member_anchor_repository;
// The hand-written promo SQL's parameter/projection types (see the user-owned repositories,
// all declared `user_owned` in metaphor.codegen.yaml).
pub use coupon_claim_repository::{
    ActiveClaimRow, ClaimableCouponRow, CouponClaimRepository, CouponClaimRow,
};
pub use loyalty_member_anchor_repository::LoyaltyMemberAnchorRepository;
pub use loyalty_order_points_repository::{
    NewOrderGrantRow, NewOrderSpendRow, OrderPointsRow,
};
pub use loyalty_point_entry_repository::{
    NewAccrualRow, NewRedemptionRow, NewReversalRow, PriorRedemptionRow, PriorReversalRow,
};
pub use pricing_rule_repository::{LineRuleQuery, LineRuleRow, OrderRuleRow};
pub use promo_bundle_component_repository::BundleComponentRow;
pub use promo_bundle_gift_repository::BundleGiftRow;
pub use promo_bundle_repository::BundleRow;
// Re-bind the caller's ambient org scope onto a transaction opened on the plain pool — the
// scope is task-local and a fresh pool transaction carries none of it. With no ambient scope
// (standalone deployment, jobs) the transaction stays plain: the module is tenant-agnostic and
// the composing service's tenancy decorator owns isolation (ADR-0029). Same convention as the
// backbone-pos and backbone-livechat write paths.
pub(crate) async fn relay_ambient_scope(conn: &mut sqlx::PgConnection) -> Result<(), sqlx::Error> {
    if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
        backbone_orm::org_scope::bind_org_scope_on(conn, &scope).await?;
    }
    Ok(())
}
