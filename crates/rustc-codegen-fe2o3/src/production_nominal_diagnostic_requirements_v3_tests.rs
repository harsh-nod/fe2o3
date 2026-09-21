//! Requirement-classifier components, not actual rustc/native qualification.
use super::*;
use fe2o3_kernel_ir::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE, AddressSpace, AmdGpuDiagnosticOperation,
    Atomic, AtomicKind, Barrier, BarrierSemantics, BasicBlock, BlockId, Convergence, Fence,
    Function, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, MemoryOrdering, Signature,
    SynchronizationScope, Terminator, ValueId, WorkgroupBarrier,
};

const PROFILES: [ProductionAmdTargetProfileV1; 2] = [
    ProductionAmdTargetProfileV1::Gfx942,
    ProductionAmdTargetProfileV1::Gfx950,
];

fn extension(namespace: &str, name: &str) -> TargetCapability {
    TargetCapability::Extension {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
    }
}

fn fixture() -> Module {
    let mut module = Module::new("nominal-requirements");
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block.clone()],
    ));
    module.functions.push(Function::internal_helper(
        "unreachable",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

fn at_site(module: &mut Module, site: usize, capability: TargetCapability) {
    match site {
        0 => &mut module.required_capabilities,
        1 => &mut module.kernels[0].required_capabilities,
        2 => &mut module.functions[0].required_capabilities,
        3 => &mut module.functions[1].required_capabilities,
        _ => panic!("exact declared capability site"),
    }
    .insert(capability);
}

fn classify(module: &Module, profile: ProductionAmdTargetProfileV1) -> R<[CapabilityV1; 1]> {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(53).unwrap();
    let result = scoped(&mut budget, |budget| {
        inert_capabilities(module, profile, budget)
    });
    assert_eq!((budget.storage(), budget.peak_storage()), (53, 53));
    result
}

#[test]
fn nominal_diagnostic_requirements_both_planned_profiles_preserve_neutral_and_bound_modules() {
    for profile in PROFILES {
        for site in 0..4 {
            let mut neutral = fixture();
            at_site(
                &mut neutral,
                site,
                extension(
                    AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
                ),
            );
            let before = neutral.clone();
            assert_eq!(
                classify(&neutral, profile).unwrap(),
                [CapabilityV1::AmdWave]
            );
            assert_eq!(
                neutral, before,
                "planned-profile check must not bind source N"
            );
            let bound = dialect_amdgcn::bind_production_target_v1(&neutral, profile).unwrap();
            let bound_before = bound.module().clone();
            assert_eq!(
                classify(bound.module(), profile).unwrap(),
                [CapabilityV1::AmdWave]
            );
            assert_eq!(bound.module(), &bound_before);
            assert_eq!(neutral, before);
            assert!(matches!(
                classify(
                    bound.module(),
                    if profile == ProductionAmdTargetProfileV1::Gfx942 {
                        ProductionAmdTargetProfileV1::Gfx950
                    } else {
                        ProductionAmdTargetProfileV1::Gfx942
                    }
                ),
                Err(E::UnsupportedRequirements)
            ));
        }
    }
}

#[test]
fn nominal_diagnostic_requirements_operation_is_seen_without_any_declared_permission() {
    for profile in PROFILES {
        let mut module = fixture();
        // Deliberately no declaration markers: this component checks the actual
        // operation visitor, not validity of a source or complete call graph.
        module.functions[1].body.as_mut().unwrap().blocks[0]
            .operations
            .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        assert!(module.required_capabilities.is_empty());
        assert!(module.kernels[0].required_capabilities.is_empty());
        assert!(
            module
                .functions
                .iter()
                .all(|f| f.required_capabilities.is_empty())
        );
        let before = module.clone();
        assert_eq!(classify(&module, profile).unwrap(), [CapabilityV1::AmdWave]);
        assert_eq!(module, before);
    }
}

#[test]
fn nominal_diagnostic_requirements_every_conflicting_target_declaration_is_refused() {
    for profile in PROFILES {
        let other = if profile == ProductionAmdTargetProfileV1::Gfx942 {
            ProductionAmdTargetProfileV1::Gfx950
        } else {
            ProductionAmdTargetProfileV1::Gfx942
        };
        for site in 0..4 {
            let mut module = fixture();
            module.required_capabilities.insert(extension(
                AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
            ));
            module.required_capabilities.insert(extension(
                AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                profile.device_target(),
            ));
            assert_eq!(classify(&module, profile).unwrap(), [CapabilityV1::AmdWave]);
            at_site(
                &mut module,
                site,
                extension(
                    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                    other.device_target(),
                ),
            );
            assert!(matches!(
                classify(&module, profile),
                Err(E::UnsupportedRequirements)
            ));
        }
    }
}

#[test]
fn nominal_diagnostic_requirements_legacy_gfx942_is_not_a_gfx950_permission() {
    for site in 0..4 {
        let mut module = fixture();
        at_site(
            &mut module,
            site,
            extension(
                AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
            ),
        );
        assert_eq!(
            classify(&module, ProductionAmdTargetProfileV1::Gfx942).unwrap(),
            [CapabilityV1::AmdWave]
        );
        assert!(matches!(
            classify(&module, ProductionAmdTargetProfileV1::Gfx950),
            Err(E::UnsupportedRequirements)
        ));
    }
}

#[test]
fn nominal_diagnostic_requirements_unknown_names_cannot_use_a_valid_target_marker() {
    for profile in PROFILES {
        for (namespace, name) in [
            (
                AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                "diagnostics.amdgcn.v3",
            ),
            (
                AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                "diagnostics.gfx942.v1.extra",
            ),
            ("fe2o3.amdgpu.extra", AMDGPU_DIAGNOSTICS_CAPABILITY_NAME),
            (AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, "gfx951:xnack-"),
        ] {
            for site in 0..4 {
                let mut module = fixture();
                module.required_capabilities.insert(extension(
                    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                    profile.device_target(),
                ));
                at_site(&mut module, site, extension(namespace, name));
                assert!(matches!(
                    classify(&module, profile),
                    Err(E::UnsupportedRequirements)
                ));
            }
        }
    }
}

#[test]
fn nominal_diagnostic_requirements_do_not_admit_other_target_sensitive_capabilities() {
    for profile in PROFILES {
        for capability in [
            TargetCapability::Subgroups,
            TargetCapability::SubgroupSize(64),
            TargetCapability::WorkgroupMemory,
            TargetCapability::WorkgroupBarrier,
            TargetCapability::DynamicWorkgroupMemory,
            extension(
                fe2o3_kernel_ir::MATRIX_CAPABILITY_NAMESPACE,
                fe2o3_kernel_ir::BF16_F32_M16N16K16_CAPABILITY,
            ),
            TargetCapability::Atomic {
                width_bits: 32,
                address_space: AddressSpace::Global,
                max_scope: SynchronizationScope::Device,
            },
        ] {
            for site in 0..4 {
                let mut module = fixture();
                module.required_capabilities.insert(extension(
                    AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
                ));
                module.required_capabilities.insert(extension(
                    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                    profile.device_target(),
                ));
                at_site(&mut module, site, capability.clone());
                assert!(matches!(
                    classify(&module, profile),
                    Err(E::UnsupportedRequirements)
                ));
            }
        }
    }
}

#[test]
fn nominal_diagnostic_requirements_atomic_and_synchronization_operations_still_refuse() {
    for profile in PROFILES {
        let semantics =
            BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]);
        for kind in [
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Load,
                pointer: ValueId(0),
                value: None,
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Acquire,
                failure_ordering: None,
            }),
            OperationKind::Barrier(Barrier {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Workgroup,
                semantics: semantics.clone(),
            }),
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: semantics.clone(),
            }),
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics,
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ] {
            let mut module = fixture();
            module.required_capabilities.insert(extension(
                AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
            ));
            module.functions[1].body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::new(vec![], kind));
            assert!(matches!(
                classify(&module, profile),
                Err(E::UnsupportedRequirements)
            ));
        }
    }
}

#[test]
fn nominal_diagnostic_requirements_extension_comparisons_are_byte_paid_before_use() {
    for profile in PROFILES {
        for name in [
            AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
            "x".repeat(4096),
        ] {
            let namespace = AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE;
            let mut module = Module::new("paid-requirement");
            module
                .required_capabilities
                .insert(extension(namespace, &name));
            let scan_work =
                1 + 20 * (namespace.len() + name.len() + profile.device_target().len() + 1);
            let expected = 17 + scan_work;
            for limit in [expected, expected - 1, 17] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 53);
                budget.charge_work(17).unwrap();
                budget.reserve_storage(53).unwrap();
                let result = scoped(&mut budget, |budget| {
                    inert_capabilities(&module, profile, budget)
                });
                if limit == expected {
                    if name == AMDGPU_DIAGNOSTICS_CAPABILITY_NAME {
                        assert_eq!(result.unwrap(), [CapabilityV1::AmdWave]);
                    } else {
                        assert!(matches!(result, Err(E::UnsupportedRequirements)));
                    }
                    assert_eq!(budget.work(), expected);
                } else {
                    let Err(E::Resource(Resource::Work(error))) = result else {
                        panic!("typed prepaid requirement-work refusal")
                    };
                    assert_eq!(error.limit(), limit);
                    assert_eq!(error.actual(), if limit == 17 { 18 } else { expected });
                    assert_eq!(budget.work(), if limit == 17 { 17 } else { 18 });
                }
                assert_eq!(
                    (
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (53, 53, None)
                );
                drop(budget);
                assert_eq!(
                    work.failed_work(),
                    if limit == expected {
                        None
                    } else {
                        Some(if limit == 17 { 18 } else { expected })
                    }
                );
            }
        }
    }
}
