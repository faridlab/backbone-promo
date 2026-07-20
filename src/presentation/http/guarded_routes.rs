//! Guarded route composition — the RECOMMENDED way to mount the promo module.
//!
//! Hand-authored (user-owned). Mirrors the pattern in backbone-pos / backbone-inventory: every
//! promo entity is company-scoped, so the read surface is wrapped in `company_auth`. The guard
//! establishes the request scope (`app.company_id` bound on a dedicated connection via
//! `with_request_scope`); the generic list/get path runs through `company_scope::fetch_*_scoped`,
//! which rides that connection, so RLS returns only the caller's company rows. Unauthenticated reads
//! get 401 — these surfaces expose company pricing/loyalty data.
//!
//! Generic create/update/delete CRUD is NOT mounted (no caller can mint a coupon or alter a pricing
//! rule directly); validated writes go through `PromoWriteService` (service/job-driven, not a bare
//! HTTP route). `all_crud_routes()` remains available for the intentional full/unguarded surface.

use axum::{middleware::from_fn_with_state, Router};
use backbone_auth::company::{company_auth, CompanyVerifier};

use crate::PromoModule;

use super::{
    create_coupon_code_read_routes, create_coupon_redemption_read_routes,
    create_loyalty_program_read_routes, create_loyalty_point_entry_read_routes,
    create_pricing_rule_read_routes, create_promo_bundle_read_routes,
    create_promo_bundle_component_read_routes,
};

/// Mount the promo module's company-scoped read surface, authenticated. Every entity here carries a
/// `company_id` (coupon_code, coupon_redemption, loyalty_program, loyalty_point_entry, pricing_rule,
/// promo_bundle, promo_bundle_component), so all reads fence to the caller's company under the RLS
/// app role. `pool` is accepted for symmetry with the other guarded composers and for future
/// validated-write routes; the read surface itself uses the services' own pools.
#[allow(unused_variables)]
pub fn create_guarded_promo_routes(m: &PromoModule, pool: sqlx::PgPool, verifier: CompanyVerifier) -> Router {
    let reads = Router::new()
        .merge(create_coupon_code_read_routes(m.coupon_code_service.clone()))
        .merge(create_coupon_redemption_read_routes(m.coupon_redemption_service.clone()))
        .merge(create_loyalty_program_read_routes(m.loyalty_program_service.clone()))
        .merge(create_loyalty_point_entry_read_routes(m.loyalty_point_entry_service.clone()))
        .merge(create_pricing_rule_read_routes(m.pricing_rule_service.clone()))
        .merge(create_promo_bundle_read_routes(m.promo_bundle_service.clone()))
        .merge(create_promo_bundle_component_read_routes(m.promo_bundle_component_service.clone()))
        .route_layer(from_fn_with_state(verifier, company_auth));
    Router::new().merge(reads)
}
