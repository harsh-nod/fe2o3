use super::*;
use crate::native_v12_text_descriptor_replay_v3::tests::{self as fixture, KINDS, PROFILES, S, W};
use fe2o3_kernel_descriptor::{
    AtomicRequirementsV2, DESCRIPTOR_TABLE_VIEW_STORAGE_V3, DecodeError, DescriptorWireErrorV3,
    KernelId, LdsRequirementsV2, SynchronizationRequirementsV2, ValidationError,
    decode_device_descriptor_table_v3,
};
use fe2o3_kernel_ir::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AmdGpuDiagnosticOperation, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, Signature, Terminator,
};

fn extension(namespace: &str, name: &str) -> TargetCapability {
    TargetCapability::Extension {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
    }
}
fn module(profile: Profile) -> fe2o3_kernel_ir::Module {
    let mut module = fixture::module();
    let mut block = BasicBlock::new(BlockId(19));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "unused",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.functions.push(Function::declaration(
        "unused_external",
        Signature::new(vec![], vec![]),
    ));
    crate::bind_production_target_v1(&module, profile)
        .unwrap()
        .module()
        .clone()
}
fn site(module: &mut fe2o3_kernel_ir::Module, site: usize) -> &mut BTreeSet<TargetCapability> {
    match site {
        0 => &mut module.required_capabilities,
        1 => &mut module.kernels[0].required_capabilities,
        2 => &mut module.functions[0].required_capabilities,
        3 => &mut module.functions[1].required_capabilities,
        4 => &mut module.functions[2].required_capabilities,
        _ => panic!("complete declared requirement site"),
    }
}
fn check(module: &fe2o3_kernel_ir::Module, profile: Profile, bytes: &[u8], accept: bool) {
    let (owner, owner_storage) = fixture::admit(module);
    let table = decode_device_descriptor_table_v3(bytes, &mut fixture::free).unwrap();
    let floor = owner_storage + bytes.len() + DESCRIPTOR_TABLE_VIEW_STORAGE_V3 + fixture::SIBLING;
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(fixture::PRIOR).unwrap();
    let result =
        check_canonical_v12_descriptor_requirements_v3(&owner, profile, &table, &mut budget);
    assert_eq!(result.is_ok(), accept);
    if let Ok(relation) = result {
        let retained = relation.retained_storage();
        assert_eq!(
            retained,
            size_of::<ReplayedDescriptorRequirementsV3<'_, '_, '_>>()
        );
        budget.reserve_storage(retained).unwrap();
        assert!(std::ptr::eq(relation.output(), &owner));
        assert!(std::ptr::eq(relation.descriptors(), &table));
        assert_eq!(relation.profile(), profile);
        assert!(!relation.grants_authority());
        drop(relation);
        budget.release_storage(retained).unwrap();
    }
    assert_eq!(budget.storage(), floor);
}

#[test]
fn descriptor_requirements_v3_generic_diagnostic_at_every_site_both_bound_profiles() {
    for profile in PROFILES {
        let bytes = fixture::wire(profile, &KINDS, "kernel", 64, 256);
        for ordinal in 0..5 {
            let mut input = module(profile);
            site(&mut input, ordinal).insert(extension(
                AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
            ));
            check(&input, profile, &bytes, true);
        }
        let mut input = module(profile);
        // The unused helper contains a real operation, not merely a marker.
        let block = &mut input.functions[1].body.as_mut().unwrap().blocks[0];
        block
            .operations
            .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        block.terminator = Some(Terminator::Unreachable);
        input
            .functions
            .push(AmdGpuDiagnosticOperation::Trap.declaration());
        check(&input, profile, &bytes, true);
    }
}

#[test]
fn descriptor_requirements_v3_missing_binding_and_every_conflicting_claim_refuse() {
    for profile in PROFILES {
        let bytes = fixture::wire(profile, &KINDS, "kernel", 64, 256);
        let other = if profile == Profile::Gfx942 {
            Profile::Gfx950
        } else {
            Profile::Gfx942
        };
        for ordinal in 0..5 {
            let mut input = module(profile);
            site(&mut input, ordinal).insert(extension(
                AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                other.device_target(),
            ));
            check(&input, profile, &bytes, false);
        }
        for ordinal in 0..3 {
            for target in [true, false] {
                let mut input = module(profile);
                site(&mut input,ordinal).retain(|cap| if target {
                    !matches!(cap, TargetCapability::Extension { namespace, .. } if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE)
                } else { !matches!(cap, TargetCapability::WaveWidth(_)) });
                check(&input, profile, &bytes, false);
            }
        }
    }
}

#[test]
fn descriptor_requirements_v3_unknown_and_legacy_diagnostics_are_not_blanket_permissions() {
    for profile in PROFILES {
        let bytes = fixture::wire(profile, &KINDS, "kernel", 64, 256);
        for ordinal in 0..5 {
            for capability in [
                extension(
                    AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                    "diagnostics.amdgcn.v3",
                ),
                extension("fe2o3.amdgpu.extra", AMDGPU_DIAGNOSTICS_CAPABILITY_NAME),
                extension(AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, "gfx951:xnack-"),
                extension(
                    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                    "diagnostics.gfx942.v1.extra",
                ),
            ] {
                let mut input = module(profile);
                site(&mut input, ordinal).insert(capability);
                check(&input, profile, &bytes, false);
            }
            let mut input = module(profile);
            site(&mut input, ordinal).insert(extension(
                AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
            ));
            check(&input, profile, &bytes, profile == Profile::Gfx942);
        }
    }
}

#[test]
fn descriptor_requirements_v3_full_unused_closure_refuses_deferred_features() {
    for profile in PROFILES {
        let bytes = fixture::wire(profile, &KINDS, "kernel", 64, 256);
        for ordinal in 0..5 {
            for capability in [
                TargetCapability::Subgroups,
                TargetCapability::SubgroupSize(64),
                TargetCapability::WorkgroupMemory,
                TargetCapability::DynamicWorkgroupMemory,
                TargetCapability::WorkgroupBarrier,
                TargetCapability::WaveWidth(WaveWidth::Wave32),
                extension(
                    fe2o3_kernel_ir::MATRIX_CAPABILITY_NAMESPACE,
                    fe2o3_kernel_ir::BF16_F32_M16N16K16_CAPABILITY,
                ),
                TargetCapability::Atomic {
                    width_bits: 32,
                    address_space: fe2o3_kernel_ir::AddressSpace::Global,
                    max_scope: fe2o3_kernel_ir::SynchronizationScope::Device,
                },
            ] {
                let mut input = module(profile);
                if capability == TargetCapability::DynamicWorkgroupMemory {
                    site(&mut input, ordinal).insert(TargetCapability::WorkgroupMemory);
                }
                if matches!(capability, TargetCapability::WaveWidth(_)) {
                    site(&mut input, ordinal)
                        .retain(|c| !matches!(c, TargetCapability::WaveWidth(_)));
                }
                site(&mut input, ordinal).insert(capability);
                check(&input, profile, &bytes, false);
            }
        }
        let mut input = module(profile);
        input.functions[1].body.as_mut().unwrap().blocks[0]
            .operations
            .push(fe2o3_kernel_ir::Operation::new(
                vec![],
                OperationKind::Fence(fe2o3_kernel_ir::Fence {
                    memory_scope: fe2o3_kernel_ir::SynchronizationScope::Device,
                    semantics: fe2o3_kernel_ir::BarrierSemantics::new(
                        fe2o3_kernel_ir::MemoryOrdering::AcquireRelease,
                        [fe2o3_kernel_ir::AddressSpace::Global],
                    ),
                }),
            ));
        check(&input, profile, &bytes, false);
    }
}

#[test]
fn descriptor_requirements_v3_launch_and_full_descriptor_requirements_are_checked() {
    let id = KernelId::from_bytes([13; 32]);
    for profile in PROFILES {
        let input = module(profile);
        check(
            &input,
            profile,
            &fixture::wire(profile, &KINDS, "kernel", 32, 256),
            false,
        );
        for (wave, cooperative, sync, atomic) in [
            (RequiredWavefrontWidthV2::Wave32, false, 0, 0),
            (RequiredWavefrontWidthV2::Wave64, true, 0, 0),
            (
                RequiredWavefrontWidthV2::Wave64,
                false,
                SynchronizationRequirementsV2::DEVICE_FENCE,
                0,
            ),
            (
                RequiredWavefrontWidthV2::Wave64,
                false,
                0,
                AtomicRequirementsV2::DEVICE_SCOPE,
            ),
        ] {
            let requirement = KernelTargetRequirementsV2::new(
                id,
                LdsRequirementsV2::new(0, 0).unwrap(),
                wave,
                cooperative,
                SynchronizationRequirementsV2::from_bits(sync).unwrap(),
                AtomicRequirementsV2::from_bits(atomic).unwrap(),
            );
            let capabilities: &[CapabilityV1] = if sync != 0 || atomic != 0 {
                &[CapabilityV1::Atomics, CapabilityV1::AmdWave]
            } else {
                &[CapabilityV1::AmdWave]
            };
            let encoded = fixture::try_wire_with(
                profile,
                &KINDS,
                "kernel",
                64,
                256,
                capabilities,
                requirement,
            );
            if wave == RequiredWavefrontWidthV2::Wave32 {
                assert!(matches!(
                    encoded,
                    Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
                        ValidationError::TargetMismatch {
                            field: "exact wavefront width"
                        }
                    )))
                ));
            } else {
                check(&input, profile, &encoded.unwrap(), false);
            }
        }
        let requirement = KernelTargetRequirementsV2::new(
            id,
            LdsRequirementsV2::new(0, 0).unwrap(),
            RequiredWavefrontWidthV2::Wave64,
            false,
            SynchronizationRequirementsV2::empty(),
            AtomicRequirementsV2::empty(),
        );
        for (static_bytes, dynamic_bytes) in [(8, 0), (0, 8), (8, 8)] {
            let requirement = KernelTargetRequirementsV2::new(
                id,
                LdsRequirementsV2::new(static_bytes, dynamic_bytes).unwrap(),
                RequiredWavefrontWidthV2::Wave64,
                false,
                SynchronizationRequirementsV2::empty(),
                AtomicRequirementsV2::empty(),
            );
            let bytes = fixture::wire_with(
                profile,
                &KINDS,
                "kernel",
                64,
                256,
                &[CapabilityV1::WorkgroupMemory, CapabilityV1::AmdWave],
                requirement,
            );
            check(&input, profile, &bytes, false);
        }
        for capabilities in [
            vec![],
            vec![CapabilityV1::WorkgroupMemory],
            vec![CapabilityV1::WorkgroupMemory, CapabilityV1::AmdWave],
        ] {
            let encoded = fixture::try_wire_with(
                profile,
                &KINDS,
                "kernel",
                64,
                256,
                &capabilities,
                requirement,
            );
            if capabilities.contains(&CapabilityV1::AmdWave) {
                check(&input, profile, &encoded.unwrap(), false);
            } else {
                assert!(matches!(
                    encoded,
                    Err(DescriptorWireErrorV3::Decode(DecodeError::Validation(
                        ValidationError::InvalidValue {
                            field: "exact wavefront width requires the AMD wave capability"
                        }
                    )))
                ));
            }
        }
    }
}
