//! Concrete descriptor V89 and exact typed source publication.
use super::*;
use crate::kernel_ir_codegen::mixed_v89 as mixed_module;
use fe2o3_compiler_ffi::{CodeObjectVersion, CompilerModuleHandoffV2};
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_descriptor::mixed_conditional_v86::{
    MIXED_CONTRACT_CODEC_STORAGE_V86 as CONTRACT_STORAGE, MixedContractErrorV86 as ContractError,
    MixedContractV86 as Contract, decode_mixed_contract_v86 as decode_contract,
};
use fe2o3_kernel_descriptor::{
    MIXED_DESCRIPTOR_READER_STORAGE_V89 as DESCRIPTOR_STORAGE,
    MixedDescriptorErrorV89 as DescriptorError, MixedDescriptorTableV89 as DescriptorTable,
    decode_mixed_descriptor_v89 as decode_descriptor,
    encode_mixed_descriptor_v89 as encode_descriptor,
    encoded_mixed_descriptor_v89_len as encoded_descriptor_len,
};
use fe2o3_verifier::{
    MixedTargetSelectionSubjectV89 as SelectionSubject,
    MixedTargetSelectionValidationErrorV89 as SelectionError,
    with_mixed_target_selection_v89 as with_selection,
};
use mixed_module::MixedModuleErrorV89 as ModuleError;
use std::mem::size_of_val;

#[cfg(test)]
const PUBLICATION_WRAPPER: &str =
    include_str!("production_pipeline_source_predicated_publish_v90.rs");
#[cfg(test)]
const MODULE_WRAPPER: &str = include_str!("kernel_ir_codegen_mixed_descriptor_v89.rs");
#[cfg(test)]
const DESCRIPTOR_SECTION: &str = ".fe2o3.kd.v89";

include!("production_pipeline_source_mixed_publish_family.rs");
mixed_publish_pipeline_family!(
    publish_predicated_worker_handoff_v90,
    with_original_source_predicated_publication_on_account_v90,
    replay_lineage_capsule_v90
);
