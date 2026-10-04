//! Same-F descriptor/V2 assembly. Caller retains the actual source/history owner.
use super::*;
use crate::compiler_descriptor::{
    TypedDescriptorRootV1,
    checked_output_policy3_v1::refined_forwarding_v1::{
        FinalOwnerV1, RefinedForwardingDescriptorErrorV1, construct_final_descriptor_source_v1,
    },
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_compiler_ffi::DeviceTargetV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
};

#[derive(Debug)]
pub(crate) enum FinalWorkerAssemblyErrorV1 {
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    Descriptor(Box<RefinedForwardingDescriptorErrorV1>),
    Handoff(Box<ProductionWorkerHandoffError>),
}
impl fmt::Display for FinalWorkerAssemblyErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for FinalWorkerAssemblyErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Descriptor(error) => Some(error.as_ref()),
            Self::Handoff(error) => Some(error.as_ref()),
        }
    }
}
fn handoff(error: ProductionWorkerHandoffError) -> FinalWorkerAssemblyErrorV1 {
    FinalWorkerAssemblyErrorV1::Handoff(Box::new(error))
}

pub(crate) struct FinalWorkerInputsV1<'a> {
    pub(crate) owner: FinalOwnerV1<'a>,
    pub(crate) catalog: &'a Catalog,
    pub(crate) profile: Profile,
    pub(crate) typed_roots: &'a [TypedDescriptorRootV1],
    pub(crate) source_envelope: Option<CompilerFfiEnvelopeV1>,
}

/// Uses the existing bounded descriptor/FFI/V2 engines. The owning caller
/// prepays their unchanged text/descriptor maxima and this input String's actual
/// capacity, and drops partial outputs before refunding its owning scope. Codec
/// allocations remain in the inherited opaque engine domain, not a heap bound.
/// No detached graph, numbered owner, source proof, or publication conversion.
pub(crate) fn prepare_v1(
    inputs: FinalWorkerInputsV1<'_>,
    llvm_ir: String,
    budget: &mut Budget<'_>,
) -> Result<PreparedProductionWorkerHandoff, FinalWorkerAssemblyErrorV1> {
    let FinalWorkerInputsV1 {
        owner,
        catalog,
        profile,
        typed_roots,
        source_envelope,
    } = inputs;
    let target = DeviceTargetV1::parse(profile.device_target())
        .map_err(|_| handoff(ProductionWorkerHandoffError::MissingProductionBindings))?;
    let module = owner.output().module();
    validate_exact_target_binding(target, module)
        .map_err(ProductionWorkerHandoffError::from)
        .map_err(handoff)?;
    let compiler_module = retain_production_compiler_module_text_v1(module, llvm_ir)
        .map_err(ProductionWorkerHandoffError::CompilerModule)
        .map_err(handoff)?;
    let envelope = derive_production_compiler_ffi_envelope(
        target,
        module,
        &compiler_module,
        source_envelope,
        owner.original_ffi_identity(),
    )
    .map_err(handoff)?;
    validate_envelope_module_roles(&envelope, &compiler_module)
        .map_err(ProductionWorkerHandoffError::from)
        .map_err(handoff)?;
    let descriptor = construct_final_descriptor_source_v1(
        &envelope,
        &compiler_module,
        typed_roots,
        owner,
        profile,
        budget,
    )
    .map_err(|error| FinalWorkerAssemblyErrorV1::Descriptor(Box::new(error)))?;
    let prepared =
        assemble_production_worker_handoff(target, envelope, compiler_module, descriptor)
            .map_err(handoff)?;
    replay_v1(owner, catalog, profile, &prepared, budget)?;
    Ok(prepared)
}

pub(crate) fn replay_v1(
    owner: FinalOwnerV1<'_>,
    catalog: &Catalog,
    profile: Profile,
    prepared: &PreparedProductionWorkerHandoff,
    budget: &mut Budget<'_>,
) -> Result<(), FinalWorkerAssemblyErrorV1> {
    let target = DeviceTargetV1::parse(profile.device_target())
        .map_err(|_| handoff(ProductionWorkerHandoffError::MissingProductionBindings))?;
    if prepared.handoff.kind() != CompilerModuleKindV1::LlvmTextIr
        || prepared.handoff.target() != target
        || prepared.handoff.code_object_version() != CodeObjectVersion::V6
        || prepared.handoff.module_bytes().len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
        || prepared.handoff.canonical_bytes().len()
            > fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
        || prepared.compiler_descriptor_source.canonical_bytes().len()
            > fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES
    {
        return Err(handoff(
            ProductionWorkerHandoffError::MissingProductionBindings,
        ));
    }
    budget
        .charge_work(
            prepared
                .handoff
                .module_bytes()
                .len()
                .checked_add(32)
                .ok_or(FinalWorkerAssemblyErrorV1::Resource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                ))?,
        )
        .map_err(FinalWorkerAssemblyErrorV1::Resource)?;
    if Sha256::digest(prepared.handoff.module_bytes()).as_slice() != prepared.llvm_ir_sha256 {
        return Err(handoff(
            ProductionWorkerHandoffError::MissingProductionBindings,
        ));
    }
    let final_llvm = std::str::from_utf8(prepared.handoff.module_bytes())
        .map_err(|_| handoff(ProductionWorkerHandoffError::MissingProductionBindings))?;
    let _ = dialect_amdgcn::check_native_v12_text_descriptor_relation_v1(
        owner.output(),
        catalog,
        owner.output().canonical().canonical_bytes(),
        profile,
        prepared.compiler_descriptor_source.table(),
        final_llvm,
        budget,
    )
    .map_err(ProductionWorkerHandoffError::NativeOutputReplay)
    .map_err(handoff)?;
    Ok(())
}

#[cfg(test)]
#[path = "production_worker_refined_forwarding_v1_tests.rs"]
pub(crate) mod tests;
