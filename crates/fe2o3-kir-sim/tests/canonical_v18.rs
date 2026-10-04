//! Synthetic canonical-owner controls, not source or GPU execution evidence.
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    Constant, Function, FunctionRole, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, StorageFieldV1,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1,
    StorageOperationV1, StorageVariantEncodingV1, StorageVariantV1, Terminator, Type, ValueDef,
    ValueId, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_kir_sim::*;
#[path = "canonical_v18/execution_lifecycle.rs"]
mod execution_lifecycle;
#[path = "canonical_v18/generic_exposure.rs"]
mod generic_exposure;
#[path = "canonical_v18/limits.rs"]
mod limits;
#[path = "canonical_v18/scalar_storage.rs"]
mod scalar_storage;

const BOUND: usize = 16_000_000;
const FLOOR: usize = 73;
const LAYOUT_LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

fn field(offset: u64, row: u32) -> StorageFieldV1 {
    StorageFieldV1 {
        offset,
        layout: StorageLayoutIdV1(row),
    }
}

fn table() -> Vec<StorageLayoutV1> {
    vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(vec![field(0, 0)].into_boxed_slice()),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Union(vec![field(0, 0), field(0, 1)].into_boxed_slice()),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Direct { tag: field(0, 0) },
                variants: [37, 42]
                    .map(|tag| StorageVariantV1 {
                        discriminant: tag,
                        direct_tag_bits: Some(tag),
                        uninhabited: false,
                        layout: StorageLayoutIdV1(1),
                    })
                    .into(),
            },
        },
    ]
}

fn module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![pointer, scalar], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = entry.derived_capabilities();
    let mut kernel = Kernel::new(
        "scalar",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities = entry.required_capabilities.clone();
    let mut module = Module::new("sim-tests::canonical-v18");
    module.required_capabilities = entry.required_capabilities.clone();
    module.storage_layouts = table();
    module.functions.push(entry);
    let empty = || {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        block
    };
    module.functions.push(Function::definition(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![empty()],
    ));
    module.functions.push(Function::device_ffi_export(
        "export",
        Signature::new(vec![], vec![]),
        vec![],
        vec![empty()],
    ));
    module.functions.push(Function::external_import(
        "external",
        Signature::new(vec![], vec![]),
    ));
    module.kernels.push(kernel);
    module
}

fn owner(module: &Module, budget: &mut Budget<'_>) -> (Owner, usize) {
    let floor = budget.storage();
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUT_LIMITS, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    let paid = receipt.retained_storage();
    budget.reserve_storage(paid).unwrap();
    (owner, paid)
}

fn view(owner: &Owner, budget: &mut Budget<'_>) -> (AdmittedSimulationModuleV1, usize) {
    let floor = budget.storage();
    let (view, receipt) = AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
        owner,
        SimulationLimitsV1::default(),
        budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    let paid = receipt.retained_storage();
    budget.reserve_storage(paid).unwrap();
    (view, paid)
}

fn request(target: SimulationTargetV1) -> SimulationRequestV1 {
    let length = 64 * 4 + 8;
    SimulationRequestV1::new(
        "scalar",
        [64, 1, 1],
        [32, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    vec![0x5a; length],
                    vec![false; length],
                    target,
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(37)),
        ],
    )
}

fn assert_output(execution: &SimulationExecutionV1) {
    let output = execution.buffer(0).unwrap();
    assert!(
        output.bytes()[..256]
            .chunks_exact(4)
            .all(|word| word == 37_u32.to_le_bytes())
    );
    assert_eq!(&output.bytes()[256..], &[0x5a; 8]);
    assert!(
        output.initialized()[..256]
            .iter()
            .all(|initialized| *initialized)
    );
    assert_eq!(&output.initialized()[256..], &[false; 8]);
    assert!(!execution.grants_execution_authority());
}

fn context_mismatch(result: Result<SimulationExecutionV1, SimulationErrorV1>) {
    assert!(matches!(
        result,
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::ScheduleReplay(
                SimulationScheduleReplayErrorV1::ContextMismatch
            ),
            ..
        }))
    ));
}

#[test]
fn exact_v18_table_roles_and_scalar_execution_keep_same_owner_identity() {
    let raw = module();
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (canonical, owner_paid) = owner(&raw, &mut budget);
    let (admitted, view_paid) = view(&canonical, &mut budget);
    assert_eq!(admitted.module(), canonical.module());
    assert_eq!(canonical.module(), &raw);
    assert_ne!(
        admitted.module().storage_layouts.as_ptr(),
        canonical.module().storage_layouts.as_ptr()
    );
    assert_eq!(admitted.identity().wire_version(), 18);
    assert_eq!(admitted.identity().digest(), canonical.identity().digest());
    assert_eq!(
        admitted.identity().canonical_length(),
        canonical.identity().canonical_length()
    );
    assert_eq!(view_paid, admitted.admitted_resident_bytes());
    assert_eq!(
        admitted
            .module()
            .functions
            .iter()
            .map(|f| f.role)
            .collect::<Vec<_>>(),
        [
            FunctionRole::KernelEntry,
            FunctionRole::InternalHelper,
            FunctionRole::DeviceFfiExport,
            FunctionRole::ExternalImport
        ]
    );
    assert!(!admitted.grants_execution_authority());
    for target in ["gfx942:xnack-", "gfx950:xnack-"]
        .map(|target| SimulationTargetV1::amdgpu_from_device_target(target).unwrap())
    {
        let request = request(target);
        let original = request.clone();
        assert_output(
            &admitted
                .simulate(&request, target, SimulationLimitsV1::default())
                .unwrap(),
        );
        for schedule in [
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 256 },
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 17,
                max_decisions: 256,
            },
        ] {
            let executed = admitted
                .simulate_scheduled(&request, target, SimulationLimitsV1::default(), schedule)
                .unwrap();
            assert_output(&executed);
            let record = executed.schedule_record().unwrap();
            let replay = admitted
                .simulate_scheduled(
                    &request,
                    target,
                    SimulationLimitsV1::default(),
                    SimulationScheduleRequestV1::Replay(record),
                )
                .unwrap();
            assert_output(&replay);
            assert_eq!(
                replay.schedule_transcript_identity(),
                executed.schedule_transcript_identity()
            );
            for other in [
                SimulationTargetV1::amdgpu_64(),
                SimulationTargetV1::amdgpu_from_device_target("gfx942:xnack-").unwrap(),
                SimulationTargetV1::amdgpu_from_device_target("gfx950:xnack-").unwrap(),
            ]
            .into_iter()
            .filter(|other| *other != target)
            {
                context_mismatch(admitted.simulate_scheduled(
                    &request,
                    other,
                    SimulationLimitsV1::default(),
                    SimulationScheduleRequestV1::Replay(record),
                ));
            }
        }
        assert_eq!(request, original);
    }
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(admitted);
    budget.release_storage(view_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR + owner_paid);
    drop(canonical);
    budget.release_storage(owner_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn table_only_change_is_a_different_replay_context() {
    let mut work = Work::new(BOUND);
    let mut budget = Budget::new(&mut work, BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let raw = module();
    let (canonical, owner_paid) = owner(&raw, &mut budget);
    let (admitted, view_paid) = view(&canonical, &mut budget);
    let mut changed = raw.clone();
    changed.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I32);
    assert_eq!(raw.functions, changed.functions);
    let (other, other_paid) = owner(&changed, &mut budget);
    let (other_view, other_view_paid) = view(&other, &mut budget);
    assert_ne!(admitted.identity(), other_view.identity());
    let target = SimulationTargetV1::amdgpu_from_device_target("gfx942:xnack-").unwrap();
    let request = request(target);
    let execution = admitted
        .simulate_scheduled(
            &request,
            target,
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 37,
                max_decisions: 256,
            },
        )
        .unwrap();
    assert_output(
        &other_view
            .simulate(&request, target, SimulationLimitsV1::default())
            .unwrap(),
    );
    context_mismatch(other_view.simulate_scheduled(
        &request,
        target,
        SimulationLimitsV1::default(),
        SimulationScheduleRequestV1::Replay(execution.schedule_record().unwrap()),
    ));
    drop(other_view);
    budget.release_storage(other_view_paid).unwrap();
    drop(other);
    budget.release_storage(other_paid).unwrap();
    drop(admitted);
    budget.release_storage(view_paid).unwrap();
    drop(canonical);
    budget.release_storage(owner_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn aggregate_storage_and_projection_remain_explicitly_unsupported() {
    for with_operations in [false, true] {
        let mut raw = module();
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(0),
                Type::pointer(
                    Type::StorageObject(StorageLayoutIdV1(1)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(1)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ));
        if with_operations {
            block.operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
                    OperationKind::Constant(Constant::U32(37)),
                ),
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(2),
                        Type::pointer(
                            Type::StorageObject(StorageLayoutIdV1(0)),
                            AddressSpace::Private,
                            AccessMode::ReadWrite,
                        ),
                    ),
                    OperationKind::Storage(StorageOperationV1::Project {
                        base: ValueId(0),
                        step: fe2o3_kernel_ir::StorageProjectionV1::Field(0),
                    }),
                ),
            ]);
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut entry =
            Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
        entry.required_capabilities = entry.derived_capabilities();
        raw.required_capabilities = entry.required_capabilities.clone();
        raw.kernels[0].required_capabilities = entry.required_capabilities.clone();
        raw.functions[0] = entry;
        let mut work = Work::new(BOUND);
        let mut budget = Budget::new(&mut work, BOUND);
        budget.reserve_storage(FLOOR).unwrap();
        let (canonical, owner_paid) = owner(&raw, &mut budget);
        let (admitted, view_paid) = view(&canonical, &mut budget);
        let request = SimulationRequestV1::new("scalar", [1, 1, 1], [1, 1, 1], vec![]);
        let error = admitted
            .preflight(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = error else {
            panic!("storage must have an explicit unsupported finding: {error:?}");
        };
        assert!(
            report
                .findings()
                .iter()
                .any(|site| site.operation == Some(0)
                    && site.feature == UnsupportedFeatureV1::InertStorage)
        );
        if with_operations {
            assert!(
                report
                    .findings()
                    .iter()
                    .any(|site| site.operation == Some(2)
                        && site.feature == UnsupportedFeatureV1::InertStorage)
            );
        }
        drop(admitted);
        budget.release_storage(view_paid).unwrap();
        drop(canonical);
        budget.release_storage(owner_paid).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
