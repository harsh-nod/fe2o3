use super::*;
use fe2o3_amd_target::AmdTargetId;
use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion, CompilerIdentityV1,
    DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1,
    KernelDescriptorV1, KernelId, LaunchConstraintsV1, ProducerIdentityV1, Text, ValidName,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Atomic, AtomicKind, BasicBlock, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, MemoryOrdering, Module, Operation, ScalarType, Signature, SynchronizationScope,
    TargetCapability, TargetCapabilityNameRefV1 as Name, Terminator, Type, ValueDef, ValueId,
};

const PRIOR: usize = 11;
const SIBLING: usize = 29;
const LARGE: usize = 50_000_000;

fn extension<'a>(namespace: &'a str, name: &'a str) -> Cap<'a> {
    Cap::Extension {
        namespace,
        name: Name::Text(name),
    }
}
fn owned_extension(namespace: &str, name: &str) -> TargetCapability {
    TargetCapability::Extension {
        namespace: namespace.into(),
        name: name.into(),
    }
}

#[test]
fn descriptor_capability_mapping_all_closed_arms_and_flag_combinations() {
    let cases = [
        (Cap::Int64, vec![]),
        (Cap::BFloat16, vec![]),
        (
            Cap::Subgroups,
            vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave],
        ),
        (
            Cap::SubgroupSize(64),
            vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave],
        ),
        (
            Cap::WaveWidth(WaveWidth::Wave64),
            vec![CapabilityV1::AmdWave],
        ),
        (
            Cap::Atomic {
                width_bits: 32,
                address_space: AddressSpace::Global,
                max_scope: SynchronizationScope::Device,
            },
            vec![CapabilityV1::Atomics],
        ),
        (
            extension(
                AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
            ),
            vec![],
        ),
        (
            extension(
                AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME,
            ),
            vec![],
        ),
        (
            extension(
                AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
            ),
            vec![],
        ),
    ];
    for matrix in [false, true] {
        for workgroup in [false, true] {
            for target in [false, true] {
                for (cap, expected) in &cases {
                    assert_eq!(
                        project_descriptor_capability_v1(*cap, matrix, workgroup, target)
                            .unwrap()
                            .iter()
                            .collect::<Vec<_>>(),
                        *expected
                    );
                }
                for cap in [
                    Cap::WorkgroupMemory,
                    Cap::WorkgroupBarrier,
                    extension(MATRIX_CAPABILITY_NAMESPACE, LDS_TILE_16X16_XOR4_CAPABILITY),
                ] {
                    assert_eq!(
                        project_descriptor_capability_v1(cap, matrix, workgroup, target)
                            .map(|set| set.iter().collect::<Vec<_>>()),
                        workgroup.then_some(vec![CapabilityV1::WorkgroupMemory])
                    );
                }
                for name in [
                    BF16_F32_M16N16K16_CAPABILITY,
                    SCALED_FP4_E2M1_F32_M16N16K128_CAPABILITY,
                    SCALED_FP8_E4M3_F32_M16N16K128_CAPABILITY,
                    SCALED_FP4_E2M1_FP8_E4M3_F32_M16N16K128_CAPABILITY,
                ] {
                    assert_eq!(
                        project_descriptor_capability_v1(
                            extension(MATRIX_CAPABILITY_NAMESPACE, name),
                            matrix,
                            workgroup,
                            target
                        )
                        .map(|set| set.iter().collect::<Vec<_>>()),
                        matrix.then_some(vec![CapabilityV1::MatrixMultiply, CapabilityV1::AmdMfma])
                    );
                }
                assert_eq!(
                    project_descriptor_capability_v1(
                        extension(
                            AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                            AMDGPU_DIAGNOSTICS_CAPABILITY_NAME
                        ),
                        matrix,
                        workgroup,
                        target
                    )
                    .is_some(),
                    target
                );
            }
        }
    }
    assert!(TAGS.windows(2).all(|rows| rows[0] < rows[1]));
}

#[test]
fn descriptor_capability_mapping_rejects_unknowns_even_with_every_allowance() {
    for cap in [
        Cap::Float16,
        Cap::Float64,
        Cap::SubgroupSize(32),
        Cap::SubgroupSize(0),
        Cap::WaveWidth(WaveWidth::Wave32),
        Cap::DynamicWorkgroupMemory,
        extension("unknown", BF16_F32_M16N16K16_CAPABILITY),
        extension(MATRIX_CAPABILITY_NAMESPACE, "unknown"),
        extension(AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, "gfx942"),
        extension(AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE, "unknown"),
    ] {
        assert_eq!(
            project_descriptor_capability_v1(cap, true, true, true),
            None
        );
    }
}

fn entry(name: &str) -> Function {
    let mut block = BasicBlock::new(BlockId(7));
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::kernel_entry(name, Signature::new(vec![], vec![]), vec![], vec![block])
}
fn fixture() -> Module {
    let mut module = Module::new("descriptor-capability-projection");
    module.functions.push(entry("entry"));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}
fn helper(name: &str, calls: &[&str]) -> Function {
    let mut block = BasicBlock::new(BlockId(9));
    block.operations = calls
        .iter()
        .map(|callee| {
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: (*callee).into(),
                    arguments: vec![],
                },
            )
        })
        .collect();
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::internal_helper(name, Signature::new(vec![], vec![]), vec![], vec![block])
}
fn add_call(module: &mut Module, callee: &str) {
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: callee.into(),
                arguments: vec![],
            },
        ));
}
fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(LARGE);
    let mut budget = Budget::new(&mut work, LARGE);
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v12(module, &mut budget)
        .expect("actual connected canonical V12 fixture");
    (owner, receipt.retained_storage())
}
fn table(capabilities: &[Vec<CapabilityV1>]) -> DeviceDescriptorTableV1 {
    let name = |text: &str| ValidName::new(text).unwrap();
    let text = |text: &str| Text::new(text).unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([3; 32]),
        EvidenceDigest::from_sha256_bytes([4; 32]),
    );
    let kernels = capabilities
        .iter()
        .enumerate()
        .map(|(index, caps)| {
            let symbol = format!("entry_{index}");
            KernelDescriptorV1::new(
                KernelId::from_bytes([index as u8 + 1; 32]),
                name(&format!("kernel_{index}")),
                name(&symbol),
                name(&format!("{symbol}.kd")),
                evidence,
                evidence,
                caps.clone(),
                KernelAbiLayoutV1::new(0, 0, 8).unwrap(),
                LaunchConstraintsV1::new(
                    1,
                    BlockSizeV1::Any,
                    DimensionsV1::new(1, 1, 1).unwrap(),
                    64,
                    0,
                    0,
                )
                .unwrap(),
                vec![],
            )
            .unwrap()
        })
        .collect();
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("test"), text("1"), [5; 20]),
        ProducerIdentityV1::new(text("test"), text("1")),
        DeviceTargetV1::new(AmdTargetId::parse("gfx942:xnack-").unwrap()),
        vec![],
        vec![],
        kernels,
    )
    .unwrap()
}

#[derive(Debug, Eq, PartialEq)]
struct Observed {
    result: Result<(), E>,
    work: usize,
    peak: usize,
    failed: Option<usize>,
}
fn run(
    owner: &Owner,
    owner_storage: usize,
    descriptors: &DeviceDescriptorTableV1,
    work_limit: usize,
    storage_limit: usize,
) -> Observed {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(PRIOR).unwrap();
    budget.reserve_storage(owner_storage + SIBLING).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = check_canonical_v12_descriptor_capabilities_v1(owner, descriptors, &mut budget);
    assert_eq!(budget.storage(), owner_storage + SIBLING);
    assert!(budget.work_ledger_identity_v1() == ledger);
    Observed {
        result,
        work: budget.work(),
        peak: budget.peak_storage(),
        failed: budget.failed_storage(),
    }
}
fn check(module: &Module, capabilities: &[Vec<CapabilityV1>]) -> Result<(), E> {
    let (owner, storage) = admit(module);
    run(&owner, storage, &table(capabilities), LARGE, LARGE).result
}

#[test]
fn descriptor_capability_actual_owner_implicit_workgroup_operation_is_not_lost() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(
                ValueId(1),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Workgroup,
                alignment: 4,
            },
        ));
    assert!(module.required_capabilities.is_empty());
    assert!(module.functions[0].required_capabilities.is_empty());
    assert_eq!(
        check(&module, &[vec![CapabilityV1::WorkgroupMemory]]),
        Ok(())
    );
    assert!(matches!(
        check(&module, &[vec![]]),
        Err(E::CapabilityMismatch { descriptor: 0 })
    ));
}

#[test]
fn descriptor_capability_actual_atomic_uses_pointer_definition_without_explicit_declaration() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        let mut module = fixture();
        let ty = Type::Scalar(scalar);
        let function = &mut module.functions[0];
        function.signature.parameters.push(Type::pointer(
            ty.clone(),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ));
        let body = function.body.as_mut().unwrap();
        body.parameters.push(ValueId(42));
        body.blocks[0].operations.push(Operation::new(
            vec![ValueDef::new(ValueId(9), ty)],
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Load,
                pointer: ValueId(42),
                value: None,
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 8),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Acquire,
                failure_ordering: None,
            }),
        ));
        assert!(function.required_capabilities.is_empty());
        assert_eq!(check(&module, &[vec![CapabilityV1::Atomics]]), Ok(()));
    }
}

#[test]
fn descriptor_capability_reachable_allowances_exclude_kernel_only_and_unreachable_declarations() {
    for cap in [
        TargetCapability::WorkgroupMemory,
        owned_extension(MATRIX_CAPABILITY_NAMESPACE, BF16_F32_M16N16K16_CAPABILITY),
    ] {
        let expected = if cap == TargetCapability::WorkgroupMemory {
            vec![CapabilityV1::WorkgroupMemory]
        } else {
            vec![CapabilityV1::MatrixMultiply, CapabilityV1::AmdMfma]
        };
        let mut module = fixture();
        module.kernels[0].required_capabilities.insert(cap.clone());
        assert!(matches!(
            check(&module, &[expected.clone()]),
            Err(E::Unsupported(Site::Kernel(_)))
        ));
        module.kernels[0].required_capabilities.clear();
        let mut unused = helper("unused", &[]);
        unused.required_capabilities.insert(cap.clone());
        module.functions.push(unused);
        assert!(matches!(
            check(&module, &[expected.clone()]),
            Err(E::Unsupported(Site::Function(_)))
        ));
        add_call(&mut module, "unused");
        assert_eq!(check(&module, &[expected.clone()]), Ok(()));
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .clear();
        module.required_capabilities.insert(cap);
        assert_eq!(check(&module, &[expected]), Ok(()));
    }
}

#[test]
fn descriptor_capability_barrier_alone_never_enables_workgroup_allowance() {
    let mut module = fixture();
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::WorkgroupBarrier);
    assert!(matches!(
        check(&module, &[vec![CapabilityV1::WorkgroupMemory]]),
        Err(E::Unsupported(Site::Function(_)))
    ));
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    assert_eq!(
        check(&module, &[vec![CapabilityV1::WorkgroupMemory]]),
        Ok(())
    );
}

#[test]
fn descriptor_capability_union_roots_duplicate_calls_and_recursive_closure_terminate() {
    let mut module = fixture();
    module.functions.push(entry("second"));
    module.kernels.push(Kernel::new(
        "second_kernel",
        "second",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    let mut leaf = helper("leaf", &["leaf"]);
    leaf.required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.functions.push(leaf);
    module.functions[1].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: "leaf".into(),
                arguments: vec![],
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: "leaf".into(),
                arguments: vec![],
            },
        ),
    ];
    assert_eq!(
        check(
            &module,
            &[
                vec![CapabilityV1::WorkgroupMemory],
                vec![CapabilityV1::WorkgroupMemory]
            ]
        ),
        Ok(())
    );
    assert!(matches!(
        check(&module, &[vec![CapabilityV1::WorkgroupMemory], vec![]]),
        Err(E::CapabilityMismatch { descriptor: 1 })
    ));
    assert_eq!(
        check(&module, &[vec![CapabilityV1::WorkgroupMemory]]),
        Err(E::KernelRoster)
    );
}

#[test]
fn descriptor_capability_missing_call_is_refused_before_connected_owner_construction() {
    let mut module = fixture();
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::F32);
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(1));
    body.blocks[0].operations.push(Operation::new(
        vec![ValueDef::new(ValueId(2), Type::F32)],
        OperationKind::Call {
            callee: "__fe2o3_ir_float_v1_sqrt_f32".into(),
            arguments: vec![ValueId(1)],
        },
    ));
    let mut work = Work::new(LARGE);
    let mut budget = Budget::new(&mut work, LARGE);
    budget.reserve_storage(SIBLING).unwrap();
    let result = Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget);
    let Err(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors)) =
        result
    else {
        panic!("missing callee must not produce a connected owner");
    };
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == fe2o3_kernel_ir::DiagnosticCode::UnknownCallee)
    );
    assert_eq!(budget.storage(), SIBLING);
}

#[test]
fn descriptor_capability_whole_closure_not_only_reachable_operations_is_checked() {
    let mut module = fixture();
    let mut unused = helper("unused", &[]);
    unused
        .required_capabilities
        .insert(TargetCapability::Float16);
    module.functions.push(unused);
    assert!(matches!(
        check(&module, &[vec![]]),
        Err(E::Unsupported(Site::Function(_)))
    ));
    module.functions[1].required_capabilities.clear();
    module.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(
                ValueId(8),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Workgroup,
                alignment: 4,
            },
        ));
    assert!(matches!(
        check(&module, &[vec![CapabilityV1::WorkgroupMemory]]),
        Err(E::Unsupported(Site::Operation(_)))
    ));
}

#[test]
fn descriptor_capability_exact_target_uses_complete_closure_but_not_descriptor_claim() {
    let mut module = fixture();
    module.required_capabilities.insert(owned_extension(
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
    ));
    assert_eq!(check(&module, &[vec![]]), Err(E::Unsupported(Site::Module)));
    for target in [
        AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    ] {
        let mut bound = module.clone();
        let mut unused = helper("unused", &[]);
        unused.required_capabilities.insert(owned_extension(
            AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
            target,
        ));
        bound.functions.push(unused);
        assert_eq!(check(&bound, &[vec![]]), Ok(()));
    }
}

#[test]
fn descriptor_capability_exact_set_refuses_extra_missing_and_historical_graph_substitution() {
    let old = fixture();
    let mut actual = old.clone();
    actual.functions[0]
        .required_capabilities
        .insert(TargetCapability::Subgroups);
    assert_eq!(check(&old, &[vec![]]), Ok(()));
    assert_eq!(
        check(
            &actual,
            &[vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave]]
        ),
        Ok(())
    );
    for wrong in [
        vec![],
        vec![CapabilityV1::Subgroup],
        vec![CapabilityV1::AmdWave],
        vec![
            CapabilityV1::Subgroup,
            CapabilityV1::AmdWave,
            CapabilityV1::Atomics,
        ],
    ] {
        assert_eq!(
            check(&actual, &[wrong]),
            Err(E::CapabilityMismatch { descriptor: 0 })
        );
    }
    assert_eq!(
        check(&old, &[vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave]]),
        Err(E::CapabilityMismatch { descriptor: 0 })
    );
}

#[test]
fn descriptor_capability_resource_work_boundaries_preserve_cumulative_floor_and_ledger() {
    let (owner, storage) = admit(&fixture());
    let descriptors = table(&[vec![]]);
    let full = run(&owner, storage, &descriptors, LARGE, LARGE);
    assert_eq!(full.result, Ok(()));
    assert_eq!(
        run(&owner, storage, &descriptors, full.work, full.peak),
        full
    );
    let short = run(&owner, storage, &descriptors, full.work - 1, full.peak);
    let Err(E::Resource(Resource::Work(error))) = short.result else {
        panic!("exact final row work");
    };
    assert_eq!(error.actual(), full.work);
    assert_eq!(error.limit(), full.work - 1);
    assert_eq!(short.work, full.work - (TAGS.len() + 1));
    for limit in [PRIOR, PRIOR + 2] {
        let first = run(&owner, storage, &descriptors, limit, LARGE);
        let Err(E::Resource(Resource::Work(error))) = first.result else {
            panic!("entry work");
        };
        assert_eq!(error.actual(), PRIOR + 3);
        assert_eq!(error.limit(), limit);
        assert_eq!(first.work, PRIOR);
        assert_eq!(first.peak, storage + SIBLING);
    }
}

#[test]
fn descriptor_capability_storage_peak_observation_requires_strict_successor() {
    let (owner, storage) = admit(&fixture());
    let descriptors = table(&[vec![]]);
    let full = run(&owner, storage, &descriptors, LARGE, LARGE);
    assert_eq!(full.result, Ok(()));
    let floor = storage + SIBLING;
    let mut reference_work = Work::new(LARGE);
    let mut reference = Budget::new(&mut reference_work, LARGE);
    reference.charge_work(PRIOR + 3).unwrap();
    reference.reserve_storage(floor).unwrap();
    let ledger = reference.work_ledger_identity_v1();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut reference).unwrap();
    reference
        .reserve_storage(receipt.retained_storage())
        .unwrap();
    let functions = inventory.functions().len();
    let inventory_work = reference.work();

    // The first backing is live before the pending-queue request. Measure its
    // actual capacity independently, without calling the checker or backing().
    reference.reserve_storage(size_of::<Scratch>()).unwrap();
    let reached_request = functions.checked_mul(size_of::<u8>()).unwrap();
    reference.reserve_storage(reached_request).unwrap();
    let mut reached = Vec::<u8>::new();
    reached.try_reserve_exact(functions).unwrap();
    reference
        .reserve_storage(reached.capacity().checked_sub(reached_request).unwrap())
        .unwrap();
    let prior_peak = reference.peak_storage();
    let pending_request = functions.checked_mul(size_of::<usize>()).unwrap();
    let peak = reference.storage().checked_add(pending_request).unwrap();
    assert!(peak > prior_peak, "pending request is the new peak");
    // Roster check, reached-backing entry, reached initialization, pending entry.
    let accepted_work = inventory_work.checked_add(3 + functions).unwrap();
    drop(reached);
    drop(inventory);
    reference
        .release_storage(reference.storage() - floor)
        .unwrap();
    assert_eq!(reference.storage(), floor);
    assert!(reference.work_ledger_identity_v1() == ledger);
    assert_eq!(full.peak, peak);

    let short = run(&owner, storage, &descriptors, LARGE, full.peak - 1);
    let Err(E::Resource(Resource::Storage(denied))) = short.result else {
        panic!(
            "pending queue requested-backing refusal: {:?}",
            short.result
        );
    };
    assert_eq!(denied.actual(), peak);
    assert_eq!(denied.limit(), peak - 1);
    assert_eq!(short.failed, Some(denied.actual()));
    assert_eq!(short.work, accepted_work);
    assert_eq!(short.peak, prior_peak);
}

#[test]
fn descriptor_capability_requested_backing_denial_is_preallocation_and_exact() {
    let mut work = Work::new(100);
    let limit = SIBLING + 3 * size_of::<usize>() - 1;
    let mut budget = Budget::new(&mut work, limit);
    budget.reserve_storage(SIBLING).unwrap();
    budget.charge_work(PRIOR).unwrap();
    let result = scoped(&mut budget, |budget| {
        let _rows = backing::<usize>(3, budget)?;
        Ok(())
    });
    let Err(E::Resource(Resource::Storage(error))) = result else {
        panic!("backing reserve");
    };
    assert_eq!((error.actual(), error.limit()), (limit + 1, limit));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (PRIOR + 1, SIBLING, SIBLING)
    );
}

#[test]
fn descriptor_capability_scope_drops_before_refund_on_all_exits() {
    use std::{cell::Cell, rc::Rc};
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for exit in 0..3 {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(SIBLING).unwrap();
        budget.charge_work(PRIOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let dropped = Rc::new(Cell::new(false));
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            let _owned = Dropped(dropped.clone());
            budget.charge_work(7)?;
            match exit {
                0 => Ok(()),
                1 => Err(E::KernelRoster),
                _ => panic!("private fault"),
            }
        });
        assert_eq!(
            result,
            match exit {
                0 => Ok(()),
                1 => Err(E::KernelRoster),
                _ => Err(E::Panicked),
            }
        );
        assert!(dropped.get());
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (PRIOR + 7, SIBLING, SIBLING + 13)
        );
    }
}

#[test]
fn descriptor_capability_scope_foreign_ledger_never_refunds_either_owner() {
    for exit in 0..3 {
        let mut one = Work::new(100);
        let mut two = Work::new(100);
        let mut budget = Budget::new(&mut one, 100);
        let mut foreign = Budget::new(&mut two, 100);
        budget.reserve_storage(SIBLING).unwrap();
        foreign.reserve_storage(61).unwrap();
        let original = budget.work_ledger_identity_v1();
        let other = foreign.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            std::mem::swap(budget, &mut foreign);
            match exit {
                0 => Ok(()),
                1 => Err(E::KernelRoster),
                _ => panic!("foreign private fault"),
            }
        });
        assert_eq!(result, Err(E::Resource(Resource::Accounting)));
        assert!(budget.work_ledger_identity_v1() == other);
        assert!(foreign.work_ledger_identity_v1() == original);
        assert_eq!((budget.storage(), foreign.storage()), (61, SIBLING + 13));
        foreign.release_storage(13).unwrap();
        std::mem::swap(&mut budget, &mut foreign);
        budget.release_storage(SIBLING).unwrap();
        foreign.release_storage(61).unwrap();
    }
}

#[test]
fn descriptor_capability_scope_undercut_and_hostile_payload_preserve_cleanup_boundary() {
    struct Hostile;
    impl Drop for Hostile {
        fn drop(&mut self) {
            panic!("hostile payload drop");
        }
    }
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(SIBLING).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            std::panic::panic_any(Hostile)
        })
    }));
    assert!(unwind.is_err());
    assert_eq!(budget.storage(), SIBLING);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert_eq!(result, Err(E::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), SIBLING - 1);
}
