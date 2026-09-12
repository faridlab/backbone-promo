//! Guarded route composition — the RECOMMENDED way to mount the promo module.
//!
//! Hand-authored (user-owned). Mirrors the pattern in backbone-pos / backbone-inventory: the read
//! surface is wrapped in `company_auth`. The module is tenant-agnostic — it ships no row fence of
//! its own; row scoping is the composing service's tenancy decorator (ADR-0029), and the generic
//! list/get paths ride the request-dedicated connection when the composer bound one. Unauthenticated
//! reads get 401 — these surfaces expose pricing/loyalty data.
//!
//! Generic create/update/delete CRUD is NOT mounted (no caller can mint a coupon or alter a pricing
//! rule directly); validated writes go through `PromoWriteService` (service/job-driven, not a bare
//! HTTP route). `all_crud_routes()` remains available for the intentional full/unguarded surface.

use axum::{middleware::from_fn_with_state, Router};
use backbone_auth::company::{company_auth, CompanyVerifier};

use crate::PromoModule;

use super::{
    create_coupon_code_read_routes, create_coupon_redemption_read_routes,
    create_loyalty_order_points_read_routes, create_loyalty_program_read_routes,
    create_loyalty_point_entry_read_routes, create_pricing_rule_read_routes,
    create_promo_bundle_read_routes, create_promo_bundle_component_read_routes,
};

/// Mount the promo module's read surface, authenticated. The module is tenant-agnostic: reads are
/// scoped by the composing service's tenancy decorator, not by any module-owned filter (ADR-0029).
/// `pool` is accepted for symmetry with the other guarded composers and for future validated-write
/// routes; the read surface itself uses the services' own pools.
#[allow(unused_variables)]
pub fn create_guarded_promo_routes(m: &PromoModule, pool: sqlx::PgPool, verifier: CompanyVerifier) -> Router {
    let reads = Router::new()
        .merge(create_coupon_code_read_routes(m.coupon_code_service.clone()))
        .merge(create_coupon_redemption_read_routes(m.coupon_redemption_service.clone()))
        .merge(create_loyalty_order_points_read_routes(m.loyalty_order_points_service.clone()))
        .merge(create_loyalty_program_read_routes(m.loyalty_program_service.clone()))
        .merge(create_loyalty_point_entry_read_routes(m.loyalty_point_entry_service.clone()))
        .merge(create_pricing_rule_read_routes(m.pricing_rule_service.clone()))
        .merge(create_promo_bundle_read_routes(m.promo_bundle_service.clone()))
        .merge(create_promo_bundle_component_read_routes(m.promo_bundle_component_service.clone()))
        .route_layer(from_fn_with_state(verifier, company_auth));
    Router::new().merge(reads)
}
