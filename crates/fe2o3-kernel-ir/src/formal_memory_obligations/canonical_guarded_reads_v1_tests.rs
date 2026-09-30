use super::*;
use crate::{BasicBlock, Kernel, Signature, ValueDef};

#[path = "canonical_guarded_reads_hostile_v1_tests.rs"]
mod hostile;
#[path = "canonical_guarded_reads_resources_v1_tests.rs"]
mod resources;

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn fixture() -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let slice = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        op(
            4,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(5), Type::INDEX),
            ValueDef::new(ValueId(6), Type::BOOL),
            crate::CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(2),
        ),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(5),
                rhs: ValueId(4),
            },
        ),
        op(
            8,
            Type::BOOL,
            OperationKind::Unary {
                op: crate::UnaryOp::Not,
                operand: ValueId(6),
            },
        ),
        op(
            9,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(7),
                rhs: ValueId(8),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(9),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut read = BasicBlock::new(BlockId(20));
    read.operations = vec![
        op(
            10,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            11,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(10),
                offset: ValueId(5),
            },
        ),
        op(
            12,
            element,
            OperationKind::Load {
                pointer: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
    ];
    read.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(30));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("canonical-guarded-read");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![slice, Type::INDEX, Type::INDEX, Type::BOOL], vec![]),
        (0..4).map(ValueId).collect(),
        vec![entry, read, exit],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn coordinate(block: u32, operation: u32) -> Coordinate {
    Coordinate {
        block: BlockCoordinate {
            function: FunctionCoordinate(0),
            block,
        },
        operation,
    }
}
fn owner(module: Module) -> VerifiedCanonicalKernelIrModuleV12 {
    try_owner(&module).unwrap()
}
fn try_owner(
    module: &Module,
) -> std::result::Result<
    VerifiedCanonicalKernelIrModuleV12,
    crate::CanonicalKernelIrReplayAdmissionErrorV12,
> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let result = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        module,
        &mut budget,
    );
    assert_eq!(budget.storage(), 0);
    result.map(|(owner, _)| owner)
}
fn run<T>(
    owner: &VerifiedCanonicalKernelIrModuleV12,
    consume: impl for<'s> FnOnce(
        &CheckedCanonicalGuardedGlobalReadsV1<'s, '_>,
        &mut Budget<'_>,
    ) -> Result<T>,
) -> Result<T> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024 * 1024);
    budget.reserve_storage(17).unwrap();
    let result = with_canonical_guarded_global_reads_v1(
        owner,
        CanonicalGuardedGlobalReadLimitsV1::default(),
        &mut budget,
        consume,
    );
    assert_eq!(budget.storage(), 17);
    result
}

#[test]
fn actual_conjunction_has_same_slice_bound_and_own_false_overflow() {
    let graph = owner(fixture());
    run(&graph, |view, budget| {
        assert!(std::ptr::eq(view.owner(budget)?, &graph));
        assert_eq!(view.function_count(budget)?, 1);
        assert_eq!(
            view.function_effects(FunctionCoordinate(0), budget)?,
            (1, 0, 0)
        );
        let CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(read) =
            view.read_at(coordinate(1, 2), budget)?
        else {
            panic!("actual guarded read must be proved locally");
        };
        assert_eq!(read.operation(), coordinate(1, 2));
        assert_eq!(read.domain().slice(), ValueId(0));
        assert_eq!(read.domain().index(), ValueId(5));
        assert_eq!(read.domain().length(), ValueId(4));
        assert!(read.requires_runtime_allocation_binding());
        let no_wrap = view
            .no_wrap_at(coordinate(1, 2), coordinate(0, 1), budget)?
            .expect("exact checked Add");
        assert_eq!(no_wrap.value(), ValueId(5));
        assert_eq!(no_wrap.overflow(), ValueId(6));
        assert_eq!(no_wrap.operands(), (ValueId(1), ValueId(2)));
        assert!(!no_wrap.predicate().is_true());
        assert_eq!(no_wrap.predicate().edge(), (BlockId(10), 0, BlockId(20)));
        Ok(())
    })
    .unwrap();
}

#[test]
fn existing_direct_rule_has_identical_domain_and_conjunct_legacy_refusal_stays() {
    let mut direct = fixture();
    let Terminator::ConditionalBranch { condition, .. } =
        direct.functions[0].body.as_mut().unwrap().blocks[0]
            .terminator
            .as_mut()
            .unwrap()
    else {
        unreachable!()
    };
    *condition = ValueId(7);
    let legacy = derive_kernel_memory_obligations(
        &direct,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(legacy.is_complete());
    let [access] = legacy.obligations().accesses() else {
        panic!("one read");
    };
    let FormalAccessDomainV1::RuntimeSliceReadBounded(expected) = access.domain() else {
        panic!("runtime domain");
    };
    let graph = owner(direct);
    run(&graph, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(actual) =
            view.read_at(coordinate(1, 2), budget)?
        else {
            panic!("direct rule");
        };
        assert_eq!(*actual.domain(), expected);
        assert!(
            view.no_wrap_at(coordinate(1, 2), coordinate(0, 1), budget)?
                .is_none()
        );
        Ok(())
    })
    .unwrap();
    let legacy_conjunction = derive_kernel_memory_obligations(
        &fixture(),
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!legacy_conjunction.is_complete());
}

#[test]
fn not_not_and_duplicate_boolean_operands_preserve_local_facts() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.extend([
        op(
            13,
            Type::BOOL,
            OperationKind::Unary {
                op: crate::UnaryOp::Not,
                operand: ValueId(9),
            },
        ),
        op(
            14,
            Type::BOOL,
            OperationKind::Unary {
                op: crate::UnaryOp::Not,
                operand: ValueId(13),
            },
        ),
        op(
            15,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(14),
                rhs: ValueId(14),
            },
        ),
    ]);
    let Terminator::ConditionalBranch { condition, .. } =
        body.blocks[0].terminator.as_mut().unwrap()
    else {
        unreachable!()
    };
    *condition = ValueId(15);
    let graph = owner(module);
    run(&graph, |view, budget| {
        assert!(matches!(
            view.read_at(coordinate(1, 2), budget)?,
            CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(_)
        ));
        assert!(
            view.no_wrap_at(coordinate(1, 2), coordinate(0, 1), budget)?
                .is_some()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn shared_origin_scc_preserves_exact_seeded_unseeded_and_mixed_cases() {
    let seeded = BTreeMap::from([
        (ValueId(1), vec![ValueId(9), ValueId(2)]),
        (ValueId(2), vec![ValueId(1)]),
    ]);
    assert_eq!(
        origins::legacy(&seeded),
        BTreeMap::from([
            (ValueId(1), Some(ValueId(9))),
            (ValueId(2), Some(ValueId(9)))
        ])
    );
    let unseeded = BTreeMap::from([
        (ValueId(1), vec![ValueId(2)]),
        (ValueId(2), vec![ValueId(1)]),
    ]);
    assert_eq!(
        origins::legacy(&unseeded),
        BTreeMap::from([(ValueId(1), None), (ValueId(2), None)])
    );
    let mixed = BTreeMap::from([
        (ValueId(1), vec![ValueId(9), ValueId(2)]),
        (ValueId(2), vec![ValueId(1), ValueId(10)]),
    ]);
    assert_eq!(
        origins::legacy(&mixed),
        BTreeMap::from([(ValueId(1), None), (ValueId(2), None)])
    );
}

#[path = "canonical_guarded_reads_carrier_resources_v1_tests.rs"]
mod carrier_resources;
#[path = "canonical_guarded_reads_carriers_v1_tests.rs"]
mod carriers;
