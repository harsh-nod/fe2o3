use super::*;
use crate::{
    BasicBlock, Kernel, Signature, StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutLimitsV1,
    StorageLayoutV1, StorageOperationV1, ValueDef,
};

#[path = "canonical_guarded_reads_v18_resource_tests.rs"]
mod resources;

#[path = "canonical_guarded_read_origins_v18_tests.rs"]
mod read_origins;

const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 8,
    edges: 16,
    containment_depth: 8,
    object_bytes: 1024,
};

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn fixture(storage: bool) -> Module {
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
    let mut module = Module::new("guarded-v18-owner");
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
    if storage {
        module.storage_layouts.push(StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        });
        let pointer = Type::pointer(
            Type::StorageObject(StorageLayoutIdV1(0)),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let mut body = BasicBlock::new(BlockId(40));
        body.operations = vec![
            op(
                1,
                Type::Scalar(ScalarType::U32),
                OperationKind::Storage(StorageOperationV1::ReadValue {
                    address: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::Storage(StorageOperationV1::WriteValue {
                    address: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new("external"),
                    arguments: vec![],
                },
            ),
        ];
        body.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::internal_helper(
            "storage",
            Signature::new(vec![pointer], vec![]),
            vec![ValueId(0)],
            vec![body],
        ));
        module.functions.push(Function::external_import(
            "external",
            Signature::new(vec![], vec![]),
        ));
    }
    module
}

fn coordinate(function: u32, block: u32, operation: u32) -> Coordinate {
    Coordinate {
        block: BlockCoordinate {
            function: FunctionCoordinate(function),
            block,
        },
        operation,
    }
}

fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LIMITS,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

fn run<T>(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    credit: usize,
    consume: impl for<'s> FnOnce(
        &CheckedCanonicalGuardedGlobalReadsV18<'s, '_>,
        &mut Budget<'_>,
    ) -> Result<T>,
) -> Result<T> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    budget.reserve_storage(credit + 17).unwrap();
    let result =
        with_canonical_guarded_global_reads_v18(owner, Default::default(), &mut budget, consume);
    assert_eq!(budget.storage(), credit + 17);
    result
}

#[test]
fn actual_v18_owner_keeps_guarded_read_predicate_and_checked_arithmetic_subjects() {
    for storage in [false, true] {
        let (graph, credit) = owner(&fixture(storage));
        run(&graph, credit, |view, budget| {
            assert!(std::ptr::eq(view.owner(budget)?, &graph));
            assert_eq!(view.function_count(budget)?, if storage { 3 } else { 1 });
            assert_eq!(
                view.function_effects(FunctionCoordinate(0), budget)?,
                (1, 0, 0)
            );
            let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
                view.read_at(coordinate(0, 1, 2), budget)?
            else {
                panic!("guarded read");
            };
            assert!(std::ptr::eq(read.owner(), &graph));
            assert_eq!(read.domain().slice(), ValueId(0));
            assert_eq!(read.domain().index(), ValueId(5));
            assert!(read.requires_runtime_allocation_binding());
            let predicate = view
                .true_at(coordinate(0, 1, 2), ValueId(7), budget)?
                .unwrap();
            assert!(std::ptr::eq(predicate.owner(), &graph));
            let fact = view
                .no_wrap_at(coordinate(0, 1, 2), coordinate(0, 0, 1), budget)?
                .unwrap();
            assert!(std::ptr::eq(fact.predicate().owner(), &graph));
            assert!(std::ptr::eq(
                fact.operation(),
                &graph.module().functions[0].body.as_ref().unwrap().blocks[0].operations[1]
            ));
            assert_eq!(fact.operands(), (ValueId(1), ValueId(2)));
            assert!(!fact.predicate().is_true());
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn storage_global_effects_and_unresolved_calls_are_not_ordinary_read_proofs() {
    let (graph, credit) = owner(&fixture(true));
    run(&graph, credit, |view, budget| {
        assert_eq!(
            view.function_effects(FunctionCoordinate(1), budget)?,
            (1, 1, 1)
        );
        assert_eq!(
            view.function_effects(FunctionCoordinate(2), budget)?,
            (0, 0, 0)
        );
        assert!(matches!(
            view.read_at(coordinate(1, 0, 0), budget)?,
            CanonicalGuardedGlobalReadOutcomeV18::NotProved(
                CanonicalGuardedGlobalReadReasonV1::NotOrdinaryGlobalRead
            )
        ));
        assert!(matches!(
            view.read_at(coordinate(1, 0, 1), budget)?,
            CanonicalGuardedGlobalReadOutcomeV18::NotProved(
                CanonicalGuardedGlobalReadReasonV1::NotOrdinaryGlobalRead
            )
        ));
        Ok(())
    })
    .unwrap();
}

#[test]
fn equal_bytes_foreign_owners_and_changed_unused_rows_do_not_rebind_facts() {
    let original = fixture(true);
    let (graph, credit) = owner(&original);
    let (foreign, _) = owner(&original);
    let mut changed = original;
    changed.storage_layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
    });
    let (changed, _) = owner(&changed);
    assert_eq!(graph.identity(), foreign.identity());
    assert_ne!(graph.identity(), changed.identity());
    run(&graph, credit, |view, budget| {
        let actual = view.owner(budget)?;
        assert!(std::ptr::eq(actual, &graph));
        assert!(!std::ptr::eq(actual, &foreign));
        assert!(!std::ptr::eq(actual, &changed));
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 2), budget)?
        else {
            panic!("guarded read");
        };
        assert!(std::ptr::eq(read.owner(), &graph));
        Ok(())
    })
    .unwrap();
}

#[test]
fn owner_generic_representation_preserves_v12_header_and_fact_sizes() {
    assert_eq!(
        size_of::<Facts<'_>>(),
        size_of::<Facts<'_, VerifiedCanonicalKernelIrModuleV18>>()
    );
    assert_eq!(
        size_of::<CheckedCanonicalGuardedGlobalReadsV1<'_, '_>>(),
        size_of::<CheckedCanonicalGuardedGlobalReadsV18<'_, '_>>()
    );
    assert_eq!(
        size_of::<CanonicalGuardedGlobalReadFactV1<'_, '_>>(),
        size_of::<CanonicalGuardedGlobalReadFactV18<'_, '_>>()
    );
    assert_eq!(
        size_of::<CanonicalGuardedPredicateFactV1<'_, '_>>(),
        size_of::<CanonicalGuardedPredicateFactV18<'_, '_>>()
    );
    assert_eq!(
        size_of::<CanonicalGuardedNoWrapFactV1<'_, '_>>(),
        size_of::<CanonicalGuardedNoWrapFactV18<'_, '_>>()
    );
}
