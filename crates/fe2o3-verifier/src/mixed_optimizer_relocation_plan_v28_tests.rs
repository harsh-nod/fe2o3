use super::*;
#[path = "mixed_optimizer_relocation_cfg_v28_tests.rs"]
mod cfg_tests;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, ComparePredicate,
    Constant, Function, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    UnaryOp, ValueDef, ValueId,
};
const WORK: usize = 500_000_000;
const STORAGE: usize = 128 << 20;
const LAYOUTS: fe2o3_kernel_ir::StorageLayoutLimitsV1 = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
    rows: 0,
    edges: 0,
    containment_depth: 0,
    object_bytes: 0,
};
fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn branch(target: u32, values: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: values.iter().copied().map(ValueId).collect(),
    }
}
fn condition(value: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(value),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn block(id: u32, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(terminator);
    block
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn less(id: u32, lhs: u32, rhs: u32) -> Operation {
    op(
        id,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn increment(id: u32, value: u32) -> Operation {
    op(
        id,
        scalar(),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(value),
            rhs: ValueId(4),
        },
    )
}
fn fixture(nested: bool) -> Module {
    let mut entry = block(0, branch(1, &[3]));
    entry.operations = vec![
        op(3, scalar(), OperationKind::Constant(Constant::U32(0))),
        op(4, scalar(), OperationKind::Constant(Constant::U32(1))),
    ];
    let mut outer = block(1, condition(11, 2, if nested { 7 } else { 3 }));
    outer.parameters = vec![ValueDef::new(ValueId(10), scalar())];
    outer.operations = vec![less(11, 10, 0)];
    let invariant = if nested { 10 } else { 2 };
    let operations = vec![
        op(
            20,
            scalar(),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(invariant),
            },
        ),
        op(
            21,
            scalar(),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(20),
                rhs: ValueId(2),
            },
        ),
    ];
    let blocks = if nested {
        let preheader = block(2, branch(3, &[3]));
        let mut inner = block(3, condition(13, 4, 6));
        inner.parameters = vec![ValueDef::new(ValueId(12), scalar())];
        inner.operations = vec![less(13, 12, 1)];
        let mut body = block(4, branch(5, &[]));
        body.operations = operations;
        let mut inner_latch = block(5, branch(3, &[22]));
        inner_latch.operations = vec![increment(22, 12)];
        let mut outer_latch = block(6, branch(1, &[23]));
        outer_latch.operations = vec![increment(23, 10)];
        vec![
            entry,
            outer,
            preheader,
            inner,
            body,
            inner_latch,
            outer_latch,
            block(7, Terminator::Return { values: vec![] }),
        ]
    } else {
        let mut body = block(2, branch(1, &[22]));
        body.operations = operations;
        body.operations.push(increment(22, 10));
        vec![
            entry,
            outer,
            body,
            block(3, Terminator::Return { values: vec![] }),
        ]
    };
    let mut module = Module::new("dynamic-relocation-cut-bindings");
    module.functions.push(Function::internal_helper(
        "dynamic",
        Signature::new(vec![scalar(); 3], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        blocks,
    ));
    module
}
fn with_pair(nested: bool, run: impl FnOnce(&Pair<'_>, &mut Budget<'_>)) {
    with_pair_module(&fixture(nested), 2, run);
}
fn with_pair_module(module: &Module, moved: usize, run: impl FnOnce(&Pair<'_>, &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(29).unwrap();
    let (input, input_storage) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let tail = fe2o3_kernel_opt::prepare_owned_licm_v18(&input, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(tail.retained_storage()).unwrap();
    let (pair, pair_storage) = tail.replay_against(&input, &mut budget).unwrap();
    budget
        .reserve_storage(pair_storage.retained_storage())
        .unwrap();
    assert_eq!(
        pair.origins()
            .iter()
            .filter(|row| row.hoist.is_some())
            .count(),
        moved
    );
    run(&pair, &mut budget);
}

fn zero_operand_functions(functions: usize) -> Module {
    let mut module = fixture(false);
    let mut function = module.functions.pop().unwrap();
    function.body.as_mut().unwrap().blocks[2].operations = vec![
        op(20, scalar(), OperationKind::Constant(Constant::U32(7))),
        increment(22, 10),
    ];
    for ordinal in 0..functions {
        let mut copy = function.clone();
        copy.id = format!("dynamic_constant_{ordinal}").into();
        module.functions.push(copy);
    }
    module
}

#[test]
fn relocation_expression_multifunction_zero_operand_visits_remain_bounded() {
    for functions in [1, 4] {
        let module = zero_operand_functions(functions);
        with_pair_module(&module, functions, |pair, budget| {
            let held = budget.storage();
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(held).unwrap();
                let result = build(pair, &mut budget).and_then(|plan| {
                    assert_eq!(plan.nodes.len(), functions);
                    assert!(plan.operands.is_empty());
                    assert!(plan.nodes.iter().all(|node| node.operands.is_empty()));
                    plan.discard(&mut budget)
                });
                assert_eq!(budget.storage(), held);
                (result, budget.work(), budget.peak_storage())
            };
            let (result, work, storage) = run(WORK, STORAGE);
            result.unwrap();
            let (result, exact_work, exact_storage) = run(work, storage);
            result.unwrap();
            assert_eq!((exact_work, exact_storage), (work, storage));
            assert!(
                matches!(resource(run(work - 1, storage).0.unwrap_err()), Resource::Work(error) if error.limit() == work - 1 && error.actual() > error.limit())
            );
            assert!(
                matches!(resource(run(work, storage - 1).0.unwrap_err()), Resource::Storage(error) if error.limit() == storage - 1 && error.actual() > error.limit())
            );
        });
    }
}

#[test]
fn relocation_expression_input_visits_have_an_independent_cross_function_charge() {
    for functions in [1, 4] {
        with_pair_module(
            &zero_operand_functions(functions),
            functions,
            |pair, budget| {
                let floor = budget.storage();
                let plan = build(pair, budget).unwrap();
                assert_eq!(plan.nodes.len(), functions);
                assert!(plan.operands.is_empty());
                let (input, input_storage) = Inventory::derive_v18(pair.input(), budget).unwrap();
                budget
                    .reserve_storage(input_storage.retained_storage())
                    .unwrap();
                let (output, output_storage) =
                    Inventory::derive_v18(pair.output(), budget).unwrap();
                budget
                    .reserve_storage(output_storage.retained_storage())
                    .unwrap();
                assert_eq!(input.functions().len(), functions);
                assert!(input.functions().iter().all(|row| !row.blocks.is_empty()));
                let held = budget.storage();

                // Independently run only the CFG scopes. Zero-operand constants
                // need no dominance queries, so the real phase adds exactly one
                // two-unit target/filter visit per function/node pair, including
                // every foreign-function node. This is not a measured plan bound.
                let before = budget.work();
                for function in input.functions() {
                    with_flow(
                        pair.input(),
                        function.coordinate,
                        Default::default(),
                        budget,
                        |_, _| Ok::<(), Error>(()),
                    )
                    .unwrap();
                }
                let scopes = budget.work() - before;
                assert_eq!(budget.storage(), held);
                let before = budget.work();
                check::input_availability(&plan, pair, &input, &output, budget).unwrap();
                assert_eq!(budget.work() - before, scopes + 2 * functions * functions);
                assert_eq!(budget.storage(), held);

                drop((input, output));
                budget
                    .release_storage(
                        input_storage.retained_storage() + output_storage.retained_storage(),
                    )
                    .unwrap();
                plan.discard(budget).unwrap();
                assert_eq!(budget.storage(), floor);
            },
        );
    }
}

#[test]
fn relocation_expression_cuts_cover_dynamic_zero_trip_without_input_slot_equality() {
    with_pair(false, |pair, budget| {
        let floor = budget.storage();
        let plan = build(pair, budget).unwrap();
        assert_eq!(plan.nodes.len(), 2);
        assert_eq!(plan.order.len(), 2);
        assert!(
            plan.operands
                .iter()
                .any(|operand| operand.expression.is_some())
        );
        // Header and zero-trip exit need the hoisted expression even when the
        // original loop body has never defined either original result slot.
        for result in 0..plan.results.len() {
            assert!(plan.cuts.contains(&CutBinding { block: 1, result }));
            assert!(plan.cuts.contains(&CutBinding { block: 3, result }));
            assert!(!plan.cuts.contains(&CutBinding { block: 0, result }));
        }
        plan.replay(pair, budget).unwrap();
        plan.discard(budget).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn relocation_expression_nested_entry_refreshes_outer_variant_leaves() {
    with_pair(true, |pair, budget| {
        let floor = budget.storage();
        let plan = build(pair, budget).unwrap();
        for origin in pair.origins().iter().filter(|row| row.hoist.is_some()) {
            assert_eq!(
                origin.output.block.block, 2,
                "outer-carried operand cannot move before outer header"
            );
        }
        for result in 0..plan.results.len() {
            for block in [3, 4, 5, 6] {
                assert!(plan.cuts.contains(&CutBinding { block, result }));
            }
            for block in [0, 1, 2, 7] {
                assert!(!plan.cuts.contains(&CutBinding { block, result }));
            }
        }
        assert!(
            plan.operands
                .iter()
                .any(|operand| operand.expression.is_none())
        );
        plan.replay(pair, budget).unwrap();
        plan.discard(budget).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn relocation_expression_checker_rejects_missing_substituted_and_cyclic_rows() {
    with_pair(false, |pair, budget| {
        let floor = budget.storage();
        let mut plan = build(pair, budget).unwrap();
        let removed = plan.cuts.pop().unwrap();
        assert!(matches!(
            plan.replay(pair, budget),
            Err(Error::Mismatch("complete CFG-cut expression bindings"))
        ));
        plan.cuts.push(removed);
        let position = plan
            .operands
            .iter()
            .position(|operand| operand.expression.is_some())
            .unwrap();
        let operand = plan.operands[position];
        plan.operands[position].expression = None;
        assert!(matches!(
            plan.replay(pair, budget),
            Err(Error::Mismatch(
                "missing or substituted expression dependency"
            ))
        ));
        plan.operands[position] = operand;
        plan.order.swap(0, 1);
        assert!(matches!(
            plan.replay(pair, budget),
            Err(Error::Mismatch("acyclic complete expression order"))
        ));
        plan.order.swap(0, 1);
        let result = plan.results[0];
        plan.results[0].input += 1;
        assert!(matches!(
            plan.replay(pair, budget),
            Err(Error::Mismatch("exact moved result binding"))
        ));
        plan.results[0] = result;
        let cut = plan.cuts[0];
        plan.cuts[0].block = 0;
        assert!(matches!(
            plan.replay(pair, budget),
            Err(Error::Mismatch("complete CFG-cut expression bindings"))
        ));
        plan.cuts[0] = cut;
        plan.replay(pair, budget).unwrap();
        plan.discard(budget).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

fn resource(error: Error) -> Resource {
    match error {
        Error::Resource(error)
        | Error::Inventory(InventoryError::Resource(error))
        | Error::Flow(FlowError::Resource(error)) => error,
        other => panic!("not exact resource refusal: {other:?}"),
    }
}
#[test]
fn relocation_expression_plan_has_exact_and_one_short_work_and_storage() {
    for nested in [false, true] {
        with_pair(nested, |pair, original_budget| {
            let held = original_budget.storage();
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(held).unwrap();
                let result = build(pair, &mut budget).and_then(|plan| plan.discard(&mut budget));
                assert_eq!(budget.storage(), held);
                (result, budget.work(), budget.peak_storage())
            };
            let (result, work, storage) = run(WORK, STORAGE);
            result.unwrap();
            let (exact, actual_work, actual_storage) = run(work, storage);
            exact.unwrap();
            assert_eq!((actual_work, actual_storage), (work, storage));
            let (short, _, _) = run(work - 1, storage);
            assert!(
                matches!(resource(short.unwrap_err()), Resource::Work(error) if error.limit() == work - 1 && error.actual() > error.limit())
            );
            let (short, _, _) = run(work, storage - 1);
            assert!(
                matches!(resource(short.unwrap_err()), Resource::Storage(error) if error.limit() == storage - 1 && error.actual() > error.limit())
            );
        });
    }
}

#[test]
fn relocation_expression_plan_keeps_observed_floor_loss_sticky() {
    with_pair(false, |pair, budget| {
        let plan = build(pair, budget).unwrap();
        let required = budget.storage();
        budget.release_storage(1).unwrap();
        assert_eq!(
            plan.replay(pair, budget),
            Err(Error::Resource(Resource::Accounting))
        );
        budget.reserve_storage(1).unwrap();
        assert_eq!(
            plan.replay(pair, budget),
            Err(Error::Resource(Resource::Accounting))
        );
        assert_eq!(
            plan.discard(budget),
            Err(Error::Resource(Resource::Accounting))
        );
        assert_eq!(
            budget.storage(),
            required,
            "restored balance does not recreate custody"
        );
    });
}

#[test]
fn relocation_expression_plan_refuses_equal_byte_foreign_endpoints_and_foreign_ledger() {
    with_pair(false, |pair, budget| {
        let plan = build(pair, budget).unwrap();
        with_pair(false, |other, _| {
            assert_eq!(pair.input().identity(), other.input().identity());
            assert_eq!(pair.output().identity(), other.output().identity());
            assert_eq!(
                plan.replay(other, budget),
                Err(Error::Mismatch(
                    "foreign checked relocation endpoints or rows"
                ))
            );
        });
        plan.replay(pair, budget).unwrap();
        let required = budget.storage();
        let mut work = Work::new(WORK);
        let mut foreign = Budget::new(&mut work, STORAGE);
        foreign.reserve_storage(required).unwrap();
        assert_eq!(
            plan.replay(pair, &mut foreign),
            Err(Error::Resource(Resource::Accounting))
        );
        assert_eq!(foreign.storage(), required);
        assert_eq!(
            plan.replay(pair, budget),
            Err(Error::Resource(Resource::Accounting))
        );
        assert_eq!(
            plan.discard(budget),
            Err(Error::Resource(Resource::Accounting))
        );
        assert_eq!(budget.storage(), required);
    });
}

#[test]
fn relocation_expression_vector_quote_is_independent_exact_and_short() {
    let count = 3;
    let bytes = count * size_of::<usize>();
    for storage in [bytes, bytes - 1] {
        let mut work = Work::new(5);
        let mut budget = Budget::new(&mut work, storage);
        let mut paid = 0;
        let result = vector::<usize>(count, &mut budget, &mut paid);
        assert_eq!(budget.work(), 5);
        if storage == bytes {
            drop(result.unwrap());
            assert_eq!(paid, bytes);
            budget.release_storage(paid).unwrap();
        } else {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == bytes && error.limit() == bytes - 1)
            );
            assert_eq!(paid, 0);
        }
        assert_eq!(budget.storage(), 0);
    }
    let mut work = Work::new(4);
    let mut budget = Budget::new(&mut work, bytes);
    let mut paid = 0;
    assert!(
        matches!(vector::<usize>(count, &mut budget, &mut paid), Err(Error::Resource(Resource::Work(error))) if error.actual() == 5 && error.limit() == 4)
    );
    assert_eq!((budget.work(), budget.storage(), paid), (0, 0, 0));
}
