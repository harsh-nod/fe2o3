use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrReplayAdmissionErrorV15, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrWorkBudgetV1, Constant, DiagnosticCode, ExecutionOperationV15 as Execution,
    ExecutionRoleV15 as Role, Function, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module,
    Operation, OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type, ValueDef,
    ValueId, VerifiedCanonicalKernelIrModuleV12, VerifiedCanonicalKernelIrModuleV15,
    VerifiedCanonicalKernelIrV12, WorkgroupSize,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

const WORK_PREFIX: usize = 11;
const STORAGE_PREFIX: usize = 7;

fn execution(result: Option<(u32, Role)>, kind: Execution) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, role)| ValueDef::new(ValueId(id), Type::Execution(role)))
            .collect(),
        OperationKind::Execution(kind),
    )
}

fn context() -> Operation {
    execution(Some((10, Role::Context)), Execution::ContextIssue)
}

fn derive(id: u32) -> Operation {
    execution(
        Some((id, Role::Workgroup)),
        Execution::WorkgroupDerive {
            context: ValueId(10),
        },
    )
}

fn end(id: u32, discarded: &[u32]) -> Operation {
    execution(
        None,
        Execution::ScopeEnd {
            workgroup: ValueId(id),
            discarded: discarded.iter().copied().map(ValueId).collect(),
        },
    )
}

fn scalar(id: u32, kind: OperationKind) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        kind,
    )
}

fn constant(id: u32, value: u32) -> Operation {
    scalar(id, OperationKind::Constant(Constant::U32(value)))
}

// Same sparse IDs, physical parameters and scope shape as kernel-ir's V15
// lifecycle fixture; its private test module is unavailable across crates.
fn module(operations: Vec<Operation>) -> Module {
    let mut module = Module::new("execution-discharge");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![BasicBlock {
            id: BlockId(7),
            parameters: vec![],
            operations,
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn sequential() -> Module {
    module(vec![
        constant(20, 3),
        context(),
        derive(11),
        constant(21, 5),
        end(11, &[]),
        derive(100),
        constant(22, 7),
        end(100, &[]),
        constant(23, 9),
    ])
}

fn diamond(condition: bool) -> Module {
    let mut module = module(vec![
        context(),
        derive(11),
        Operation::effect_free(
            ValueDef::new(ValueId(50), Type::BOOL),
            OperationKind::Constant(Constant::Bool(condition)),
        ),
    ]);
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(50),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    for id in [30, 20] {
        blocks.push(BasicBlock {
            id: BlockId(id),
            parameters: vec![],
            operations: vec![constant(id, id), end(11, &[])],
            terminator: Some(Terminator::Branch {
                target: BlockId(40),
                arguments: vec![ValueId(id)],
            }),
        });
    }
    blocks.insert(
        2,
        BasicBlock {
            id: BlockId(40),
            parameters: vec![ValueDef::new(ValueId(60), Type::Scalar(ScalarType::U32))],
            operations: vec![],
            terminator: Some(Terminator::Return { values: vec![] }),
        },
    );
    module
}

fn mixed_roots() -> Module {
    let mut module = sequential();
    let mut ordinary = self::module(vec![constant(20, 17)]);
    ordinary.functions[0].id = "ordinary".into();
    ordinary.kernels[0].id = "ordinary-root".into();
    ordinary.kernels[0].entry = "ordinary".into();
    let mut context_only = self::module(vec![context()]);
    context_only.functions[0].id = "context-only".into();
    context_only.kernels[0].id = "context-root".into();
    context_only.kernels[0].entry = "context-only".into();
    module.functions.extend(ordinary.functions);
    module.functions.extend(context_only.functions);
    module.kernels.splice(0..0, context_only.kernels);
    module.kernels.extend(ordinary.kernels);
    let metadata = TargetCapability::Extension {
        namespace: "discharge.test".into(),
        name: "preserved".into(),
    };
    module.required_capabilities.insert(metadata.clone());
    module.functions[0]
        .required_capabilities
        .insert(metadata.clone());
    module.kernels[0].required_capabilities.insert(metadata);
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
    module.functions.insert(
        1,
        Function::external_import(
            "retained-declaration",
            Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
        ),
    );
    module
}

fn arithmetic() -> Module {
    let mut module = module(vec![
        constant(20, 9),
        context(),
        derive(11),
        scalar(
            21,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(20),
            },
        ),
        end(11, &[]),
        scalar(
            22,
            OperationKind::Call {
                callee: "callback".into(),
                arguments: vec![ValueId(21)],
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(22),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    let u32_type = Type::Scalar(ScalarType::U32);
    module.functions[0].signature.parameters = vec![
        Type::pointer(
            u32_type.clone(),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
        u32_type.clone(),
    ];
    module.functions.push(Function::internal_helper(
        "callback",
        Signature::new(vec![u32_type.clone()], vec![u32_type]),
        vec![ValueId(0)],
        vec![BasicBlock {
            id: BlockId(91),
            parameters: vec![],
            operations: vec![
                constant(1, 3),
                scalar(
                    2,
                    OperationKind::Binary {
                        op: BinaryOp::Multiply,
                        lhs: ValueId(0),
                        rhs: ValueId(1),
                    },
                ),
            ],
            terminator: Some(Terminator::Return {
                values: vec![ValueId(2)],
            }),
        }],
    ));
    module.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(1),
    };
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    module
}

fn input(module: &Module) -> (VerifiedCanonicalKernelIrModuleV15, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
            module,
            &mut budget,
        )
        .expect("fixture must pass full V15 semantic and lifecycle admission");
    assert_eq!(budget.storage(), 0);
    assert_eq!(owner.module(), module);
    (owner, storage.retained_storage())
}

struct Run {
    result: Result<
        (
            ProductionExecutionDischargeV29,
            ProductionExecutionDischargeStorageV29,
        ),
        ProductionExecutionDischargeErrorV29,
    >,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn run(
    input: &VerifiedCanonicalKernelIrModuleV15,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Run {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    work.charge_work(WORK_PREFIX).unwrap();
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = ProductionExecutionDischargeV29::try_discharge(input, &mut budget);
    assert_eq!(
        budget.storage(),
        floor,
        "all discharge paths must restore the incoming floor"
    );
    let (used, peak, failed_storage) = (
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    Run {
        result,
        work: used,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

fn check(module: &Module, erased: usize) -> ProductionExecutionDischargeV29 {
    let (input, storage) = input(module);
    let before_bytes = input.canonical_bytes().to_vec();
    let before_identity = *input.identity();
    let measured = run(&input, STORAGE_PREFIX + storage, usize::MAX, usize::MAX);
    let (owner, retained) = measured.result.unwrap();
    assert_eq!(owner.input_identity(), input.identity());
    assert_eq!(owner.erased_operations().len(), erased);
    assert!(retained.retained_storage() >= owner.output().canonical().canonical_bytes().len());
    assert!(measured.peak >= STORAGE_PREFIX + storage + retained.retained_storage());
    let mut expected = module.clone();
    for block in expected
        .functions
        .iter_mut()
        .filter_map(|f| f.body.as_mut())
        .flat_map(|body| &mut body.blocks)
    {
        block.operations.retain(|operation| match &operation.kind {
            OperationKind::Execution(
                Execution::ContextIssue | Execution::WorkgroupDerive { .. },
            ) => false,
            OperationKind::Execution(Execution::ScopeEnd { discarded, .. })
                if discarded.is_empty() =>
            {
                false
            }
            _ => true,
        });
    }
    let output: &VerifiedCanonicalKernelIrModuleV12 = owner.output();
    assert_eq!(
        output.module(),
        &expected,
        "ordinary operations, ABI and all metadata must survive exactly"
    );
    assert_eq!(
        fe2o3_kernel_ir::decode_module_v12(output.canonical().canonical_bytes()).unwrap(),
        expected
    );
    assert_eq!(input.module(), module);
    assert_eq!(input.canonical_bytes(), before_bytes);
    assert_eq!(input.identity(), &before_identity);
    owner
}

#[test]
fn discharges_context_only_and_distinct_sequential_scopes() {
    check(&module(vec![context()]), 1);
    check(
        &module(vec![execution(
            Some((u32::MAX, Role::Context)),
            Execution::ContextIssue,
        )]),
        1,
    );
    let owner = check(&sequential(), 5);
    use ProductionExecutionErasureKindV29::{ContextIssue, ScopeEnd, WorkgroupDerive};
    assert_eq!(
        owner
            .erased_operations()
            .iter()
            .map(|row| (
                row.function_ordinal,
                row.block_ordinal,
                row.operation_ordinal,
                row.kind
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, 0, 1, ContextIssue),
            (0, 0, 2, WorkgroupDerive),
            (0, 0, 4, ScopeEnd),
            (0, 0, 5, WorkgroupDerive),
            (0, 0, 7, ScopeEnd),
        ]
    );
}

#[test]
fn execution_free_graph_has_no_discharge() {
    let (input, storage) = input(&module(vec![constant(20, 17)]));
    let attempt = run(&input, STORAGE_PREFIX + storage, usize::MAX, usize::MAX);
    assert!(matches!(
        attempt.result,
        Err(ProductionExecutionDischargeErrorV29::NoExecution)
    ));
    assert_eq!((attempt.failed_work, attempt.failed_storage), (None, None));
}

#[test]
fn preserves_balanced_branch_terminators_scalar_phi_and_block_order() {
    for condition in [false, true] {
        let owner = check(&diamond(condition), 4);
        assert_eq!(
            owner
                .erased_operations()
                .iter()
                .map(|row| (
                    row.function_ordinal,
                    row.block_ordinal,
                    row.operation_ordinal
                ))
                .collect::<Vec<_>>(),
            vec![(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 3, 1)]
        );
    }
}

#[test]
fn preserves_mixed_roots_function_order_metadata_and_physical_abi() {
    check(&mixed_roots(), 6);
}

#[test]
fn discharged_store_and_retained_callback_match_cpu_arithmetic() {
    let owner = check(&arithmetic(), 3);
    let canonical = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.output().canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(canonical.identity(), owner.output().canonical().identity());
    let limits = SimulationLimitsV1::default();
    let simulation = AdmittedSimulationModuleV1::admit_v12(canonical, limits).unwrap();
    let target = SimulationTargetV1::amdgpu_64();
    for value in [0_u32, 1, 123, u32::MAX / 3 - 10, u32::MAX / 3 - 9] {
        let request = SimulationRequestV1::new(
            "entry",
            [1, 1, 1],
            [1, 1, 1],
            vec![
                SimulationArgumentV1::Buffer(
                    BufferArgumentV1::new(
                        ScalarType::U32,
                        AccessMode::ReadWrite,
                        4,
                        [0xa5a5_a5a5_u32, 0x5a5a_5a5a]
                            .into_iter()
                            .flat_map(u32::to_le_bytes)
                            .collect(),
                        vec![true; 8],
                        target,
                    )
                    .unwrap(),
                ),
                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value)),
            ],
        );
        let original = request.clone();
        let result = simulation.simulate(&request, target, limits).unwrap();
        let expected = [(value + 9) * 3, 0x5a5a_5a5a];
        assert_eq!(
            result.buffer(0).unwrap().bytes(),
            expected
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert_eq!(request, original);
    }
}

fn tile_module(consumption: u8) -> Module {
    let mut module = module(vec![
        context(),
        derive(11),
        execution(
            Some((
                12,
                Role::MaskedTileU32 {
                    lanes: 64,
                    elements: 1,
                },
            )),
            Execution::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes: 64,
                elements: 1,
            },
        ),
    ]);
    if consumption == 0 {
        operations(&mut module).push(end(11, &[12]));
        return module;
    }
    operations(&mut module).push(execution(
        Some((
            13,
            Role::LaneFragmentU32 {
                lanes: 64,
                elements: 1,
            },
        )),
        Execution::TileIntoFragmentU32 {
            tile: ValueId(12),
            lanes: 64,
            elements: 1,
        },
    ));
    if consumption == 1 {
        operations(&mut module).push(end(11, &[13]));
    } else {
        operations(&mut module).push(Operation::new(
            vec![
                ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(21), Type::BOOL),
            ],
            OperationKind::Execution(Execution::FragmentIntoPartsU32 {
                fragment: ValueId(13),
                lanes: 64,
                elements: 1,
            }),
        ));
        operations(&mut module).push(end(11, &[]));
    }
    module
}

#[test]
fn rejects_semantically_verified_tiles_fragments_and_nonempty_discard_rosters() {
    for consumption in 0..3 {
        let module = tile_module(consumption);
        let (input, storage) = input(&module);
        let attempt = run(&input, STORAGE_PREFIX + storage, usize::MAX, usize::MAX);
        assert!(
            matches!(
                attempt.result,
                Err(ProductionExecutionDischargeErrorV29::UnsupportedExecution)
            ),
            "fully valid unsupported execution graph must fail discharge"
        );
        assert_eq!(attempt.failed_work, None);
        assert_eq!(attempt.failed_storage, None);
        assert_eq!(input.module(), &module);
    }
}

#[test]
fn malformed_lifecycle_is_rejected_by_v15_owner_before_discharge() {
    let mut unbalanced = diamond(true);
    unbalanced.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .pop();
    let mut cycle = module(vec![context(), derive(11), end(11, &[])]);
    cycle.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    let mut nominal_parameter = module(vec![]);
    nominal_parameter.functions[0].signature.parameters[0] = Type::Execution(Role::Context);
    for module in [
        module(vec![context(), derive(11)]),
        module(vec![
            context(),
            derive(11),
            derive(100),
            end(100, &[]),
            end(11, &[]),
        ]),
        module(vec![context(), derive(11), end(11, &[]), end(11, &[])]),
        unbalanced,
        cycle,
        nominal_parameter,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(STORAGE_PREFIX).unwrap();
        let error =
            VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
                &module,
                &mut budget,
            )
            .unwrap_err();
        let CanonicalKernelIrReplayAdmissionErrorV15::Verification(errors) = error else {
            panic!("lifecycle fixture must reach semantic verification: {error:?}");
        };
        assert!(
            errors
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidSemanticOperation)
        );
        assert_eq!(budget.storage(), STORAGE_PREFIX);
    }
}

#[test]
fn exact_measured_work_and_peak_storage_succeed_and_one_short_fails() {
    for module in [
        module(vec![context()]),
        diamond(true),
        arithmetic(),
        mixed_roots(),
    ] {
        let (input, storage) = input(&module);
        let floor = STORAGE_PREFIX + storage;
        let measured = run(&input, floor, usize::MAX, usize::MAX);
        let (expected, receipt) = measured.result.unwrap();
        assert!(measured.work > WORK_PREFIX && measured.peak > floor);
        assert_eq!(
            (measured.failed_work, measured.failed_storage),
            (None, None)
        );
        let exact = run(&input, floor, measured.work, measured.peak);
        let (actual, exact_receipt) = exact.result.unwrap();
        assert_eq!(actual.output(), expected.output());
        assert_eq!(actual.erased_operations(), expected.erased_operations());
        assert_eq!(exact_receipt.retained_storage(), receipt.retained_storage());
        assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
        let short_work = run(&input, floor, measured.work - 1, measured.peak);
        assert!(matches!(short_work.result,
            Err(ProductionExecutionDischargeErrorV29::Resource(ResourceError::Work(error)))
                if error.actual() == measured.work && error.limit() == measured.work - 1
        ));
        assert_eq!(short_work.failed_work, Some(measured.work));
        assert!(short_work.work < measured.work);
        assert_eq!(short_work.failed_storage, None);
        let short_storage = run(&input, floor, measured.work, measured.peak - 1);
        assert!(short_storage.result.is_err());
        assert_eq!(short_storage.failed_storage, Some(measured.peak));
        assert!(short_storage.peak < measured.peak);
        assert_eq!(short_storage.failed_work, None);
        for (work_limit, storage_limit) in [(WORK_PREFIX, measured.peak), (measured.work, floor)] {
            assert!(
                run(&input, floor, work_limit, storage_limit)
                    .result
                    .is_err()
            );
        }
    }
}

#[test]
fn nested_copy_and_output_admission_errors_restore_the_incoming_floor() {
    let (input, storage) = input(&sequential());
    let floor = STORAGE_PREFIX + storage;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    let (candidate, _) = input
        .copy_module_for_transformation_v15(&mut budget)
        .unwrap();
    let copy_work = budget.work();
    drop(candidate);
    assert_eq!(budget.storage(), floor);
    let bytes = input.canonical_bytes().len();
    // Stop work immediately before each foreign admission, after independently
    // measuring the public V15 candidate-copy API.
    for (limit, copying) in [
        (WORK_PREFIX + bytes, true),
        (WORK_PREFIX + bytes * 2 + copy_work, false),
    ] {
        let attempt = run(&input, floor, limit, usize::MAX);
        assert!(matches!(
            (attempt.result, copying),
            (Err(ProductionExecutionDischargeErrorV29::Input(_)), true)
                | (Err(ProductionExecutionDischargeErrorV29::Output(_)), false)
        ));
        assert_eq!(attempt.work, limit);
        assert!(attempt.failed_work.is_some_and(|failed| failed > limit));
        assert_eq!(attempt.failed_storage, None);
    }
}

#[test]
fn retained_storage_transfers_and_prior_resource_denials_survive_cleanup() {
    let (input, input_storage) = input(&sequential());
    let floor = STORAGE_PREFIX + input_storage;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.charge_work(WORK_PREFIX).unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(budget.charge_work(usize::MAX).is_err());
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let (owner, storage) =
        ProductionExecutionDischargeV29::try_discharge(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert!(budget.work() > WORK_PREFIX);
    assert!(budget.peak_storage() >= floor + storage.retained_storage());
    budget.reserve_storage(storage.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(
        owner.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations
            .len(),
        4
    );
    assert_eq!(
        budget.storage(),
        STORAGE_PREFIX + storage.retained_storage()
    );
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), STORAGE_PREFIX);
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

fn output(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .expect("altered candidate must be independently valid V12 before replay is tested");
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}

fn replay(
    input: &VerifiedCanonicalKernelIrModuleV15,
    output: &VerifiedCanonicalKernelIrModuleV12,
    rows: &[ProductionExecutionErasureV29],
    floor: usize,
) -> Result<(), ProductionExecutionDischargeErrorV29> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    let result = replay_erasure(input, output, rows, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), None);
    assert!(
        budget.work() >= input.canonical_bytes().len() + output.canonical().canonical_bytes().len()
    );
    assert_eq!(work.failed_work(), None);
    result
}

type Mutation = (&'static str, fn(&mut Module));

fn rejects_valid_output_mutations(source: &Module, mutations: &[Mutation]) {
    let (input, input_storage) = input(source);
    let (discharged, storage) = run(
        &input,
        STORAGE_PREFIX + input_storage,
        usize::MAX,
        usize::MAX,
    )
    .result
    .unwrap();
    let floor = STORAGE_PREFIX + input_storage + storage.retained_storage();
    replay(
        &input,
        discharged.output(),
        discharged.erased_operations(),
        floor,
    )
    .unwrap();
    for (label, mutate) in mutations {
        let mut candidate = discharged.output().module().clone();
        mutate(&mut candidate);
        assert_ne!(
            &candidate,
            discharged.output().module(),
            "mutation did nothing: {label}"
        );
        let (altered, altered_storage) = output(&candidate);
        assert!(
            matches!(
                replay(
                    &input,
                    &altered,
                    discharged.erased_operations(),
                    floor + altered_storage
                ),
                Err(ProductionExecutionDischargeErrorV29::ReplayMismatch)
            ),
            "valid altered {label} passed structural replay"
        );
    }
}

#[test]
fn replay_rejects_valid_ordinary_operation_call_store_and_terminator_changes() {
    rejects_valid_output_mutations(
        &arithmetic(),
        &[
            ("constant", |m| operations(m)[0] = constant(20, 8)),
            ("arithmetic operand", |m| {
                let OperationKind::Binary { rhs, .. } = &mut operations(m)[1].kind else {
                    panic!()
                };
                *rhs = ValueId(1);
            }),
            ("callback argument", |m| {
                let OperationKind::Call { arguments, .. } = &mut operations(m)[2].kind else {
                    panic!()
                };
                arguments[0] = ValueId(1);
            }),
            ("store value", |m| {
                let OperationKind::Store { value, .. } = &mut operations(m)[3].kind else {
                    panic!()
                };
                *value = ValueId(21);
            }),
            ("deleted store", |m| {
                operations(m).pop().unwrap();
            }),
            ("extra ordinary operation", |m| {
                operations(m).push(constant(90, 123))
            }),
            ("callback return", |m| {
                m.functions[1].body.as_mut().unwrap().blocks[0].terminator =
                    Some(Terminator::Return {
                        values: vec![ValueId(0)],
                    });
            }),
        ],
    );
}

#[test]
fn replay_rejects_valid_metadata_order_and_physical_abi_changes() {
    rejects_valid_output_mutations(
        &mixed_roots(),
        &[
            ("module identity", |m| m.id = "other-module".into()),
            ("kernel identity", |m| m.kernels[0].id = "other-root".into()),
            ("launch extent", |m| {
                m.kernels[0].domain = LaunchDomain::D1 {
                    x: LaunchExtent::Static(128),
                }
            }),
            ("workgroup size", |m| {
                m.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1))
            }),
            ("module capabilities", |m| m.required_capabilities.clear()),
            ("function capabilities", |m| {
                m.functions[0].required_capabilities.clear()
            }),
            ("kernel capabilities", |m| {
                m.kernels[0].required_capabilities.clear()
            }),
            ("root order", |m| m.kernels.swap(0, 1)),
            ("function order", |m| m.functions.swap(1, 2)),
            ("ordinary operation order", |m| operations(m).swap(0, 1)),
            ("additional physical parameter", |m| {
                m.functions[0]
                    .signature
                    .parameters
                    .push(Type::Scalar(ScalarType::U32));
                m.functions[0]
                    .body
                    .as_mut()
                    .unwrap()
                    .parameters
                    .push(ValueId(99));
            }),
            ("physical parameter identity order", |m| {
                m.functions[0].body.as_mut().unwrap().parameters.swap(0, 1)
            }),
        ],
    );
    rejects_valid_output_mutations(
        &diamond(true),
        &[
            ("block order", |m| {
                m.functions[0].body.as_mut().unwrap().blocks.swap(1, 3)
            }),
            ("branch destination", |m| {
                let Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) = &mut m.functions[0].body.as_mut().unwrap().blocks[0].terminator
                else {
                    panic!()
                };
                std::mem::swap(then_target, else_target);
            }),
            ("scalar edge argument", |m| {
                let block = &mut m.functions[0].body.as_mut().unwrap().blocks[1];
                block.operations.push(constant(90, 99));
                block.terminator = Some(Terminator::Branch {
                    target: BlockId(40),
                    arguments: vec![ValueId(90)],
                });
            }),
        ],
    );
}

#[test]
fn replay_binds_every_erasure_kind_and_input_ordinal_without_trusting_rows() {
    let (input, input_storage) = input(&sequential());
    let (owner, storage) = run(
        &input,
        STORAGE_PREFIX + input_storage,
        usize::MAX,
        usize::MAX,
    )
    .result
    .unwrap();
    let rows = owner.erased_operations();
    let floor = STORAGE_PREFIX + input_storage + storage.retained_storage();
    replay(&input, owner.output(), rows, floor).unwrap();
    let mut mutations = vec![vec![], rows[1..].to_vec(), rows[..rows.len() - 1].to_vec()];
    let mut extra = rows.to_vec();
    extra.push(rows[0]);
    mutations.push(extra);
    let mut swapped = rows.to_vec();
    swapped.swap(1, 3);
    mutations.push(swapped);
    for index in 0..rows.len() {
        for field in 0..4 {
            let mut altered = rows.to_vec();
            match field {
                0 => altered[index].function_ordinal += 1,
                1 => altered[index].block_ordinal += 1,
                2 => altered[index].operation_ordinal += 1,
                _ => {
                    altered[index].kind = match altered[index].kind {
                        ProductionExecutionErasureKindV29::ContextIssue => {
                            ProductionExecutionErasureKindV29::ScopeEnd
                        }
                        _ => ProductionExecutionErasureKindV29::ContextIssue,
                    }
                }
            }
            mutations.push(altered);
        }
    }
    for (case, altered) in mutations.iter().enumerate() {
        assert!(
            matches!(
                replay(&input, owner.output(), altered, floor),
                Err(ProductionExecutionDischargeErrorV29::ReplayMismatch)
            ),
            "altered erasure case {case} replayed"
        );
    }
}

#[test]
fn replay_rejects_foreign_input_with_an_equal_erased_output() {
    let module = sequential();
    let (first, first_storage) = input(&module);
    let (owner, storage) = run(
        &first,
        STORAGE_PREFIX + first_storage,
        usize::MAX,
        usize::MAX,
    )
    .result
    .unwrap();
    let mut foreign = module.clone();
    // Move an independent ordinary operation across ContextIssue. Erasure yields
    // the same ordinary graph, but its original operation ordinals differ.
    operations(&mut foreign).swap(0, 1);
    let (foreign, foreign_storage) = input(&foreign);
    assert_ne!(first.identity(), foreign.identity());
    let (foreign_output, foreign_receipt) = run(
        &foreign,
        STORAGE_PREFIX + foreign_storage,
        usize::MAX,
        usize::MAX,
    )
    .result
    .unwrap();
    assert_eq!(foreign_output.output(), owner.output());
    assert_ne!(
        foreign_output.erased_operations(),
        owner.erased_operations()
    );
    let floor = STORAGE_PREFIX
        + first_storage
        + foreign_storage
        + storage.retained_storage()
        + foreign_receipt.retained_storage();
    assert!(matches!(
        replay(&foreign, owner.output(), owner.erased_operations(), floor),
        Err(ProductionExecutionDischargeErrorV29::ReplayMismatch)
    ));
    replay(
        &foreign,
        foreign_output.output(),
        foreign_output.erased_operations(),
        floor,
    )
    .unwrap();
}
