//! Normal inert descriptor continuation requires both output and compiler-ABI
//! memory records from the same retained checked source owner.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::ProductionPhysicalEntryCheckedKirOwnerV20 as Checked;
#[path = "compiler_descriptor_physical_entry_abi_v20.rs"]
mod abi;
#[cfg(test)]
pub(crate) use abi::qualify_actual_owner_abi_controls_v20;
pub(crate) use abi::{PhysicalEntryPreparedAbiV20, prepare_physical_entry_abi_v20};

/// Closed borrowed capability permission. It can only be made after source,
/// ABI, mandatory ranked/formal and exact module joins below.
pub(super) struct PhysicalEntryDescriptorAdmissionV20<'a> {
    module: &'a Module,
    _checked: &'a Checked,
    _abi: &'a PhysicalEntryPreparedAbiV20,
}
impl PhysicalEntryDescriptorAdmissionV20<'_> {
    pub(super) fn admits(&self, module: &Module, capability: &TargetCapability) -> bool {
        std::ptr::eq(self.module, module)
            && matches!(capability,
            TargetCapability::Extension{namespace,name}
            if namespace==fe2o3_kernel_ir::AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20
                &&name==fe2o3_kernel_ir::AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20)
    }
}
fn mismatch(detail: &'static str) -> CompilerDescriptorError {
    CompilerDescriptorError::ProductionDescriptorMismatch(detail)
}
pub(crate) fn construct_physical_entry_descriptor_source_v20(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    checked: &Checked,
    abi: &PhysicalEntryPreparedAbiV20,
    budget: &mut Budget<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    checked
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::PhysicalEntryV20(Box::new(error)))?;
    abi::validate_prepared_abi_v20(checked, typed_roots, abi, budget)?;
    let [root] = typed_roots else {
        return Err(mismatch("physical descriptor one actual typed root"));
    };
    let module = checked.executable().module();
    let semantic = checked.semantic_ssa().source_semantic();
    let [semantic_root] = semantic.roots() else {
        return Err(mismatch("physical descriptor one source root"));
    };
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or_else(|| mismatch("physical descriptor actual semantic function"))?;
    let [kernel] = module.kernels.as_slice() else {
        return Err(mismatch("physical descriptor one actual kernel"));
    };
    if envelope.target().to_string() != "gfx942:xnack-"
        || envelope.code_object_version() != CodeObjectVersion::V6
    {
        return Err(mismatch("physical descriptor exact target/COV6"));
    }
    // Source/physical ABI reads were joined separately above. The ordinary
    // validator consumes the real output allocation report, not a fake kernarg
    // parameter or a claim that this is the whole physical memory report.
    let geometry = validate_production_v1_descriptor_root_evidence(
        module,
        root,
        semantic,
        function,
        kernel,
        checked.memory_obligations().output(),
        "gfx942:xnack-",
    )?;
    if geometry.rank() != 1
        || geometry.workgroup() != [64, 1, 1]
        || geometry.max_grid() != [2, 1, 1]
        || geometry.max_flat_workgroup_size() != 64
        || geometry.static_shared_memory_bytes() != 0
        || geometry.allow_exact_tiled_matrix()
        || geometry.allow_workgroup_memory()
    {
        return Err(mismatch("physical descriptor exact source geometry"));
    }
    let profiles = [DescriptorConstructionProfileV1 {
        rank: 1,
        workgroup: [64, 1, 1],
        max_grid: [2, 1, 1],
        max_flat_workgroup_size: 64,
        static_shared_memory_bytes: 0,
        allow_exact_tiled_matrix: false,
        allow_workgroup_memory: false,
        producer_version: "physical-entry-v20-gfx942-cov6-inert-v1",
    }];
    let admission = PhysicalEntryDescriptorAdmissionV20 {
        module,
        _checked: checked,
        _abi: abi,
    };
    construct_compiler_descriptor_source_with_profiles_and_admission_v1(
        envelope,
        module,
        compiler_module,
        typed_roots,
        &profiles,
        DescriptorCapabilityAdmissionV1::PhysicalEntryV20(&admission),
    )?
    .ok_or_else(|| mismatch("physical descriptor typed closure"))
}
#[cfg(test)]
#[path = "compiler_descriptor_physical_entry_v20_tests.rs"]
mod tests;
