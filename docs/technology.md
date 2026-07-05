# Technology & the "why"

> Reader: **Evaluator + Maintainer** · Mode: **Explanation** · Last reviewed 2026-07-05.
> Read this to understand the stack and the reasoning behind each choice. Each row names one rejected
> alternative, so the choice is a decision, not a default.

## The stack at a glance

| Layer | Choice | Version | One-line rationale | Rejected alternative |
|-------|--------|---------|--------------------|--------------------|
| Language | **Rust 2021** | edition 2021 | A pricing/ledger core wants correctness and no GC pauses on the hot path; the type system encodes the invariants (signed points, non-negative price) at compile time. | Go — simpler, but weaker at making illegal states unrepresentable. |
| Crate shape | **`[lib]` only** | — | A `module` is a bounded-context *library* consumed by services; it has no `main`. | A standalone service — wrong altitude; promo composes into selling/POS hosts. |
| DB | **PostgreSQL** | via SQLx | Partial unique indexes (`WHERE deleted_at IS NULL`), advisory locks, and transactional `ON CONFLICT` are exactly the primitives the idempotency + concurrency rules need. | MySQL — no partial indexes; the idempotency keys would need app-side emulation. |
| DB access | **SQLx 0.8** | 0.8 | Async, compile-time-checkable queries, no ORM magic between the resolver and the rows it ranks. | Diesel — sync-first and heavier; the resolver's ranked read is clearer in hand-written SQL. |
| Money | **`rust_decimal`** | 1.36 | Exact base-10 decimals; IDR at 2dp, half-away-from-zero. No binary float drift in prices or points. | `f64` — rounding error is unacceptable in money and a points ledger. |
| Async runtime | **Tokio** | 1.x | The ecosystem standard; SQLx, Axum, Tonic all target it. | async-std — smaller ecosystem, no upside here. |
| HTTP | **Axum** | 0.7 | The generated 12-endpoint CRUD surface per entity is Axum routers; Tower middleware composes the guarded router. | actix-web — heavier actor model; Axum's `Router` merge is what the codegen emits. |
| Errors | **`thiserror`** | 1.0 | Domain errors (`PricingError::CouponExhausted`, `InsufficientPoints`) are typed enums a caller can match on. | `anyhow` at the boundary — fine internally, but callers need to *branch* on the error kind. |
| Validation | **`validator`** | 0.16 | Declarative field validation generated from the schema attributes. | Hand-rolled guards — drift from the schema SSoT. |
| RPC (optional) | **Tonic / Prost** | 0.12 / 0.13 | Feature-gated; present for a future gRPC surface. **Disabled today** (`generators.disabled: [grpc, proto, graphql]`). | Always-on gRPC — unused weight; kept behind a feature flag. |

Versions are pinned in [`Cargo.toml`](../Cargo.toml). When behavior is version-specific, this handbook
says which version.

## The choice that defines the module: the port pattern

The most important technical decision isn't a library — it's the **`PriceResolverPort` trait** and the
**zero-normal-Cargo-edge** rule that follows from it.

- Selling/POS hold a `PriceResolverPort` *trait object*. They call `resolve(&PriceQuery)` and get back a
  `ResolvedPrice`. They do **not** depend on the `backbone-promo` crate.
- A composing service wires `PromoWriteService` behind the port (`PromoPriceResolver { service }`).
- Verify the decoupling holds: `cargo tree -e normal -i backbone-selling` from promo is **empty** —
  selling and POS appear only as `dev-dependencies` (the seam tests exercise the real write paths).

This is the same pattern POS uses to sit behind billing/payment. It is what makes promo genuinely
swappable rather than a compile-time dependency dressed up as a boundary. Full rationale in
[ADR-001](./adr/ADR-001-pricing-boundary-and-resolution-seam.md).

## Why schema-YAML codegen

promo is a `module`, so **`schema/models/*.model.yaml` is the single source of truth.** From it the
`metaphor-schema` plugin generates the entity structs, DTOs, migrations, repositories, service aliases,
HTTP handlers, and route registration. Hand-authored code lives only inside `// <<< CUSTOM` markers or
in sibling `*_custom.rs` / hand-owned files (`promo_write_service.rs`, `promo_ports.rs`,
`promo_events.rs`).

The rationale: the *plumbing* (12 CRUD endpoints, migrations, DTOs) is mechanical and identical across
entities — generating it eliminates a whole class of drift and boilerplate bugs. The *domain logic*
(the resolver, coupon bounding, the loyalty ledger) is what deserves human attention, so it is
hand-written and regeneration-safe. See the [Maintainer guide](./maintainer-guide.md) for the mechanics.

## Feature flags

Declared in [`Cargo.toml`](../Cargo.toml): `events`, `auth`, `grpc`, `openapi`, `validation` — all
**off by default** (`default = []`). The shipped library is the lean CRUD + write-service core; optional
surfaces are opt-in per host.

## What is deliberately absent

- **No GL / accounting library.** By design — promo posts no journal entries (see [Philosophy](./philosophy.md)).
- **No tax engine.** Region-neutral; PPN is billing/tax's job.
- **No network client for selling/POS.** The seam is in-process behind a trait; a caller composes it.
- **gRPC/proto/GraphQL generators are disabled** in `index.model.yaml` — the module ships an HTTP + port
  surface only, today.

---
Next: **[Architecture](./architecture.md)** — how these pieces are wired.
