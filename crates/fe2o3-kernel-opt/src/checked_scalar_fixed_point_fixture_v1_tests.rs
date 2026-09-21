use super::*;
pub(super) use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CheckedBinaryOperator, ComparePredicate, Constant, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV12,
};
pub(super) use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationErrorV1, SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationExecutionErrorKindV1,
    SimulationExecutionOutcomeV1, SimulationInvocationV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationTargetV1,
};
pub(super) const W: usize = usize::MAX / 4;
pub(super) const S: usize = 512 * 1024 * 1024;
pub(super) const SIBLING: usize = 43;
pub(super) fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let (input, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (input, storage.retained_storage())
}
pub(super) fn with_input(module: Module, run: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (input, retained) = admit(&module);
    drop(module);
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let floor = retained + SIBLING;
    budget.reserve_storage(floor).unwrap();
    run(&input, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(input);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), SIBLING);
}
pub(super) fn finish(input: &Owner, budget: &mut Budget<'_>) -> CheckedScalarFixedPointOwnerV1 {
    let value = prepare_checked_scalar_fixed_point_v1(input, budget).unwrap();
    budget.reserve_storage(value.retained_storage()).unwrap();
    value
}
pub(super) fn release(value: CheckedScalarFixedPointOwnerV1, budget: &mut Budget<'_>) {
    let retained = value.retained_storage();
    drop(value);
    budget.release_storage(retained).unwrap();
}
pub(super) fn op(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
pub(super) fn binary(id: u32, lhs: u32, rhs: u32) -> Operation {
    op(
        id,
        Type::Scalar(ScalarType::U32),
        Kind::Binary {
            op: BinaryOp::BitOr,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
pub(super) fn branch(id: u32, values: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(id),
        arguments: values.iter().copied().map(ValueId).collect(),
    }
}
pub(super) fn block(id: u32, term: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(term);
    block
}
pub(super) fn operations(owner: &Owner) -> impl Iterator<Item = &Operation> {
    owner
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
}
pub(super) fn binaries(owner: &Owner) -> usize {
    operations(owner)
        .filter(|op| matches!(op.kind, Kind::Binary { .. }))
        .count()
}

// Legal CFG order is entry -> producers -> use; physical order deliberately puts
// use first. Policy3 may merge the CFG but does not add integer neutral rules.
pub(super) fn reverse_chain(length: usize) -> Module {
    assert!((1..=64).contains(&length));
    let ty = Type::Scalar(ScalarType::U32);
    let mut entry = block(10, branch(100, &[]));
    entry
        .operations
        .push(op(2, ty.clone(), Kind::Constant(Constant::U32(0))));
    let result = 3 + length as u32;
    let mut use_block = block(
        20,
        Terminator::Return {
            values: vec![ValueId(result)],
        },
    );
    use_block.operations.push(binary(result, 1, result - 1));
    let mut blocks = vec![entry, use_block];
    for index in (0..length).rev() {
        let id = 3 + index as u32;
        let next = if index + 1 == length {
            20
        } else {
            101 + index as u32
        };
        let mut producer = block(100 + index as u32, branch(next, &[]));
        producer
            .operations
            .push(binary(id, if index == 0 { 2 } else { id - 1 }, 2));
        blocks.push(producer);
    }
    let mut module = Module::new("scalar-fixed-point-reverse");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(1)],
        blocks,
    ));
    module
}

pub(super) fn integer_kernel(zero: Constant, neighbor: bool) -> Module {
    let ty = zero.ty();
    let Type::Scalar(scalar) = ty else {
        unreachable!()
    };
    let width = width(scalar);
    let mut entry = block(17, Terminator::Return { values: vec![] });
    let value = if neighbor {
        match scalar {
            ScalarType::I8 => Constant::I8(1),
            ScalarType::I16 => Constant::I16(1),
            ScalarType::I32 => Constant::I32(1),
            ScalarType::I64 => Constant::I64(1),
            ScalarType::U8 => Constant::U8(1),
            ScalarType::U16 => Constant::U16(1),
            ScalarType::U32 => Constant::U32(1),
            ScalarType::U64 => Constant::U64(1),
            _ => unreachable!(),
        }
    } else {
        zero
    };
    entry.operations = vec![
        op(3, ty.clone(), Kind::Constant(value)),
        Operation::checked_binary(
            ValueDef::new(ValueId(4), ty.clone()),
            ValueDef::new(ValueId(5), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(0),
            ValueId(3),
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(1),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, width as u32),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
    ];
    let mut module = Module::new("scalar-fixed-point-integer-kernel");
    module.functions.push(Function::kernel_entry(
        "k_impl",
        Signature::new(
            vec![
                ty.clone(),
                Type::pointer(ty, AddressSpace::Global, AccessMode::ReadWrite),
                Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry],
    ));
    module.kernels.push(Kernel::new(
        "k",
        "k_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
pub(super) fn float_kernel(zero: Constant) -> Module {
    let ty = zero.ty();
    let Type::Scalar(scalar) = ty else {
        unreachable!()
    };
    let mut entry = block(17, Terminator::Return { values: vec![] });
    entry.operations = vec![
        op(2, ty.clone(), Kind::Constant(zero)),
        op(
            3,
            ty.clone(),
            Kind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(1),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, width(scalar) as u32),
            },
        ),
    ];
    let mut module = Module::new("scalar-fixed-point-ieee-kernel");
    module.functions.push(Function::kernel_entry(
        "k_impl",
        Signature::new(
            vec![
                ty.clone(),
                Type::pointer(ty, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    module.kernels.push(Kernel::new(
        "k",
        "k_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
pub(super) fn width(scalar: ScalarType) -> usize {
    match scalar {
        ScalarType::Bool | ScalarType::I8 | ScalarType::U8 => 1,
        ScalarType::I16 | ScalarType::U16 | ScalarType::F16 => 2,
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => 4,
        ScalarType::I64 | ScalarType::U64 | ScalarType::F64 => 8,
        _ => panic!("outside scalar fixture"),
    }
}
pub(super) fn buffer(scalar: ScalarType) -> SimulationArgumentV1 {
    let n = width(scalar);
    SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            scalar,
            AccessMode::ReadWrite,
            n as u32,
            vec![0xa5; n * 2],
            vec![false; n * 2],
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap(),
    )
}
pub(super) fn scalar_request(scalar: ScalarType, value: u128, flag: bool) -> SimulationRequestV1 {
    let mut arguments = vec![
        SimulationArgumentV1::Scalar(
            ScalarBitsV1::new(scalar, value, SimulationTargetV1::amdgpu_64()).unwrap(),
        ),
        buffer(scalar),
    ];
    if flag {
        arguments.push(buffer(ScalarType::Bool));
    }
    SimulationRequestV1::new("k", [1, 1, 1], [1, 1, 1], arguments)
}

pub(super) fn loop_kernel(trap: bool, volatile: bool) -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let mut entry = block(
        10,
        Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(20),
            then_arguments: vec![ValueId(4)],
            else_target: BlockId(20),
            else_arguments: vec![ValueId(4)],
        },
    );
    entry.operations = vec![
        op(4, u32_ty.clone(), Kind::Constant(Constant::U32(0))),
        op(5, u32_ty.clone(), Kind::Constant(Constant::U32(1))),
    ];
    let mut header = block(
        20,
        Terminator::ConditionalBranch {
            condition: ValueId(11),
            then_target: BlockId(30),
            then_arguments: vec![],
            else_target: BlockId(40),
            else_arguments: vec![],
        },
    );
    header
        .parameters
        .push(ValueDef::new(ValueId(10), u32_ty.clone()));
    header.operations.push(op(
        11,
        Type::BOOL,
        Kind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(10),
            rhs: ValueId(0),
        },
    ));
    let mut body = block(30, branch(20, &[13]));
    let mut access = MemoryAccess::new(AddressSpace::Global, 4);
    access.volatile = volatile;
    body.operations = vec![
        binary(12, 1, 4),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(12),
                access,
            },
        ),
        op(
            13,
            u32_ty.clone(),
            Kind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(5),
            },
        ),
    ];
    if trap {
        body.operations.push(op(
            14,
            u32_ty.clone(),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(1),
                rhs: ValueId(4),
            },
        ));
    }
    let mut dead = block(99, Terminator::Return { values: vec![] });
    dead.operations.push(Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(2),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let mut module = Module::new("scalar-fixed-point-cyclic-effects");
    module.functions.push(Function::kernel_entry(
        "k_impl",
        Signature::new(
            vec![
                u32_ty.clone(),
                u32_ty.clone(),
                Type::pointer(u32_ty, AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![],
        ),
        (0..4).map(ValueId).collect(),
        vec![
            entry,
            header,
            body,
            block(40, Terminator::Return { values: vec![] }),
            dead,
        ],
    ));
    module.kernels.push(Kernel::new(
        "k",
        "k_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
pub(super) fn sim(owner: &Owner) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(canonical.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(canonical, SimulationLimitsV1::default()).unwrap()
}

// Operation/block locations intentionally change under legal CFG/DCE rewrites.
// Compare the COMPLETE ordered observable effect/lifecycle projection, retaining
// invocation, function ordinal and every event payload; never drop an effect.
#[derive(Default, Debug, Eq, PartialEq)]
pub(super) struct Effects(pub Vec<(SimulationInvocationV1, usize, EventKind)>);
impl SimulationEventSinkV1 for Effects {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if !matches!(
            event.kind,
            EventKind::BlockEnter
                | EventKind::OperationBegin
                | EventKind::OperationEnd { .. }
                | EventKind::Terminator
                | EventKind::Branch { .. }
        ) {
            if self.0.len() >= 1024 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "fixed-point complete effect bound".into(),
                });
            }
            self.0.push((
                event.invocation,
                event.site.function_ordinal,
                event.kind.clone(),
            ));
        }
        Ok(())
    }
}
pub(super) fn compare_sim(
    before: &AdmittedSimulationModuleV1,
    after: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    trap: bool,
) -> Option<Vec<SimulationArgumentV1>> {
    let unchanged = request.clone();
    let mut a = Effects::default();
    let mut b = Effects::default();
    let old = before.simulate_observed_with_sink(
        request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        &mut a,
    );
    let new = after.simulate_observed_with_sink(
        request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        &mut b,
    );
    assert_eq!(request, &unchanged);
    assert_eq!(a, b);
    assert!(a.0.iter().any(|(_, _, event)| matches!(event, EventKind::InvocationEnd { outcome } if *outcome == if trap { SimulationExecutionOutcomeV1::Failed } else { SimulationExecutionOutcomeV1::Completed })));
    match (old, new) {
        (Ok(old), Ok(new)) => {
            assert!(!trap);
            assert_eq!(old.arguments(), new.arguments());
            Some(new.arguments().to_vec())
        }
        (Err(SimulationErrorV1::Execution(old)), Err(SimulationErrorV1::Execution(new))) => {
            assert!(trap);
            assert_eq!(old.kind, new.kind);
            assert!(matches!(
                old.kind,
                SimulationExecutionErrorKindV1::UndefinedIntegerOperation(_)
            ));
            assert_eq!(old.invocation, new.invocation);
            assert!(old.observation_failure.is_none() && new.observation_failure.is_none());
            None
        }
        other => panic!("simulation refused or changed outcome: {other:?}"),
    }
}
