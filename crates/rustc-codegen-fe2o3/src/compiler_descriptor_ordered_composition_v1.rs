//! Descriptor admission only from the exact checked source composition owner.
//! Full formal and conservative launch-envelope conditions remain unresolved;
//! the ordinary inert descriptor does not serialize a runtime discharge.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, FormalMemoryAccessKind,
};
use fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1 as Checked;

pub(super) struct OrderedCompositionDescriptorAdmissionV1<'a> {
    module: &'a Module,
    _checked: &'a Checked,
}
impl OrderedCompositionDescriptorAdmissionV1<'_> {
    pub(super) fn admits(&self, module: &Module, capability: &TargetCapability) -> bool {
        std::ptr::eq(self.module, module) && ordered_capability(capability)
    }
}
fn ordered_capability(capability: &TargetCapability) -> bool {
    matches!(capability, TargetCapability::Extension { namespace, name }
        if namespace == fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE
        && name == fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME)
}
fn mismatch(message: &'static str) -> CompilerDescriptorError {
    CompilerDescriptorError::ProductionDescriptorMismatch(message)
}

/// Validates the independently computed additional condition roster against the
/// actual original accesses and source geometry. No allocation data is supplied.
fn validate_envelope_conditions(
    checked: &Checked,
    root: &TypedDescriptorRootV1,
    geometry: crate::production_geometry_v1::ProductionGeometryV1,
    budget: &mut Budget<'_>,
) -> Result<(), CompilerDescriptorError> {
    budget
        .charge_work(512)
        .map_err(CompilerDescriptorError::OrderedCompositionResourceV1)?;
    let [source] = checked.source_launch().roots() else {
        return Err(mismatch("composition source launch roster"));
    };
    let grid = geometry.max_grid();
    let expected = u64::from(grid[0])
        .checked_mul(64)
        .ok_or_else(|| mismatch("composition launch extent overflow"))?;
    if grid[0] == 0
        || grid[0] == u32::MAX
        || grid[1..] != [1, 1]
        || source.layout().global_extents() != [expected, 1, 1]
        || source.layout().workgroup_extents() != geometry.workgroup().map(u64::from)
    {
        return Err(mismatch("composition finite actual source launch"));
    }
    let bytes = expected
        .checked_mul(4)
        .ok_or_else(|| mismatch("composition buffer extent overflow"))?;
    let report = checked.formal_obligations();
    let conditions = checked.launch_envelope_requirements();
    if conditions.len() > 8 || report.accesses().len() > 16 || report.allocations().len() > 8 {
        return Err(mismatch("composition retained condition bounds"));
    }
    let mut seen = [false; 8];
    for row in conditions {
        let index = usize::try_from(row.parameter_index())
            .map_err(|_| mismatch("composition condition parameter"))?;
        if index >= seen.len() || seen[index] {
            return Err(mismatch("composition unique condition parameter"));
        }
        seen[index] = true;
        let argument = root
            .arguments
            .as_slice()
            .get(index)
            .ok_or_else(|| mismatch("composition typed condition parameter"))?;
        let allocation = report
            .allocations()
            .iter()
            .find(|a| a.identity().parameter_index() == row.parameter_index())
            .ok_or_else(|| mismatch("composition formal condition allocation"))?;
        let mut read = false;
        let mut write = false;
        for access in report
            .accesses()
            .iter()
            .filter(|a| a.allocation() == allocation.identity())
        {
            match access.kind() {
                FormalMemoryAccessKind::Read => read = true,
                FormalMemoryAccessKind::Write => write = true,
                FormalMemoryAccessKind::Atomic => {
                    return Err(mismatch("composition atomic condition"));
                }
            }
        }
        if !condition_shape(
            row.minimum_byte_len(),
            bytes,
            row.requires_initialized_read(),
            row.requires_write_permission(),
            read,
            write,
        ) || (read && argument.access == AccessMode::WriteOnly)
            || (write && argument.access == AccessMode::ReadOnly)
        {
            return Err(mismatch("composition exact unresolved launch envelope"));
        }
    }
    for access in report.accesses() {
        let index = usize::try_from(access.allocation().parameter_index())
            .map_err(|_| mismatch("composition access parameter"))?;
        if !seen.get(index).copied().unwrap_or(false) {
            return Err(mismatch("composition missing unresolved envelope"));
        }
    }
    Ok(())
}
fn condition_shape(
    actual_bytes: u64,
    expected_bytes: u64,
    actual_read: bool,
    actual_write: bool,
    read: bool,
    write: bool,
) -> bool {
    expected_bytes != 0
        && actual_bytes == expected_bytes
        && (read || write)
        && actual_read == read
        && actual_write == write
}
pub(crate) fn construct_ordered_composition_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    checked: &Checked,
    budget: &mut Budget<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    checked
        .verify_equivalence(budget)
        .map_err(|e| CompilerDescriptorError::OrderedCompositionV1(Box::new(e)))?;
    let module = checked.executable().module();
    let semantic = checked.semantic_ssa().source_semantic();
    let [root] = typed_roots else {
        return Err(mismatch("composition one typed root"));
    };
    let [semantic_root] = semantic.roots() else {
        return Err(mismatch("composition one semantic root"));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(mismatch("composition one canonical kernel"));
    };
    if semantic.wire_version() != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V32
        || envelope.target().to_string() != "gfx942:xnack-"
        || envelope.code_object_version() != CodeObjectVersion::V6
    {
        return Err(mismatch("composition exact source/target profile"));
    }
    validate_production_v1_semantic_ownership_evidence(typed_roots, semantic)?;
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or_else(|| mismatch("composition semantic root"))?;
    let entry = function
        .kernel_entry()
        .ok_or_else(|| mismatch("composition semantic entry"))?;
    if entry.kernel_binding_identity().as_bytes() != &root.kernel_binding_bytes()
        || std::str::from_utf8(entry.export_symbol().as_bytes()).ok() != Some(root.entry_symbol())
        || kernel.id.as_str() != root.entry_symbol()
    {
        return Err(mismatch("composition actual source/typed/canonical entry"));
    }
    let geometry = validate_production_v1_descriptor_root_evidence(
        module,
        root,
        semantic,
        function,
        kernel,
        checked.formal_obligations(),
        "gfx942:xnack-",
    )?;
    if geometry.rank() != 1
        || geometry.workgroup() != [64, 1, 1]
        || geometry.max_flat_workgroup_size() != 64
        || geometry.static_shared_memory_bytes() != 0
        || geometry.allow_exact_tiled_matrix()
        || geometry.allow_workgroup_memory()
    {
        return Err(mismatch("composition closed ordinary source geometry"));
    }
    validate_envelope_conditions(checked, root, geometry, budget)?;
    let profiles = [DescriptorConstructionProfileV1 {
        rank: geometry.rank(),
        workgroup: geometry.workgroup(),
        max_grid: geometry.max_grid(),
        max_flat_workgroup_size: geometry.max_flat_workgroup_size(),
        static_shared_memory_bytes: 0,
        allow_exact_tiled_matrix: false,
        allow_workgroup_memory: false,
        producer_version: "ordered-composition-v1-gfx942-cov6",
    }];
    let admission = OrderedCompositionDescriptorAdmissionV1 {
        module,
        _checked: checked,
    };
    construct_compiler_descriptor_source_with_profiles_and_admission_v1(
        envelope,
        module,
        compiler_module,
        typed_roots,
        &profiles,
        DescriptorCapabilityAdmissionV1::OrderedCompositionV1(&admission),
    )?
    .ok_or_else(|| mismatch("composition typed descriptor closure"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_descriptor_refuses_structural_composition_capability() {
        let mut module = Module::new("inert-only-capability-control");
        module
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE
                    .into(),
                name: fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME.into(),
            });
        assert!(matches!(
            descriptor_capabilities(&module, false, false),
            Err(CompilerDescriptorError::UnsupportedCapability(_))
        ));
    }
    #[test]
    fn near_capabilities_do_not_gain_projection() {
        for name in ["", "ordered-program-v2", "arbitrary_extension"] {
            assert!(!ordered_capability(&TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE
                    .into(),
                name: name.into(),
            }));
        }
    }
    #[test]
    fn additional_condition_flags_and_exact_extent_are_not_optional() {
        assert!(condition_shape(512, 512, false, true, false, true));
        assert!(condition_shape(512, 512, true, true, true, true));
        for bytes in [0, 256, 511, 513, u64::MAX] {
            assert!(!condition_shape(bytes, 512, false, true, false, true));
        }
        assert!(!condition_shape(512, 512, false, false, false, true));
        assert!(!condition_shape(512, 512, true, true, false, true));
        assert!(!condition_shape(512, 512, false, true, true, true));
        assert!(!condition_shape(512, 512, false, false, false, false));
    }
}
