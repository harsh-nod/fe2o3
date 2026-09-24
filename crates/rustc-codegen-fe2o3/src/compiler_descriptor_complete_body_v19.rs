//! Exact V19 source/ranked/formal descriptor continuation, not a V12 projection.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::ProductionCompleteBodyCheckedKirOwnerV19 as Checked;

pub(crate) fn construct_complete_body_v19_descriptor_source(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    checked: &Checked,
    budget: &mut Budget<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    checked
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::CompleteBodyV19(Box::new(error)))?;
    let module = checked.executable().module();
    let semantic = checked.semantic_ssa().source_semantic();
    let [root] = typed_roots else {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete-body one typed root",
        ));
    };
    let [semantic_root] = semantic.roots() else {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete-body one semantic root",
        ));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete-body one canonical kernel",
        ));
    };
    if semantic.wire_version() != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V36
        || envelope.target().to_string() != "gfx942:xnack-"
    {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete-body exact source/target profile",
        ));
    }
    validate_production_v1_semantic_ownership_evidence(typed_roots, semantic)?;
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete-body semantic root",
        ))?;
    let entry =
        function
            .kernel_entry()
            .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                "complete-body semantic entry",
            ))?;
    if entry.kernel_binding_identity().as_bytes() != &root.kernel_binding_bytes()
        || std::str::from_utf8(entry.export_symbol().as_bytes()).ok() != Some(root.entry_symbol())
        || kernel.id.as_str() != root.entry_symbol()
    {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete-body source/typed/canonical identity",
        ));
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
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete-body closed geometry",
        ));
    }
    let profiles = [DescriptorConstructionProfileV1 {
        rank: geometry.rank(),
        workgroup: geometry.workgroup(),
        max_grid: geometry.max_grid(),
        max_flat_workgroup_size: geometry.max_flat_workgroup_size(),
        static_shared_memory_bytes: 0,
        allow_exact_tiled_matrix: false,
        allow_workgroup_memory: false,
        producer_version: "complete-body-v19-gfx942-cov6-v1",
    }];
    construct_compiler_descriptor_source_with_profiles_and_complete_body_v19(
        envelope,
        module,
        compiler_module,
        typed_roots,
        &profiles,
        true,
    )?
    .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
        "complete-body typed descriptor closure",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared_module(name: &str) -> Module {
        let mut module = Module::new("inert-capability-only-control");
        module
            .required_capabilities
            .insert(fe2o3_kernel_ir::gfx942_xnack_minus_target_capability());
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave64,
            ));
        module
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: fe2o3_kernel_ir::AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAMESPACE_V19
                    .into(),
                name: name.into(),
            });
        module
    }

    #[test]
    fn old_descriptor_projection_does_not_admit_the_new_structural_capability() {
        let module =
            declared_module(fe2o3_kernel_ir::AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAME_V19);
        assert!(matches!(
            descriptor_capabilities(&module, false, false),
            Err(CompilerDescriptorError::UnsupportedCapability(_))
        ));
    }

    #[test]
    fn exact_v19_projection_preserves_the_existing_target_and_wave_capabilities() {
        let name = fe2o3_kernel_ir::AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAME_V19;
        let module = declared_module(name);
        let mut without_body = module.clone();
        without_body.required_capabilities.retain(|capability|
            !matches!(capability, TargetCapability::Extension { namespace, name: actual }
                if namespace == fe2o3_kernel_ir::AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAMESPACE_V19
                    && actual == name));
        assert_eq!(
            descriptor_capabilities_with_complete_body_v19(&module, false, false, true).unwrap(),
            descriptor_capabilities(&without_body, false, false).unwrap(),
        );
    }

    #[test]
    fn new_descriptor_projection_never_admits_unknown_or_near_matching_capabilities() {
        for name in [
            "complete_body_u32_e32_v2",
            "complete_body_u32_e32_v1-extra",
            "",
        ] {
            let module = declared_module(name);
            assert!(matches!(
                descriptor_capabilities_with_complete_body_v19(&module, false, false, true),
                Err(CompilerDescriptorError::UnsupportedCapability(_)),
            ));
        }
    }
}
