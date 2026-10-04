//! Fixed Rust -> Policy11 -> LICM -> Store consensus -> typed source proof -> V3.
//! Prepared inputs remain inert: this module cannot publish, finalize or launch.

use super::*;
use crate::compiler_descriptor::source_owned_v29::mixed_v28::{self as descriptor};
use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedFixedpointLicmOutputHandoffV29 as Native,
    ProductionConditionalMixedFixedpointOutputHandoffV29 as Prefix,
    ProductionMixedFixedpointLicmRelocationV29 as Relocation,
    ProductionMixedFixedpointStoreConsensusV46 as Consensus,
};
use fe2o3_verifier::PreparedTypedSourceTailV50 as Composed;
use target_result::mixed_licm_v28::{
    ConditionalMixedTargetLlvmV26 as TargetOwner, check_and_lower_mixed_target_llvm_v26,
    worker_input_v26::{
        PreparedMixedWorkerInputV26 as WorkerOwner, prepare_mixed_worker_input_v26,
    },
};

type Target<'h, 'v, 's> = TargetOwner<'h, 'v, 's, Native<'v, 'v, 'v, 's>>;
pub(crate) type Worker<'n, 'h, 'v, 's, 't, 'wire> =
    WorkerOwner<'n, 'h, 'v, 's, 't, 'wire, Native<'v, 'v, 'v, 's>>;
type MixedDescriptorWireV28<'h, 'v, 's> =
    descriptor::MixedDescriptorWireV28<'h, 'v, 's, Prefix<'v, 's>>;

#[path = "production_pipeline_source_mixed_publication_v28.rs"]
pub(crate) mod publication;

include!("production_pipeline_source_mixed_worker_family.rs");
mixed_worker_pipeline_family!(
    with_original_source_mixed_worker_input_v28,
    with_original_source_mixed_worker_test_limits_v28,
    fe2o3_verifier::prepare_typed_source_tail_with_references_v69,
    prepare_store_consensus_v46,
    complete_native_v46
);
