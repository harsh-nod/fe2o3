//! Concrete descriptor V53 and exact typed source publication.
use super::*;
use crate::kernel_ir_codegen::mixed_v53 as mixed_module;
use fe2o3_compiler_ffi::{CodeObjectVersion, CompilerModuleHandoffV2};
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_descriptor::mixed_conditional_v26::{
    MIXED_CONTRACT_CODEC_STORAGE_V26 as CONTRACT_STORAGE, MixedContractErrorV26 as ContractError,
    MixedContractV26 as Contract, decode_mixed_contract_v26 as decode_contract,
};
use fe2o3_kernel_descriptor::{
    MIXED_DESCRIPTOR_READER_STORAGE_V53 as DESCRIPTOR_STORAGE,
    MixedDescriptorErrorV53 as DescriptorError, MixedDescriptorTableV53 as DescriptorTable,
    decode_mixed_descriptor_v53 as decode_descriptor,
    encode_mixed_descriptor_v53 as encode_descriptor,
    encoded_mixed_descriptor_v53_len as encoded_descriptor_len,
};
use fe2o3_verifier::{
    MixedTargetSelectionSubjectV53 as SelectionSubject,
    MixedTargetSelectionValidationErrorV53 as SelectionError,
    with_mixed_target_selection_v53 as with_selection,
};
use mixed_module::MixedModuleErrorV53 as ModuleError;
use std::mem::size_of_val;

#[cfg(test)]
const PUBLICATION_WRAPPER: &str = include_str!("production_pipeline_source_mixed_publish_v53.rs");
#[cfg(test)]
const MODULE_WRAPPER: &str = include_str!("kernel_ir_codegen_mixed_descriptor_v53.rs");
#[cfg(test)]
const DESCRIPTOR_SECTION: &str = ".fe2o3.kd.v53";

include!("production_pipeline_source_mixed_publish_family.rs");
mixed_publish_pipeline_family!(
    publish_mixed_worker_handoff_v53,
    with_original_source_mixed_publication_on_account_v28,
    replay_lineage_capsule_v50
);
