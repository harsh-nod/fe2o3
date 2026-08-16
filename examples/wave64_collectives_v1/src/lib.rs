#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Exact source, CPU oracle, and proof-facing contract for masked Wave64 sums.
//!
//! One logical active-lane mask selects participants in a physically
//! convergent 64-lane wave. The crate includes ordinary attributed Rust kernel
//! source, an exact finite-corpus host oracle, a Verus integer model, and an
//! identity-bound executable correspondence from that source model to the
//! canonical semantic Kernel IR profile. The correspondence grants no
//! compiler, artifact, runtime, machine, generalized-safety, or parity
//! authority.

pub mod contract;
pub mod kernel;
pub mod oracle;
pub mod source_kir_refinement;

pub use contract::{
    EMPTY_MASK_POLICY_V1, INACTIVE_LANE_OUTPUT_POLICY_V1, LaneOutputsV1,
    MAX_EXACT_INPUT_MAGNITUDE_V1, PHYSICAL_EXECUTION_POLICY_V1, WAVE64_LANES_V1, lane_is_active_v1,
    lane_outputs_v1,
};
pub use oracle::{
    CollectiveOutputV1, OracleErrorV1, OracleErrorV1OrMismatch, OracleStateV1, OutputMismatchV1,
    compare_wave64_collectives_v1, wave64_collectives_oracle_v1,
};
pub use source_kir_refinement::{
    WAVE64_COLLECTIVES_V1_KIR_SCHEMA_SHA256, WAVE64_REFINEMENT_BOUNDARY_V1,
    Wave64RefinementErrorV1, Wave64RefinementIdentitiesV1, Wave64SemanticOutputV1,
    Wave64SemanticOutputsV1, Wave64SourceKirRefinementV1, exact_wave64_refinement_identities_v1,
    source_contributor_mask_v1, verify_wave64_source_model_to_kir_v1,
};
