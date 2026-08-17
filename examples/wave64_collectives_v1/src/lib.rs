#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Exact source, CPU oracle, and proof-facing contract for masked Wave64 sums.
//!
//! One logical active-lane mask selects participants in a physically
//! convergent 64-lane wave. The crate includes ordinary attributed Rust kernel
//! source, an exact finite-corpus host oracle, a Verus integer model, and an
//! exact-syntax reviewed correspondence from the attributed source to the CPU
//! model, plus an identity-bound executable correspondence from that source
//! model to the canonical semantic Kernel IR profile. Neither correspondence
//! grants compiler, artifact, runtime, machine, generalized-safety, or parity
//! authority.

pub mod contract;
pub mod kernel;
pub mod oracle;
pub mod source_kir_refinement;
pub mod source_model_correspondence;

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
pub use source_model_correspondence::{
    REVIEWED_SOURCE_CPU_CORRESPONDENCE_BOUNDARY_V2, ReviewedSourceAlgorithmV2, SourceCpuBindingV2,
    SourceCpuContentIdentitiesV2, SourceCpuCorrespondenceErrorV2, SourceCpuCorrespondenceReceiptV2,
    SourceCpuOutputsV2, SourceStructureErrorV2, bind_source_cpu_content_to_outer_commit_v2,
    collect_reviewed_source_algorithm_v2, exact_source_cpu_content_identities_v2,
    interpret_reviewed_source_algorithm_v2, verify_reviewed_source_to_cpu_correspondence_v2,
};
