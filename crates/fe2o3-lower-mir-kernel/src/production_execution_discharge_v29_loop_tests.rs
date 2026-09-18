use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, BinaryOp, BlockId,
    CanonicalKernelIrWorkBudgetV1, ComparePredicate, Constant, DiagnosticCode,
    ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role, Kernel, KernelIrEncodeError,
    LaunchDomain, LaunchExtent, MemoryAccess, Operation, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId, VerifiedCanonicalKernelIrV12, WorkgroupMemory, WorkgroupMemoryExtent,
    WorkgroupSize,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

const ENTRY: u32 = 7;
const HEADER: u32 = 41;
const BODY: u32 = 83;
const EXIT: u32 = 127;

fn scalar(id: u32, kind: OperationKind) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        kind,
    )
}

fn binary(id: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    scalar(
        id,
        OperationKind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn execution(result: Option<(u32, Role)>, kind: Execution) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, role)| ValueDef::new(ValueId(id), Type::Execution(role)))
            .collect(),
        OperationKind::Execution(kind),
    )
}

fn pointer_type() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    )
}

fn pointer(id: u32, offset: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), pointer_type()),
        OperationKind::GetElementPointer {
            base: ValueId(0),
            offset: ValueId(offset),
        },
    )
}

fn store(pointer: u32, value: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}

fn branch(target: u32, arguments: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: arguments.iter().copied().map(ValueId).collect(),
    }
}

fn block(
    id: u32,
    parameters: &[u32],
    operations: Vec<Operation>,
    terminator: Terminator,
) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        parameters: parameters
            .iter()
            .map(|&id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)))
            .collect(),
        operations,
        terminator: Some(terminator),
    }
}

fn less_than(id: u32, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn block_mut(module: &mut Module, id: u32) -> &mut BasicBlock {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|b| b.id == BlockId(id))
        .unwrap()
}

// Parameters: output, outer bound, seed, inner bound (unused by the single loop).
// The header carries ordinary (counter, accumulator) values; nominal ownership
// is identical on entry and on the backedge despite those changing values.
fn loop_module() -> Module {
    let mut module = Module::new("v29-loop-discharge");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                pointer_type(),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![
            block(
                ENTRY,
                &[],
                vec![
                    execution(Some((10, Role::Context)), Execution::ContextIssue),
                    execution(
                        Some((11, Role::Workgroup)),
                        Execution::WorkgroupDerive {
                            context: ValueId(10),
                        },
                    ),
                    scalar(20, OperationKind::Constant(Constant::U32(0))),
                    scalar(21, OperationKind::Constant(Constant::U32(1))),
                    scalar(22, OperationKind::Constant(Constant::U32(3))),
                    scalar(23, OperationKind::Constant(Constant::U32(2))),
                ],
                branch(HEADER, &[20, 2]),
            ),
            // Deliberately not CFG traversal order.
            block(
                EXIT,
                &[],
                vec![
                    execution(
                        None,
                        Execution::ScopeEnd {
                            workgroup: ValueId(11),
                            discarded: vec![],
                        },
                    ),
                    pointer(60, 21),
                    store(60, 31),
                ],
                Terminator::Return { values: vec![] },
            ),
            block(
                BODY,
                &[],
                vec![
                    binary(50, BinaryOp::Add, 30, 21),
                    binary(51, BinaryOp::Multiply, 31, 22),
                    binary(52, BinaryOp::Add, 51, 50),
                    binary(53, BinaryOp::Add, 30, 23),
                    pointer(54, 53),
                    store(54, 52),
                ],
                branch(HEADER, &[50, 52]),
            ),
            block(
                HEADER,
                &[30, 31],
                vec![less_than(32, 30, 1)],
                Terminator::ConditionalBranch {
                    condition: ValueId(32),
                    then_target: BlockId(BODY),
                    then_arguments: vec![],
                    else_target: BlockId(EXIT),
                    else_arguments: vec![],
                },
            ),
        ],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    module
}

fn iteration_scope_module() -> Module {
    let mut module = loop_module();
    let derive = block_mut(&mut module, ENTRY).operations.remove(1);
    let end = block_mut(&mut module, EXIT).operations.remove(0);
    let body = block_mut(&mut module, BODY);
    body.operations.insert(0, derive);
    body.operations.push(end);
    module
}

fn nested_loop_module() -> Module {
    let mut module = loop_module();
    let body = block_mut(&mut module, BODY);
    let mut stores = body.operations.split_off(3);
    body.operations.truncate(1);
    body.terminator = Some(branch(61, &[20, 31]));
    *stores.last_mut().unwrap() = store(54, 80);
    module.functions[0].body.as_mut().unwrap().blocks.extend([
        block(
            61,
            &[70, 71],
            vec![less_than(72, 70, 3)],
            Terminator::ConditionalBranch {
                condition: ValueId(72),
                then_target: BlockId(97),
                then_arguments: vec![],
                else_target: BlockId(109),
                else_arguments: vec![ValueId(71)],
            },
        ),
        block(
            97,
            &[],
            vec![
                binary(73, BinaryOp::Add, 70, 21),
                binary(74, BinaryOp::Multiply, 71, 22),
                binary(75, BinaryOp::Add, 74, 50),
                binary(76, BinaryOp::Add, 75, 73),
            ],
            branch(61, &[73, 76]),
        ),
        block(109, &[80], stores, branch(HEADER, &[50, 80])),
    ]);
    module
}

fn input(module: &Module) -> (VerifiedCanonicalKernelIrModuleV15, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (input, storage) =
        VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
            module,
            &mut budget,
        )
        .expect("loop fixture must pass canonical V15 admission");
    assert_eq!(input.module(), module);
    assert_eq!(budget.storage(), 0);
    (input, storage.retained_storage())
}

fn discharge(
    module: &Module,
) -> (
    VerifiedCanonicalKernelIrModuleV15,
    ProductionExecutionDischargeV29,
    usize,
) {
    let (input, input_storage) = input(module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let floor = 7 + input_storage;
    budget.reserve_storage(floor).unwrap();
    let (owner, storage) =
        ProductionExecutionDischargeV29::try_discharge(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(owner.input_identity(), input.identity());
    assert_eq!(input.module(), module);
    let mut expected = module.clone();
    for block in &mut expected.functions[0].body.as_mut().unwrap().blocks {
        block.operations.retain(|op| match &op.kind {
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
    assert_eq!(
        owner.output().module(),
        &expected,
        "retain every ordinary loop field exactly"
    );
    use ProductionExecutionErasureKindV29::{ContextIssue, ScopeEnd, WorkgroupDerive};
    assert_eq!(
        owner
            .erased_operations()
            .iter()
            .map(|row| row.kind())
            .collect::<Vec<_>>(),
        vec![ContextIssue, WorkgroupDerive, ScopeEnd]
    );
    (input, owner, floor + storage.retained_storage())
}

fn simulation(owner: &ProductionExecutionDischargeV29) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.output().canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(canonical.identity(), owner.output().canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(canonical, SimulationLimitsV1::default()).unwrap()
}

fn compare_cpu(module: &Module, cases: &[(u32, u32)], nested: bool) {
    let (_input, owner, _) = discharge(module);
    let simulation = simulation(&owner);
    let target = SimulationTargetV1::amdgpu_64();
    for &(count, inner_count) in cases {
        for seed in [0, 7, 101] {
            let mut initial = [0xa5a5_a5a5_u32; 16];
            initial[0] = 0x1357_9bdf;
            initial[15] = 0xfdb9_7531;
            let mut expected = initial;
            let mut accumulator = seed;
            for i in 0..count {
                if nested {
                    for j in 0..inner_count {
                        accumulator = accumulator * 3 + (i + 1) + (j + 1);
                    }
                } else {
                    accumulator = accumulator * 3 + (i + 1);
                }
                expected[i as usize + 2] = accumulator;
            }
            expected[1] = accumulator;
            let bytes = initial.into_iter().flat_map(u32::to_le_bytes).collect();
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
                            bytes,
                            vec![true; 64],
                            target,
                        )
                        .unwrap(),
                    ),
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(count)),
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(seed)),
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(inner_count)),
                ],
            );
            let original = request.clone();
            let result = simulation
                .simulate(&request, target, SimulationLimitsV1::default())
                .unwrap();
            assert_eq!(
                result.buffer(0).unwrap().bytes(),
                expected
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
                "count={count}, inner_count={inner_count}, seed={seed}, nested={nested}"
            );
            assert_eq!(result.buffer(0).unwrap().initialized(), &[true; 64]);
            assert!(!result.grants_execution_authority());
            assert_eq!(request, original);
        }
    }
}

#[test]
fn discharged_loops_match_cpu_with_changing_phi_values_and_store_canaries() {
    for module in [loop_module(), iteration_scope_module()] {
        compare_cpu(&module, &[(0, 0), (1, 0), (5, 0), (9, 0)], false);
    }
}

#[test]
fn discharged_nested_loops_match_cpu_including_empty_inner_loops() {
    compare_cpu(
        &nested_loop_module(),
        &[(0, 4), (1, 0), (1, 1), (1, 5), (4, 0), (4, 1), (3, 4)],
        true,
    );
}

#[test]
fn replay_refuses_independently_valid_loop_bound_edge_phi_and_metadata_changes() {
    type Mutation = (&'static str, fn(&mut Module));
    let mutations: &[Mutation] = &[
        ("bound", |m| {
            let OperationKind::Compare { rhs, .. } = &mut block_mut(m, HEADER).operations[0].kind
            else {
                panic!()
            };
            *rhs = ValueId(3);
        }),
        ("inclusive bound", |m| {
            let OperationKind::Compare { predicate, .. } =
                &mut block_mut(m, HEADER).operations[0].kind
            else {
                panic!()
            };
            *predicate = ComparePredicate::LessThanOrEqual;
        }),
        ("branch targets", |m| {
            let Some(Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            }) = &mut block_mut(m, HEADER).terminator
            else {
                panic!()
            };
            std::mem::swap(then_target, else_target);
        }),
        ("backedge target", |m| {
            block_mut(m, BODY).terminator = Some(branch(EXIT, &[]))
        }),
        ("initial phi arguments", |m| {
            block_mut(m, ENTRY).terminator = Some(branch(HEADER, &[20, 20]))
        }),
        ("backedge phi arguments", |m| {
            block_mut(m, BODY).terminator = Some(branch(HEADER, &[50, 31]))
        }),
        ("phi parameter order", |m| {
            block_mut(m, HEADER).parameters.swap(0, 1)
        }),
        ("counter step", |m| {
            block_mut(m, BODY).operations[0] = binary(50, BinaryOp::Add, 30, 23)
        }),
        ("loop store", |m| {
            *block_mut(m, BODY).operations.last_mut().unwrap() = store(54, 31)
        }),
        ("ordinary memory metadata", |m| {
            let OperationKind::Store { access, .. } =
                &mut block_mut(m, BODY).operations.last_mut().unwrap().kind
            else {
                panic!()
            };
            access.volatile = true;
        }),
        ("block order", |m| {
            m.functions[0].body.as_mut().unwrap().blocks.swap(1, 2)
        }),
    ];
    let (input, owner, floor) = discharge(&loop_module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    replay_erasure(
        &input,
        owner.output(),
        owner.erased_operations(),
        &mut budget,
    )
    .unwrap();
    for &(label, mutate) in mutations {
        let mut candidate = owner.output().module().clone();
        mutate(&mut candidate);
        assert_ne!(
            &candidate,
            owner.output().module(),
            "{label} must change the output"
        );
        let (altered, storage) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &candidate,
                &mut budget,
            )
            .unwrap_or_else(|error| {
                panic!("{label} must remain independently valid V12: {error:?}")
            });
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(
            matches!(
                replay_erasure(&input, &altered, owner.erased_operations(), &mut budget),
                Err(ProductionExecutionDischargeErrorV29::ReplayMismatch)
            ),
            "{label} passed replay"
        );
        drop(altered);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn loop_admission_still_refuses_changed_ownership_and_live_scope_traps() {
    let mut changed = loop_module();
    let end = block_mut(&mut changed, EXIT).operations[0].clone();
    block_mut(&mut changed, BODY).operations.push(end);
    let mut trapping = loop_module();
    let trap = AmdGpuDiagnosticOperation::Trap;
    block_mut(&mut trapping, BODY)
        .operations
        .push(trap.operation(None));
    trapping.functions.push(trap.declaration());
    for module in [changed, trapping] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let error =
            VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
                &module,
                &mut budget,
            )
            .unwrap_err();
        let CanonicalKernelIrReplayAdmissionErrorV15::Verification(errors) = error else {
            panic!("expected lifecycle refusal: {error:?}")
        };
        assert!(
            errors
                .diagnostics()
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidSemanticOperation)
        );
    }
}

#[test]
fn balanced_tile_loop_is_valid_v15_but_still_refused_by_discharge() {
    let mut module = iteration_scope_module();
    module.functions[0].signature.parameters.extend([
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::INDEX,
    ]);
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .extend([ValueId(4), ValueId(5)]);
    let body = block_mut(&mut module, BODY);
    body.operations.insert(
        1,
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
                input: ValueId(4),
                base: ValueId(5),
                lanes: 64,
                elements: 1,
            },
        ),
    );
    *body.operations.last_mut().unwrap() = execution(
        None,
        Execution::ScopeEnd {
            workgroup: ValueId(11),
            discarded: vec![ValueId(12)],
        },
    );
    let (input, storage) = input(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(storage).unwrap();
    assert!(matches!(
        ProductionExecutionDischargeV29::try_discharge(&input, &mut budget),
        Err(ProductionExecutionDischargeErrorV29::UnsupportedExecution)
    ));
    assert_eq!(budget.storage(), storage);
}

#[test]
fn loop_does_not_make_runtime_memory_authority_canonical() {
    let mut module = loop_module();
    block_mut(&mut module, ENTRY)
        .operations
        .push(Operation::effect_free(
            ValueDef::new(
                ValueId(90),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::DynamicAtLeast(13),
                alignment: 4,
            }),
        ));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let error = VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
        &module,
        &mut budget,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CanonicalKernelIrReplayAdmissionErrorV15::Encode(
            KernelIrEncodeError::UnsupportedInVersion {
                feature: "authenticated dynamic workgroup-memory extent",
                ..
            }
        )
    ));
}
