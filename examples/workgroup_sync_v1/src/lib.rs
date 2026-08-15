#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Source, oracle, and formal contracts for two fixed wave64 synchronization profiles.
//!
//! Both profiles are type-checked as ordinary attributed Rust kernels. Their
//! exact compiler profiles, artifacts, and protected runtime paths remain
//! separate evidence phases.

pub mod contract;
pub mod kernel;
pub mod scoped_atomic;
pub mod vectors;

pub use contract::{
    ATOMIC_ADDRESS_SPACE_V1, ATOMIC_ORDERING_V1, ATOMIC_SCOPE_V1, AtomicAddressSpaceV1,
    AtomicComparisonErrorV1, AtomicLaneV1, AtomicOrderingV1, AtomicProfileErrorV1, AtomicProfileV1,
    AtomicScopeV1, LDS_OWNER_LANE_V1, ReductionComparisonErrorV1, ReductionLaneV1,
    ReductionProfileErrorV1, WORKGROUP_LANES_V1, atomic_add_oracle_v1, canonical_atomic_lanes_v1,
    canonical_atomic_profile_v1, canonical_reduction_trace_v1, compare_atomic_output_v1,
    compare_reduction_output_v1, lds_reduction_oracle_v1,
};
pub use kernel::{
    LDS_REDUCTION_COMPILER_PROFILE_REGISTERED_V1, LDS_REDUCTION_WORKGROUP_V1,
    SCOPED_ATOMIC_COMPILER_PROFILE_REGISTERED_V1,
};
pub use vectors::{AtomicVectorV1, ReductionVectorV1, atomic_vectors_v1, reduction_vectors_v1};
