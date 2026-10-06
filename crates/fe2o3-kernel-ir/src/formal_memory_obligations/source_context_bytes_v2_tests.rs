use super::*;
use crate::{BasicBlock, FunctionBody, FunctionRole, Signature, ValueDef};

const FLOOR: usize = 13;

fn block(id: u32, parameters: Vec<ValueDef>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        parameters,
        operations: vec![],
        terminator: Some(terminator),
    }
}

fn bare() -> Function {
    Function {
        id: "source_context".into(),
        signature: Signature::new(vec![], vec![]),
        role: FunctionRole::InternalHelper,
        body: Some(FunctionBody {
            parameters: vec![],
            blocks: vec![block(0, vec![], Terminator::Return { values: vec![] })],
        }),
        required_capabilities: Default::default(),
    }
}

fn seeded_loop() -> Function {
    let mut source = bare();
    source.signature.parameters = vec![Type::INDEX, Type::BOOL];
    let body = source.body.as_mut().unwrap();
    body.parameters = vec![ValueId(0), ValueId(1)];
    body.blocks = vec![
        block(
            0,
            vec![],
            Terminator::Branch {
                target: BlockId(1),
                arguments: vec![ValueId(0)],
            },
        ),
        block(
            1,
            vec![ValueDef::new(ValueId(2), Type::INDEX)],
            Terminator::ConditionalBranch {
                condition: ValueId(1),
                then_target: BlockId(1),
                then_arguments: vec![ValueId(2)],
                else_target: BlockId(2),
                else_arguments: vec![ValueId(2)],
            },
        ),
        block(
            2,
            vec![ValueDef::new(ValueId(3), Type::INDEX)],
            Terminator::Return { values: vec![] },
        ),
        block(
            3,
            vec![ValueDef::new(ValueId(4), Type::INDEX)],
            Terminator::Branch {
                target: BlockId(2),
                arguments: vec![ValueId(4)],
            },
        ),
    ];
    for (index, value) in [(0, 10), (3, 11)] {
        body.blocks[index].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(value), Type::INDEX),
            OperationKind::Constant(Constant::Index(7)),
        ));
    }
    source
}

fn ambiguous_join() -> Function {
    let mut source = seeded_loop();
    let body = source.body.as_mut().unwrap();
    body.blocks.truncate(4);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    body.blocks[1].parameters.clear();
    body.blocks[2].parameters.clear();
    body.blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(0)],
    });
    body.blocks[2].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(10)],
    });
    body.blocks[3].terminator = Some(Terminator::Return { values: vec![] });
    source
}

#[test]
fn byte_source_context_reuses_original_phi_scc_and_reachable_definition_semantics() {
    for source in [seeded_loop(), ambiguous_join()] {
        let mut module = Module::new("source_context_oracle");
        module.functions.push(source.clone());
        crate::verify_module(&module).unwrap();
        let (legacy, _) = collect_definitions(&source).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut context =
            ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
                .unwrap()
                .unwrap();
        for value in [0, 1, 2, 3, 4, 10, 11, 99, u32::MAX] {
            assert_eq!(
                context
                    .unique_origin(&source, ValueId(value), &mut budget)
                    .unwrap(),
                legacy.unique_ssa_origin(ValueId(value))
            );
            let actual = context
                .operation(&source, ValueId(value), &mut budget)
                .unwrap();
            let expected = legacy.operations.get(&ValueId(value)).copied();
            assert_eq!(
                actual.map(|(op, site)| (op as *const _, site)),
                expected.map(|(op, site)| (op as *const _, site))
            );
        }
        context.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn byte_source_context_borrows_original_types_and_excludes_unreachable_operation_rows() {
    let source = seeded_loop();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut context = ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(
        context
            .unique_origin(&source, ValueId(2), &mut budget)
            .unwrap(),
        Some(ValueId(0))
    );
    assert_eq!(
        context
            .unique_origin(&source, ValueId(3), &mut budget)
            .unwrap(),
        Some(ValueId(0))
    );
    assert!(
        context
            .operation(&source, ValueId(11), &mut budget)
            .unwrap()
            .is_none()
    );
    let body = source.body.as_ref().unwrap();
    assert!(std::ptr::eq(
        context
            .value_type(&source, ValueId(11), &mut budget)
            .unwrap()
            .unwrap(),
        &body.blocks[3].operations[0].results[0].ty
    ));
    assert!(std::ptr::eq(
        context
            .value_type(&source, ValueId(0), &mut budget)
            .unwrap()
            .unwrap(),
        &source.signature.parameters[0]
    ));
    let (operation, location) = context
        .operation(&source, ValueId(10), &mut budget)
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(operation, &body.blocks[0].operations[0]));
    assert_eq!(location, FunctionOperationLocation::new(BlockId(0), 0));
    context.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn byte_source_context_entry_parameters_never_gain_a_backedge_only_origin() {
    let mut source = bare();
    let body = source.body.as_mut().unwrap();
    body.blocks[0]
        .parameters
        .push(ValueDef::new(ValueId(5), Type::INDEX));
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![ValueId(5)],
    });
    let (legacy, _) = collect_definitions(&source).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut context = ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(legacy.unique_ssa_origin(ValueId(5)), None);
    assert_eq!(
        context
            .unique_origin(&source, ValueId(5), &mut budget)
            .unwrap(),
        None
    );
    assert!(
        context
            .operation(&source, ValueId(5), &mut budget)
            .unwrap()
            .is_none()
    );
    context.release(&mut budget).unwrap();
}

#[test]
fn byte_source_context_empty_phi_function_has_independent_exact_work() {
    // Constructor1 + byteCFG106 + source-index8 + original census(2+4)
    // + shared empty SCC40 + final output reserve2 =163. No measured oracle.
    const WORK: usize = 163;
    let source = bare();
    for limit in [WORK - 1, WORK] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let result = ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget);
        if limit == WORK {
            let context = result.unwrap().unwrap();
            assert_eq!(budget.work(), WORK);
            assert!(context.origins.is_empty());
            context.release(&mut budget).unwrap();
        } else {
            assert!(
                matches!(result, Err(SourceContextErrorV2::Resource(ResourceError::Work(error))) if error.actual() == WORK && error.limit() == WORK - 1)
            );
            assert_eq!(budget.failed_work(), Some(WORK));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn byte_source_context_measured_full_phase_exact_and_short_boundaries_keep_original_floor() {
    for source in [bare(), seeded_loop(), ambiguous_join()] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let context = ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
            .unwrap()
            .unwrap();
        let needed = (budget.work(), budget.peak_storage(), budget.storage());
        context.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        for short in [None, Some(false), Some(true)] {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(needed.0 - usize::from(short == Some(false)));
            let mut budget = Budget::new(&mut work, needed.1 - usize::from(short == Some(true)));
            budget.reserve_storage(FLOOR).unwrap();
            let result =
                ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget);
            if let Some(storage) = short {
                let error = result.err().unwrap();
                if storage {
                    assert!(matches!(
                        error,
                        SourceContextErrorV2::Resource(ResourceError::Storage { .. })
                    ));
                    assert!(budget.failed_storage().is_some());
                } else {
                    assert!(matches!(
                        error,
                        SourceContextErrorV2::Resource(ResourceError::Work(_))
                    ));
                    assert!(budget.failed_work().is_some());
                }
                let before = budget.work();
                assert_eq!(
                    ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
                        .err()
                        .unwrap(),
                    error
                );
                assert_eq!(budget.work(), before);
            } else {
                let context = result.unwrap().unwrap();
                assert_eq!(
                    (budget.work(), budget.peak_storage(), budget.storage()),
                    needed
                );
                context.release(&mut budget).unwrap();
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn byte_source_context_latches_same_ledger_source_or_floor_refusals_after_restoration() {
    let source = seeded_loop();
    let foreign = source.clone();
    for undercut in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let mut context =
            ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
                .unwrap()
                .unwrap();
        let retained = budget.storage();
        if undercut {
            budget.rollback_storage(FLOOR).unwrap();
        }
        let accepted = budget.work();
        let error = context
            .unique_origin(
                if undercut { &source } else { &foreign },
                ValueId(2),
                &mut budget,
            )
            .unwrap_err();
        assert_eq!(
            error,
            SourceContextErrorV2::Resource(ResourceError::Accounting)
        );
        if undercut {
            budget.reserve_storage(retained - FLOOR).unwrap();
        }
        assert_eq!(
            context
                .unique_origin(&source, ValueId(2), &mut budget)
                .unwrap_err(),
            error
        );
        assert_eq!(
            context
                .value_type(&source, ValueId(0), &mut budget)
                .unwrap_err(),
            error
        );
        assert_eq!(budget.work(), accepted);
        context.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn byte_source_context_foreign_ledger_does_not_poison_or_refund_original_custody() {
    let source = seeded_loop();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let mut context = ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
        .unwrap()
        .unwrap();
    let original = (budget.work(), budget.storage());
    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut other = Budget::new(&mut other_work, usize::MAX);
    other.reserve_storage(41).unwrap();
    assert_eq!(
        context
            .unique_origin(&source, ValueId(2), &mut other)
            .unwrap_err(),
        SourceContextErrorV2::Resource(ResourceError::Accounting)
    );
    assert_eq!((other.work(), other.storage()), (0, 41));
    assert_eq!((budget.work(), budget.storage()), original);
    assert_eq!(
        context
            .unique_origin(&source, ValueId(2), &mut budget)
            .unwrap(),
        Some(ValueId(0))
    );
    budget.reserve_storage(29).unwrap();
    context.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR + 29);
}

#[test]
fn byte_source_context_declarations_and_bad_cfgs_keep_source_and_floor_intact() {
    let mut declaration = bare();
    declaration.body = None;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(
        ByteSourceContextV2::build(&declaration, ControlFlowLimits::DEFAULT, &mut budget)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, FLOOR, FLOOR)
    );
    for mode in 0..3 {
        let mut source = bare();
        let body = source.body.as_mut().unwrap();
        match mode {
            0 => body.blocks.clear(),
            1 => body.blocks[0].terminator = None,
            _ => {
                body.blocks[0].terminator = Some(Terminator::Branch {
                    target: BlockId(17),
                    arguments: vec![],
                })
            }
        }
        let unchanged = source.clone();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(matches!(
            ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget),
            Err(SourceContextErrorV2::ControlFlow(_))
        ));
        assert_eq!(source, unchanged);
        assert_eq!(budget.storage(), FLOOR);
    }
}
