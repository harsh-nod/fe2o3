//! Inert MIR34 owner/replay controls, not live rustc authentication or native evidence.
use super::super::{
    DescriptorCapabilityAdmissionV1, descriptor_capabilities,
    descriptor_capabilities_with_admission_v1, descriptor_capabilities_with_complete_body_v19,
};
use super::*;
use fe2o3_kernel_descriptor::CapabilityV1;
use fe2o3_kernel_ir::{AssemblyEffect, AssemblyOperandKind};
use fe2o3_lower_mir_kernel::{
    ProductionRankedSemanticProjectionReceiptV1, ProductionSemanticKirLimitsV1,
    ProductionSemanticKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticGfx942InlineInstructionV30 as Instruction;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionSemanticMirLimitsV1,
    ProductionSemanticMirOwnerV1, ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/helper_inline_singleton_semantic_fixture_v1.rs"
    ));
    pub(super) fn ordinary() -> fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 {
        base::source(vec![base::subtract()])
    }
}

fn formal(kind: Instruction, ranked: bool) -> ProductionFormalMemoryOwnerV1 {
    formal_from(fixture::Fixture::new(kind).admit(), ranked)
}
fn formal_from(
    admitted: fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    ranked: bool,
) -> ProductionFormalMemoryOwnerV1 {
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let checked = if ranked {
        // This fixture has two scalar root arguments and no externally observed
        // memory effects. Its helper result is retained but dead at the root.
        let kernel = ProductionRankedKernelV1::new(
            "helper_value_source",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: u64::from_le_bytes([31; 8]),
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                }],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let lowered = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("helper_descriptor_fixture", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        let receipt =
            ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
                semantic,
                lowered,
                "inert no-memory helper descriptor control".into(),
                vec![],
            )
            .unwrap();
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
            1,
        )
        .unwrap()
    } else {
        ProductionSemanticKirOwnerV1::try_lower(semantic, ProductionSemanticKirLimitsV1::default())
            .unwrap()
    };
    ProductionFormalMemoryOwnerV1::try_admit(checked).unwrap()
}
fn target(formal: &ProductionFormalMemoryOwnerV1) -> RetainedProductionTargetV30 {
    RetainedProductionTargetV30::try_lower(
        formal,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap()
}
fn project(
    formal: &ProductionFormalMemoryOwnerV1,
    optimized: &RetainedProductionTargetV30,
) -> Result<Vec<CapabilityV1>, Error> {
    let module = optimized.module();
    let admission = verify_and_admit(formal, optimized, module, "gfx942:xnack-")?.unwrap();
    descriptor_capabilities_with_admission_v1(
        module,
        false,
        false,
        DescriptorCapabilityAdmissionV1::InlineHelpersV30(&admission),
    )
}
fn assembly_mut(module: &mut Module) -> &mut fe2o3_kernel_ir::InlineAssembly {
    module
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.body)
        .flat_map(|body| &mut body.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::InlineAssembly(assembly) => Some(assembly),
            _ => None,
        })
        .unwrap()
}

#[test]
fn all_six_replayed_helpers_project_runtime_requirements_without_changing_canonical_subject() {
    for kind in fixture::KINDS {
        let formal = formal(kind, true);
        let optimized = target(&formal);
        let module = optimized.module();
        let original = module.clone();
        let canonical = formal.semantic_kir().canonical_kernel_ir_bytes().to_vec();
        assert_eq!(
            project(&formal, &optimized).unwrap(),
            [CapabilityV1::AmdWave]
        );
        assert!(module.effective_capabilities().iter().any(exact_inline));
        assert_eq!(*module, original);
        assert_eq!(formal.semantic_kir().canonical_kernel_ir_bytes(), canonical);
        assert!(optimized.report().is_production_replay_compatible());
        formal.verify_equivalence().unwrap();
    }
}

#[test]
fn retained_fixed_optimizer_accepts_real_cfg_changes_not_preoptimization_body_equality() {
    let formal = formal(Instruction::VOrB32, true);
    let optimized = target(&formal);
    let before = formal
        .semantic_kir()
        .module()
        .functions
        .iter()
        .find(|function| contains_inline(function))
        .unwrap();
    let after = optimized
        .module()
        .functions
        .iter()
        .find(|function| contains_inline(function))
        .unwrap();
    assert_ne!(before.body, after.body);
    assert!(optimized.report().changed());
    assert_eq!(
        project(&formal, &optimized).unwrap(),
        [CapabilityV1::AmdWave]
    );
}

#[test]
fn ordinary_and_complete_body_only_projection_refuse_helper_structural_capability() {
    let formal = formal(Instruction::VOrB32, true);
    let optimized = target(&formal);
    let module = optimized.module();
    assert!(matches!(
        descriptor_capabilities(module, false, false),
        Err(Error::UnsupportedCapability(_))
    ));
    assert!(matches!(
        descriptor_capabilities_with_complete_body_v19(module, false, false, true),
        Err(Error::UnsupportedCapability(_))
    ));
}

#[test]
fn helper_admission_keeps_unknown_near_match_and_whole_body_extensions_fail_closed() {
    let formal = formal(Instruction::VOrB32, true);
    let optimized = target(&formal);
    let module = optimized.module();
    let admission = verify_and_admit(&formal, &optimized, module, "gfx942:xnack-")
        .unwrap()
        .unwrap();
    for (namespace, name) in [
        (
            "fe2o3.amdgpu.extra",
            AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME,
        ),
        (
            AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE,
            "authenticated-inline-assembly.gfx942.v2",
        ),
        (
            AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE,
            "authenticated-inline-assembly.gfx942.v1-extra",
        ),
        (
            fe2o3_kernel_ir::AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAMESPACE_V19,
            fe2o3_kernel_ir::AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAME_V19,
        ),
    ] {
        let capability = TargetCapability::Extension {
            namespace: namespace.into(),
            name: name.into(),
        };
        assert!(!admission.admits(module, &capability));
        assert!(
            dialect_amdgcn::project_descriptor_capability_v1(
                fe2o3_kernel_ir::TargetCapabilityRefV1::from_owned(&capability),
                false,
                false,
                true,
            )
            .is_none()
        );
    }
}

#[test]
fn helper_requires_retained_mandatory_checks_and_matching_actual_source_owner() {
    let unchecked = formal(Instruction::VOrB32, false);
    let optimized = target(&unchecked);
    assert!(matches!(
        verify_and_admit(&unchecked, &optimized, optimized.module(), "gfx942:xnack-"),
        Err(Error::ProductionDescriptorMismatch(
            "inline helper complete mandatory ranked check roster"
        ))
    ));
    let original = formal(Instruction::VOrB32, true);
    let foreign = formal(Instruction::VXorB32, true);
    let optimized = target(&original);
    assert!(matches!(
        verify_and_admit(&foreign, &optimized, optimized.module(), "gfx942:xnack-"),
        Err(Error::ProductionDescriptorMismatch(
            "inline helper retained fixed target/source custody"
        ))
    ));
}

#[test]
fn altered_inline_opcode_operands_effects_options_and_source_identity_refuse() {
    let formal = formal(Instruction::VOrB32, true);
    let optimized = target(&formal);
    for mutation in 0..6 {
        let mut module = optimized.module().clone();
        let assembly = assembly_mut(&mut module);
        match mutation {
            0 => assembly.mnemonic = "v_xor_b32".into(),
            1 => {
                assembly.operands[1].kind =
                    AssemblyOperandKind::Input(fe2o3_kernel_ir::ValueId(u32::MAX))
            }
            2 => {
                assembly.declared_effects.insert(AssemblyEffect::ReadGlobal);
            }
            3 => {
                assembly.options.insert(AssemblyOption::ReadOnly);
            }
            4 => assembly.source.function = [0x81; 32],
            5 => assembly.source.statement = [0x82; 32],
            _ => unreachable!(),
        }
        assert!(matches!(
            verify_and_admit(&formal, &optimized, &module, "gfx942:xnack-"),
            Err(Error::ProductionDescriptorMismatch(
                "inline helper retained fixed target/source custody"
            ))
        ));
    }
}

#[test]
fn helper_abi_removal_direct_root_and_launch_changes_refuse() {
    let formal = formal(Instruction::VOrB32, true);
    let optimized = target(&formal);
    for mutation in 0..4 {
        let mut module = optimized.module().clone();
        let helper = module
            .functions
            .iter_mut()
            .find(|function| contains_inline(function))
            .unwrap();
        match mutation {
            0 => helper.signature.results.clear(),
            1 => helper.body = None,
            2 => helper.role = FunctionRole::KernelEntry,
            3 => {
                module.kernels[0].workgroup_size =
                    Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1))
            }
            _ => unreachable!(),
        }
        assert!(verify_and_admit(&formal, &optimized, &module, "gfx942:xnack-").is_err());
    }
}

#[test]
fn exact_helper_profile_refuses_root_isa_non_source_op_and_hidden_effects() {
    let formal = formal(Instruction::VOrB32, true);
    let optimized = target(&formal);
    for mutation in 0..4 {
        let mut module = optimized.module().clone();
        let helper = module
            .functions
            .iter_mut()
            .find(|function| contains_inline(function))
            .unwrap();
        if mutation == 0 {
            helper.role = FunctionRole::KernelEntry;
            assert!(matches!(
                validate_helper_profile(helper),
                Err(Error::ProductionDescriptorMismatch(
                    "inline helper requires internal helper ownership"
                ))
            ));
        } else {
            let assembly = helper
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match &mut operation.kind {
                    OperationKind::InlineAssembly(assembly) => Some(assembly),
                    _ => None,
                })
                .unwrap();
            match mutation {
                1 => assembly.mnemonic = "s_mov_b32".into(),
                2 => {
                    assembly.declared_effects.insert(AssemblyEffect::ReadGlobal);
                }
                3 => {
                    assembly.options.insert(AssemblyOption::ReadOnly);
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_helper_profile(helper),
                Err(Error::ProductionDescriptorMismatch(
                    "inline helper exact six-u32 NoMemory profile"
                ))
            ));
        }
    }
}

#[test]
fn exact_target_wave_scopes_and_borrowed_target_module_are_required() {
    let formal = formal(Instruction::VOrB32, true);
    let optimized = target(&formal);
    let module = optimized.module();
    assert!(verify_and_admit(&formal, &optimized, module, "gfx950:xnack-").is_err());
    for mutation in 0..3 {
        let mut altered = module.clone();
        match mutation {
            0 => {
                altered
                    .required_capabilities
                    .remove(&TargetCapability::WaveWidth(WaveWidth::Wave64));
            }
            1 => {
                altered.kernels[0]
                    .required_capabilities
                    .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));
            }
            2 => {
                altered
                    .required_capabilities
                    .insert(fe2o3_kernel_ir::gfx950_xnack_minus_target_capability());
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module_join(formal.semantic_kir().module(), &altered, "gfx942:xnack-")
                .is_err()
        );
        assert!(verify_and_admit(&formal, &optimized, &altered, "gfx942:xnack-").is_err());
    }
    let admission = verify_and_admit(&formal, &optimized, module, "gfx942:xnack-")
        .unwrap()
        .unwrap();
    let clone = module.clone();
    assert!(!optimized.joins(&formal, &clone, "gfx942:xnack-"));
    assert!(matches!(
        descriptor_capabilities_with_admission_v1(
            &clone,
            false,
            false,
            DescriptorCapabilityAdmissionV1::InlineHelpersV30(&admission),
        ),
        Err(Error::UnsupportedCapability(_))
    ));
}

#[test]
fn ordinary_no_assembly_owner_keeps_existing_runtime_capability_projection() {
    let formal = formal_from(fixture::ordinary(), false);
    let optimized = target(&formal);
    let module = optimized.module();
    assert!(!needs_inline(module));
    assert!(
        verify_and_admit(&formal, &optimized, module, "gfx942:xnack-")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        descriptor_capabilities(module, false, false).unwrap(),
        descriptor_capabilities_with_admission_v1(
            module,
            false,
            false,
            DescriptorCapabilityAdmissionV1::Ordinary
        )
        .unwrap(),
    );
}
