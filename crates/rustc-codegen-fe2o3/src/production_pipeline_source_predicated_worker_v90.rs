//! Full predicated Policy11, LICM, StoreConsensus and typed source continuation.
use super::predicated_licm_v90 as mixed_fixedpoint_licm_v29;
use super::*;
use crate::compiler_descriptor::source_owned_v29::mixed_v28 as descriptor;
use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalPredicatedFixedpointOutputHandoffV89 as Prefix,
    ProductionConditionalPredicatedLicmOutputHandoffV90 as Native,
    ProductionMixedRuntimeOccurrenceV89 as Occurrence,
    ProductionPredicatedFixedpointLicmRelocationV90 as Relocation,
    ProductionPredicatedStoreConsensusV90 as Consensus,
};
use fe2o3_verifier::PreparedPredicatedTypedSourceTailV90 as Composed;
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
    descriptor::MixedDescriptorWireV28<'h, 'v, 's, Prefix<'v, 's>, Occurrence>;

#[path = "production_pipeline_source_predicated_publication_v90.rs"]
pub(crate) mod publication;

include!("production_pipeline_source_mixed_worker_family.rs");
mixed_worker_pipeline_family!(
    with_original_source_predicated_worker_input_v90,
    with_original_source_predicated_worker_test_limits_v90,
    fe2o3_verifier::prepare_predicated_typed_source_tail_with_references_v90,
    prepare_predicated_store_consensus_v90,
    complete_native_v90
);
