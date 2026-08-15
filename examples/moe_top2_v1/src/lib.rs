#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Exact Phase A source and proof-facing contracts for deterministic top-2
//! mixture-of-experts routing.
//!
//! This crate contains real ordinary attributed Rust kernel source. It does
//! not claim compiler lowering, an admitted descriptor, an HSACO artifact,
//! hardware execution, protected evidence, or a machine-checked Verus/source
//! refinement proof.

pub mod contract;
pub mod kernel;
pub mod source_identity;

pub use contract::{
    DROP_ROUTE_V1, EXACT_PROFILE_V1, FINITE_LOGIT_POLICY_V1, LayoutV1,
    MOE_EXPERT_CAPACITY_V1, MOE_EXPERTS_V1, MOE_LOGIT_ELEMENTS_V1, MOE_ROUTES_V1,
    MOE_ROUTES_PER_TOKEN_V1, MOE_TOKENS_V1, MOE_WAVE_LANES_V1, MoeTop2ProfileV1,
    OverflowPolicyV1, ProfileMismatchV1, TieBreakPolicyV1, exact_launch_v1,
    logit_index_v1, route_id_v1, validate_profile_v1,
};
pub use source_identity::{
    MOE_KERNEL_NAMESPACE_V1, MOE_KERNEL_SOURCE_SHA256_V1, MOE_PROFILE_IDENTITY_V1,
    SourceIdentityMismatchV1, validate_kernel_source_identity_v1,
};
