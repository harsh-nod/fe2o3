use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as Function, Function as KirFunction, Module,
    Operation as KirOperation, Signature, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 43;
fn block(block: u32) -> Block {
    Block {
        function: Function(0),
        block,
    }
}
fn branch(target: u32, arguments: Vec<ValueId>) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments,
    }
}
fn basic(
    id: u32,
    parameters: Vec<ValueDef>,
    operations: Vec<KirOperation>,
    terminator: Terminator,
) -> BasicBlock {
    let mut b = BasicBlock::new(BlockId(id));
    b.parameters = parameters;
    b.operations = operations;
    b.terminator = Some(terminator);
    b
}
fn module(blocks: Vec<BasicBlock>, parameters: Vec<Type>, values: Vec<ValueId>) -> Module {
    let mut module = Module::new("canonical-loops");
    module.functions.push(KirFunction::internal_helper(
        "f",
        Signature::new(parameters, vec![]),
        values,
        blocks,
    ));
    module
}
fn fixture(step: Constant, checked: bool, reversed: bool) -> Module {
    let scalar = step.ty();
    let lhs = ValueId(if reversed { 3 } else { 2 });
    let rhs = ValueId(if reversed { 2 } else { 3 });
    let add = if checked {
        KirOperation::checked_binary(
            ValueDef::new(ValueId(4), scalar.clone()),
            ValueDef::new(ValueId(5), Type::BOOL),
            CheckedBinaryOperator::Add,
            lhs,
            rhs,
        )
    } else {
        KirOperation::effect_free(
            ValueDef::new(ValueId(4), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs,
                rhs,
            },
        )
    };
    module(
        vec![
            basic(
                90,
                vec![],
                vec![KirOperation::effect_free(
                    ValueDef::new(ValueId(3), scalar.clone()),
                    OperationKind::Constant(step),
                )],
                branch(11, vec![ValueId(0)]),
            ),
            basic(
                11,
                vec![ValueDef::new(ValueId(2), scalar.clone())],
                vec![],
                Terminator::ConditionalBranch {
                    condition: ValueId(1),
                    then_target: BlockId(70),
                    then_arguments: vec![],
                    else_target: BlockId(100),
                    else_arguments: vec![],
                },
            ),
            basic(70, vec![], vec![add], branch(11, vec![ValueId(4)])),
            basic(100, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
        vec![scalar, Type::BOOL],
        vec![ValueId(0), ValueId(1)],
    )
}
fn with_inventory(module: Module, run: impl FnOnce(&Inventory<'_>, &mut Budget<'_>)) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, owner_bytes) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget)
            .expect("fixture independently verifies");
    budget
        .reserve_storage(owner_bytes.retained_storage())
        .unwrap();
    let (inventory, inventory_bytes) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_bytes.retained_storage())
        .unwrap();
    let floor = budget.storage();
    run(&inventory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_bytes.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_bytes.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
fn with_report(
    module: Module,
    run: impl FnOnce(&mut CanonicalKirLoopsV1<'_, '_>, &mut Budget<'_>),
) {
    with_inventory(module, |inventory, budget| {
        let floor = budget.storage();
        let (mut report, receipt) =
            CanonicalKirLoopsV1::derive(inventory, Default::default(), budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            receipt.retained_storage(),
            size_of::<CanonicalKirLoopsV1<'_, '_>>()
                + bytes::<NaturalLoop>(report.loops.capacity()).unwrap()
                + bytes::<Block>(report.members.capacity()).unwrap()
                + bytes::<Edge>(report.edges.capacity()).unwrap()
                + bytes::<Recurrence>(report.recurrences.capacity()).unwrap()
        );
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        report
            .replay(inventory, Default::default(), budget)
            .unwrap();
        run(&mut report, budget);
        drop(report);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn fixed_integer_widths_directions_and_checked_overflow_are_exact_typed_relations() {
    for step in [
        Constant::I8(-1),
        Constant::U8(255),
        Constant::I16(-3),
        Constant::U16(32768),
        Constant::I32(i32::MIN),
        Constant::U32(u32::MAX),
        Constant::I64(i64::MIN),
        Constant::U64(u64::MAX),
    ] {
        for checked in [false, true] {
            for reversed in [false, true] {
                let expected = fixed_integer_bits(&step).unwrap();
                with_report(
                    fixture(step.clone(), checked, reversed),
                    |report, budget| {
                        assert!(report.belongs_to(report.inventory()));
                        assert_eq!(report.loop_count(), 1);
                        assert_eq!(report.members(0, budget).unwrap(), &[block(1), block(2)]);
                        let loop_ = report.natural_loop(0, budget).unwrap();
                        assert!(loop_.is_single_entry());
                        assert!(loop_.has_dedicated_exits());
                        assert_eq!(
                            loop_.unconditional_preheader(),
                            Some(Edge {
                                source: block(0),
                                successor: 0
                            })
                        );
                        let recurrence = report.recurrences(0, budget).unwrap()[0];
                        assert_eq!((recurrence.scalar(), recurrence.step_bits()), expected);
                        assert_eq!(recurrence.parameter_operand(), u8::from(reversed));
                        assert_eq!(
                            recurrence.initial(),
                            Definition::FunctionArgument {
                                function: Function(0),
                                argument: 0
                            }
                        );
                        assert_eq!(recurrence.overflow().is_some(), checked);
                    },
                );
            }
        }
    }
}
#[test]
fn zero_step_index_nonadd_and_forwarded_header_value_are_nonmatches_not_proofs() {
    for case in 0..4 {
        let mut module = fixture(
            if case == 0 {
                Constant::U32(0)
            } else if case == 1 {
                Constant::Index(1)
            } else {
                Constant::U32(1)
            },
            false,
            false,
        );
        let body = module.functions[0].body.as_mut().unwrap();
        if case == 2
            && let OperationKind::Binary { op, .. } = &mut body.blocks[2].operations[0].kind
        {
            *op = BinaryOp::Subtract;
        }
        if case == 3 {
            body.blocks[2].terminator = Some(branch(11, vec![ValueId(2)]));
        }
        with_report(module, |report, budget| {
            assert_eq!(report.loop_count(), 1);
            assert!(report.recurrences(0, budget).unwrap().is_empty());
        });
    }
}
#[test]
fn a_cast_of_a_literal_is_not_silently_promoted_to_an_exact_step() {
    let mut source = fixture(Constant::U32(1), false, false);
    let body = source.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations[0] = KirOperation::effect_free(
        ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U16)),
        OperationKind::Constant(Constant::U16(1)),
    );
    body.blocks[0].operations.push(KirOperation::effect_free(
        ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
        OperationKind::Cast {
            kind: fe2o3_kernel_ir::CastKind::ZeroExtend,
            value: ValueId(3),
            to: Type::Scalar(ScalarType::U32),
        },
    ));
    if let OperationKind::Binary { rhs, .. } = &mut body.blocks[2].operations[0].kind {
        *rhs = ValueId(6);
    }
    with_report(source, |report, budget| {
        assert_eq!(report.loop_count(), 1);
        assert!(report.recurrences(0, budget).unwrap().is_empty());
    });
}
#[test]
fn entry_self_loop_is_described_but_has_no_fabricated_external_initializer() {
    with_report(
        module(
            vec![basic(55, vec![], vec![], branch(55, vec![]))],
            vec![],
            vec![],
        ),
        |report, budget| {
            assert_eq!(report.loop_count(), 1);
            assert_eq!(report.members(0, budget).unwrap(), &[block(0)]);
            assert!(!report.natural_loop(0, budget).unwrap().is_single_entry());
            assert!(report.external_header_edges(0, budget).unwrap().is_empty());
            assert!(report.recurrences(0, budget).unwrap().is_empty());
        },
    );
}
#[test]
fn diamond_and_distinct_latches_keep_complete_membership_and_edge_multiplicity() {
    for split_latch in [false, true] {
        let cond = |yes, no| Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(yes),
            then_arguments: vec![],
            else_target: BlockId(no),
            else_arguments: vec![],
        };
        with_report(
            module(
                vec![
                    basic(90, vec![], vec![], branch(10, vec![])),
                    basic(10, vec![], vec![], cond(20, 100)),
                    basic(20, vec![], vec![], cond(30, 40)),
                    basic(
                        30,
                        vec![],
                        vec![],
                        branch(if split_latch { 10 } else { 50 }, vec![]),
                    ),
                    basic(40, vec![], vec![], branch(50, vec![])),
                    basic(50, vec![], vec![], branch(10, vec![])),
                    basic(100, vec![], vec![], Terminator::Return { values: vec![] }),
                ],
                vec![Type::BOOL],
                vec![ValueId(0)],
            ),
            |report, budget| {
                assert_eq!(report.loop_count(), 1);
                assert_eq!(
                    report.members(0, budget).unwrap(),
                    &[block(1), block(2), block(3), block(4), block(5)]
                );
                assert_eq!(
                    report.latch_edges(0, budget).unwrap().len(),
                    if split_latch { 2 } else { 1 }
                );
            },
        );
    }
}
#[test]
fn checked_overflow_result_keeps_its_live_use_and_no_wrap_is_not_inferred() {
    let mut source = fixture(Constant::U32(u32::MAX), true, false);
    source.functions[0].body.as_mut().unwrap().blocks[2]
        .operations
        .push(KirOperation::effect_free(
            ValueDef::new(ValueId(6), Type::BOOL),
            OperationKind::Unary {
                op: fe2o3_kernel_ir::UnaryOp::Not,
                operand: ValueId(5),
            },
        ));
    with_report(source, |report, budget| {
        let overflow = report.recurrences(0, budget).unwrap()[0]
            .overflow()
            .unwrap();
        assert_eq!(
            overflow,
            Definition::Result {
                operation: Operation {
                    block: block(2),
                    operation: 0
                },
                result: 1
            }
        );
        assert!(
            report
                .inventory()
                .uses()
                .iter()
                .any(|u| report.inventory().definitions()[u.definition].coordinate == overflow)
        );
    });
}
#[test]
fn duplicate_backedges_and_missing_unconditional_preheader_remain_distinct() {
    for duplicate in [false, true] {
        let mut source = fixture(Constant::U32(1), false, false);
        let body = source.functions[0].body.as_mut().unwrap();
        if duplicate {
            body.blocks[2].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(1),
                then_target: BlockId(11),
                then_arguments: vec![ValueId(4)],
                else_target: BlockId(11),
                else_arguments: vec![ValueId(4)],
            });
        } else {
            body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(1),
                then_target: BlockId(11),
                then_arguments: vec![ValueId(0)],
                else_target: BlockId(100),
                else_arguments: vec![],
            });
        }
        with_report(source, |report, budget| {
            assert_eq!(report.loop_count(), 1);
            assert!(report.recurrences(0, budget).unwrap().is_empty());
            assert_eq!(
                report.latch_edges(0, budget).unwrap().len(),
                if duplicate { 2 } else { 1 }
            );
            if !duplicate {
                assert_eq!(
                    report
                        .natural_loop(0, budget)
                        .unwrap()
                        .unconditional_preheader(),
                    None
                );
                assert!(
                    !report
                        .natural_loop(0, budget)
                        .unwrap()
                        .has_dedicated_exits()
                );
            }
        });
    }
}
#[test]
fn nested_loops_multiple_functions_and_disconnected_cycles_preserve_canonical_order() {
    let cond = |yes, no| Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    };
    let mut source = module(
        vec![
            basic(40, vec![], vec![], branch(10, vec![])),
            basic(10, vec![], vec![], cond(20, 90)),
            basic(20, vec![], vec![], cond(30, 50)),
            basic(30, vec![], vec![], branch(20, vec![])),
            basic(50, vec![], vec![], branch(10, vec![])),
            basic(90, vec![], vec![], Terminator::Return { values: vec![] }),
            basic(100, vec![], vec![], branch(100, vec![])),
        ],
        vec![Type::BOOL],
        vec![ValueId(0)],
    );
    let mut second = source.functions[0].clone();
    second.id = "g".into();
    source.functions.push(second);
    with_report(source, |report, budget| {
        assert_eq!(report.loop_count(), 4);
        assert_eq!(
            report.members(0, budget).unwrap(),
            &[block(1), block(2), block(3), block(4)]
        );
        assert_eq!(report.members(1, budget).unwrap(), &[block(2), block(3)]);
        assert_eq!(
            report.natural_loop(2, budget).unwrap().header().function,
            Function(1)
        );
    });
}
#[test]
fn irreducible_two_entry_cycle_is_not_fabricated_as_a_natural_recurrence() {
    let cond = |yes, no| Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    };
    with_report(
        module(
            vec![
                basic(0, vec![], vec![], cond(1, 2)),
                basic(1, vec![], vec![], branch(2, vec![])),
                basic(2, vec![], vec![], cond(1, 3)),
                basic(3, vec![], vec![], Terminator::Return { values: vec![] }),
            ],
            vec![Type::BOOL],
            vec![ValueId(0)],
        ),
        |report, _| assert_eq!(report.loop_count(), 0),
    );
}
#[test]
fn outer_backedge_equation_does_not_claim_its_inner_region_is_reducible_or_progresses() {
    let cond = |yes, no| Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    };
    let scalar = Type::Scalar(ScalarType::U32);
    let source = module(
        vec![
            basic(
                90,
                vec![],
                vec![KirOperation::effect_free(
                    ValueDef::new(ValueId(3), scalar.clone()),
                    OperationKind::Constant(Constant::U32(1)),
                )],
                branch(10, vec![ValueId(0)]),
            ),
            basic(
                10,
                vec![ValueDef::new(ValueId(2), scalar.clone())],
                vec![],
                cond(20, 100),
            ),
            basic(20, vec![], vec![], cond(30, 40)),
            basic(30, vec![], vec![], branch(40, vec![])),
            basic(40, vec![], vec![], cond(30, 50)),
            basic(
                50,
                vec![],
                vec![KirOperation::effect_free(
                    ValueDef::new(ValueId(4), scalar.clone()),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(2),
                        rhs: ValueId(3),
                    },
                )],
                branch(10, vec![ValueId(4)]),
            ),
            basic(100, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
        vec![scalar, Type::BOOL],
        vec![ValueId(0), ValueId(1)],
    );
    with_report(source, |report, budget| {
        assert_eq!(report.loop_count(), 1);
        assert_eq!(
            report.members(0, budget).unwrap(),
            &[block(1), block(2), block(3), block(4), block(5)]
        );
        assert_eq!(report.recurrences(0, budget).unwrap().len(), 1);
        assert_eq!(
            report.latch_edges(0, budget).unwrap(),
            &[Edge {
                source: block(5),
                successor: 0
            }]
        );
        with_canonical_kir_control_flow_v1(
            report.inventory().owner(),
            Function(0),
            Default::default(),
            budget,
            |view, budget| {
                assert!(!view.is_reducible(budget)?);
                Ok::<_, Error>(())
            },
        )
        .unwrap();
    });
}
#[test]
fn equal_content_foreign_inventory_is_not_an_owner_join() {
    with_report(fixture(Constant::U32(1), false, false), |report, budget| {
        let (other, receipt) = Inventory::derive(report.inventory().owner(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(
            report.replay(&other, Default::default(), budget),
            Err(Error::ForeignInventory)
        );
        drop(other);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}
#[test]
fn equal_content_distinct_verified_owner_is_rejected_before_replay() {
    with_report(fixture(Constant::U32(1), false, false), |report, _| {
        with_inventory(fixture(Constant::U32(1), false, false), |other, budget| {
            assert!(!std::ptr::eq(report.inventory().owner(), other.owner()));
            assert_eq!(
                report.replay(other, Default::default(), budget),
                Err(Error::ForeignInventory)
            );
        });
    });
}
#[test]
fn independent_replay_rejects_omitted_extra_and_changed_loop_edge_member_and_recurrence_rows() {
    for mutation in 0..17 {
        with_report(fixture(Constant::I32(-1), true, false), |report, budget| {
            match mutation {
                0 => report.loops.clear(),
                1 => report.members.pop().map(|_| ()).unwrap(),
                2 => report.members[0] = block(0),
                3 => report.edges[0].successor = 1,
                4 => report.loops[0].single_entry = false,
                5 => report.loops[0].dedicated_exits = false,
                6 => report.recurrences.clear(),
                7 => report.recurrences[0].step_bits = 1,
                8 => report.recurrences[0].scalar = ScalarType::U32,
                9 => report.recurrences[0].parameter_operand = 1,
                10 => report.recurrences[0].overflow = None,
                11 => report.recurrences[0].initial = report.recurrences[0].update,
                12 => report.recurrences[0].parameter = report.recurrences[0].step,
                13 => report.recurrences[0].update = report.recurrences[0].initial,
                14 => report.recurrences[0].step = report.recurrences[0].parameter,
                15 => report.recurrences[0].backedge = report.recurrences[0].initial_edge,
                _ => report.loops[0].recurrences.end += 1,
            }
            assert_eq!(
                report.replay(report.inventory(), Default::default(), budget),
                Err(Error::ReplayMismatch)
            );
        });
    }
}
#[test]
fn independent_replay_rejects_actual_appended_loop_member_edge_and_recurrence_rows() {
    for extra in 0..4 {
        with_inventory(
            fixture(Constant::I32(-1), true, false),
            |inventory, budget| {
                let inherited = budget.storage();
                let (mut report, receipt) =
                    CanonicalKirLoopsV1::derive(inventory, Default::default(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let original_floor = budget.storage();
                // These private hostile fixtures use the same prepaid growth helper
                // as production, including actual capacity and old/new coexistence.
                match extra {
                    0 => {
                        let row = report.loops[0].clone();
                        push(&mut report.loops, row, 2, budget).unwrap();
                    }
                    1 => {
                        let row = report.members[0];
                        push(&mut report.members, row, 3, budget).unwrap();
                    }
                    2 => {
                        let row = report.edges[0];
                        push(&mut report.edges, row, 4, budget).unwrap();
                    }
                    _ => {
                        let row = report.recurrences[0];
                        push(&mut report.recurrences, row, 2, budget).unwrap();
                    }
                }
                let added = budget.storage().checked_sub(original_floor).unwrap();
                let owned = size_of::<CanonicalKirLoopsV1<'_, '_>>()
                    + bytes::<NaturalLoop>(report.loops.capacity()).unwrap()
                    + bytes::<Block>(report.members.capacity()).unwrap()
                    + bytes::<Edge>(report.edges.capacity()).unwrap()
                    + bytes::<Recurrence>(report.recurrences.capacity()).unwrap();
                assert_eq!(owned, receipt.retained_storage() + added);
                assert_eq!(
                    report.replay(inventory, Default::default(), budget),
                    Err(Error::ReplayMismatch)
                );
                assert_eq!(budget.storage(), original_floor + added);
                drop(report);
                budget.release_storage(owned).unwrap();
                assert_eq!(budget.storage(), inherited);
            },
        );
    }
}

#[test]
fn exact_and_one_short_work_storage_and_row_caps_fail_without_empty_success() {
    with_inventory(fixture(Constant::U64(1), true, false), |inventory, _| {
        let execute = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = CanonicalKirLoopsV1::derive(inventory, Default::default(), &mut budget);
            let summary = result.map(|(report, receipt)| {
                assert!(receipt.retained_storage() > 0);
                report.loop_count()
            });
            assert_eq!(budget.storage(), FLOOR);
            (summary, budget.work(), budget.peak_storage())
        };
        let (result, work, peak) = execute(LIMIT, LIMIT);
        assert_eq!(result, Ok(1));
        assert_eq!(execute(work, peak).0, Ok(1));
        assert!(matches!(
            execute(work - 1, peak).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            execute(work, peak - 1).0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let limits = CanonicalKirLoopLimitsV1 {
            rows: 5,
            ..Default::default()
        };
        assert!(matches!(
            CanonicalKirLoopsV1::derive(inventory, limits, &mut budget),
            Err(Error::InputLimit {
                kind: "retained rows",
                ..
            })
        ));
        assert_eq!(budget.storage(), FLOOR);
    });
}
#[test]
fn independent_replay_has_its_own_exact_and_one_short_work_storage_budget() {
    with_report(fixture(Constant::U8(1), false, false), |report, _| {
        let execute = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = report.replay(report.inventory(), Default::default(), &mut budget);
            assert_eq!(budget.storage(), FLOOR);
            (result, budget.work(), budget.peak_storage())
        };
        let (result, work, peak) = execute(LIMIT, LIMIT);
        assert_eq!(result, Ok(()));
        assert_eq!(execute(work, peak).0, Ok(()));
        assert!(matches!(
            execute(work - 1, peak).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            execute(work, peak - 1).0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
    });
}
#[test]
fn private_allocation_scope_restores_floor_on_error_and_panic_without_rewinding_history() {
    for panic in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result: Result<()> = scoped(&mut budget, |budget| {
            let _table = filled(256, 3u64, budget)?;
            if panic {
                panic!("test allocation panic");
            } else {
                Err(Error::ReplayMismatch)
            }
        });
        assert_eq!(
            result,
            Err(if panic {
                Error::Panicked
            } else {
                Error::ReplayMismatch
            })
        );
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.peak_storage() >= FLOOR + 2048);
        assert!(budget.work() >= 258);
    }
}
#[test]
fn capacity_excess_exact_one_short_and_inconsistent_capacity_keep_accepted_prefix() {
    for limit in [FLOOR + 80, FLOOR + 79] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR + 24).unwrap();
        let result = reconcile_capacity::<u64>(3, 10, &mut budget);
        if limit == FLOOR + 80 {
            assert_eq!(result, Ok(()));
            assert_eq!(budget.storage(), FLOOR + 80);
            let before = budget.storage();
            assert_eq!(
                reconcile_capacity::<u64>(10, 3, &mut budget),
                Err(Resource::Accounting.into())
            );
            assert_eq!(budget.storage(), before);
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(budget.storage(), FLOOR + 24);
        }
    }
}
#[test]
fn capacity_growth_prepays_coexistence_and_oversized_arithmetic_fails_before_allocation() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    scoped(&mut budget, |budget| {
        let mut values = vector::<u64>(1, budget)?;
        values.push(1);
        let old = bytes::<u64>(values.capacity())?;
        push(&mut values, 2, 16, budget)?;
        assert!(budget.peak_storage() >= FLOOR + old + bytes::<u64>(values.capacity())?);
        assert_eq!(
            vector::<u64>(usize::MAX, budget).unwrap_err(),
            Error::Resource(Resource::Arithmetic)
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
