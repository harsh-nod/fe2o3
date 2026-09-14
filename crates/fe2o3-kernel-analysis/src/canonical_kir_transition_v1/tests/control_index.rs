//! Explicit checked transition records, independent of an optimizer producer.
use super::*;
use crate::{
    CanonicalKirBlockControlV1, CanonicalKirBlockPlacementV1, CanonicalKirEdgePlacementV1,
    CheckedCanonicalKirControlIndexV1 as ControlIndex,
};

fn checked_view(
    a: &Inventory<'_>,
    b: &Inventory<'_>,
    rows: &Rows,
    floor: usize,
    inspect: impl FnOnce(&CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>, &mut Budget<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + LIMIT);
    budget.reserve_storage(floor).unwrap();
    let receipt = {
        let (checked, receipt) =
            check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        inspect(&checked, &mut budget);
        receipt
    };
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn selected_false_edge_and_consumed_merge_connector_have_explicit_placement() {
    let mut entry = BasicBlock::new(BlockId(900));
    entry.operations.push(constant(7, Constant::Bool(false)));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(50),
        then_arguments: vec![],
        else_target: BlockId(70),
        else_arguments: vec![],
    });
    let input = module(
        vec![],
        vec![],
        vec![],
        vec![
            entry,
            returning(50, vec![], &[]),
            returning(70, vec![], &[]),
        ],
    );
    let output = module(vec![], vec![], vec![], vec![returning(900, vec![], &[])]);
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, Some(edge(0, 1))), (2, None)]],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            checked_view(a, b, rows, floor, |checked, budget| {
                let before = budget.storage();
                let (control, receipt) = ControlIndex::derive(checked, budget).unwrap();
                assert_eq!(budget.storage(), before);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert!(std::ptr::eq(control.input(), a));
                assert!(std::ptr::eq(control.output(), b));
                assert!(!control.grants_authority());
                assert_eq!(
                    control.block(block(0), budget).unwrap(),
                    CanonicalKirBlockControlV1 {
                        reachable: true,
                        placement: Some(CanonicalKirBlockPlacementV1 {
                            output: block(0),
                            segment: 0
                        }),
                        selected_successor: Some(edge(0, 1)),
                    }
                );
                let dead = control.block(block(1), budget).unwrap();
                assert!(!dead.reachable);
                assert_eq!(dead.placement, None);
                let tail = control.block(block(2), budget).unwrap();
                assert!(tail.reachable);
                assert_eq!(
                    tail.placement,
                    Some(CanonicalKirBlockPlacementV1 {
                        output: block(0),
                        segment: 1
                    })
                );
                let omitted = control.edge(edge(0, 0), budget).unwrap();
                assert!(!omitted.executable);
                assert_eq!(omitted.placement, CanonicalKirEdgePlacementV1::Omitted);
                let selected = control.edge(edge(0, 1), budget).unwrap();
                assert!(selected.executable);
                assert_eq!(
                    selected.placement,
                    CanonicalKirEdgePlacementV1::InternalConnector(CanonicalKirBlockPlacementV1 {
                        output: block(0),
                        segment: 0
                    })
                );
                assert_eq!(control.operand(term(0, 0), budget).unwrap(), None);
                drop(control);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), before);
            })
        },
    );
}

#[test]
fn repeated_edges_keep_distinct_argument_and_output_use_occurrences() {
    let mut entry = BasicBlock::new(BlockId(u32::MAX));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(77),
        then_arguments: vec![ValueId(2)],
        else_target: BlockId(77),
        else_arguments: vec![ValueId(3)],
    });
    let mut join = returning(77, vec![], &[10]);
    join.parameters.push(ValueDef::new(ValueId(10), U32));
    let input = module(
        vec![Type::BOOL, U32, U32],
        vec![U32],
        vec![1, 2, 3],
        vec![entry, join],
    );
    inspect(
        input.clone(),
        input,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            checked_view(a, b, rows, floor, |checked, budget| {
                let before = budget.storage();
                let (control, receipt) = ControlIndex::derive(checked, budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(
                    control.block(block(0), budget).unwrap().selected_successor,
                    None
                );
                for successor in 0..2 {
                    let old_edge = edge(0, successor);
                    let placement = control.edge(old_edge, budget).unwrap();
                    assert!(placement.executable);
                    assert_eq!(
                        placement.placement,
                        CanonicalKirEdgePlacementV1::Retained(old_edge)
                    );
                    assert_eq!(
                        control
                            .edge_argument(edge_arg(0, successor, 0), budget)
                            .unwrap(),
                        Some(edge_arg(0, successor, 0))
                    );
                    let arguments = control.input_edge_arguments(old_edge, budget).unwrap();
                    assert_eq!(arguments.len(), 1);
                    assert_eq!(arguments[0].value, ValueId(2 + successor));
                    let use_row = control
                        .operand(term(0, 1 + successor), budget)
                        .unwrap()
                        .unwrap();
                    assert_eq!(use_row.coordinate, term(0, 1 + successor));
                    assert_eq!(
                        use_row.definition,
                        Definition::FunctionArgument {
                            function: F,
                            argument: 1 + successor
                        }
                    );
                }
                assert!(matches!(
                    control.edge(edge(0, 2), budget),
                    Err(Error::InvalidCoordinate)
                ));
                drop(control);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), before);
            });
        },
    );
}

#[test]
fn empty_and_return_only_control_indexes_have_independent_exact_boundaries() {
    for nonempty in [false, true] {
        let input = if nonempty {
            module(
                vec![],
                vec![],
                vec![],
                vec![returning(u32::MAX, vec![], &[])],
            )
        } else {
            Module::new("x")
        };
        inspect(
            input.clone(),
            input,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                checked_view(a, b, rows, floor, |checked, setup| {
                    // State: two inventory references, nine slice headers, eighteen
                    // Vec headers = 74 words. Return-only payload: eight words and
                    // one reachability byte. No observed successful budget is used.
                    let word = size_of::<usize>();
                    let header = size_of::<ControlIndex<'_, '_, '_>>();
                    let block_payload =
                        usize::from(nonempty) * size_of::<CanonicalKirBlockControlV1>();
                    let retained = header + block_payload;
                    let peak = retained + 74 * word + usize::from(nonempty) * (8 * word + 1);
                    let exact_work = if nonempty { 42 } else { 7 };
                    let borrowed = setup.storage() + 19;
                    for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
                        let mut work =
                            CanonicalKernelIrWorkBudgetV1::new(7 + exact_work - work_under);
                        {
                            let mut budget =
                                Budget::new(&mut work, borrowed + peak - storage_under);
                            budget.charge_work(7).unwrap();
                            budget.reserve_storage(borrowed).unwrap();
                            let outcome = ControlIndex::derive(checked, &mut budget);
                            assert_eq!(budget.storage(), borrowed);
                            match (work_under, storage_under) {
                                (0, 0) => {
                                    let (control, receipt) = outcome.unwrap();
                                    assert_eq!(receipt.retained_storage(), retained);
                                    assert_eq!(budget.work(), 7 + exact_work);
                                    assert_eq!(budget.peak_storage(), borrowed + peak);
                                    budget.reserve_storage(retained).unwrap();
                                    drop(control);
                                    budget.release_storage(retained).unwrap();
                                }
                                (1, 0) => {
                                    assert!(matches!(
                                        outcome,
                                        Err(Error::Resource(Resource::Work(_)))
                                    ));
                                    assert_eq!(budget.work(), 7 + exact_work - 1);
                                    assert_eq!(budget.peak_storage(), borrowed + peak);
                                }
                                (0, 1) => {
                                    assert!(matches!(
                                        outcome,
                                        Err(Error::Resource(Resource::Storage(_)))
                                    ));
                                    assert_eq!(budget.failed_storage(), Some(borrowed + peak));
                                    assert_eq!(budget.work(), 7 + if nonempty { 11 } else { 2 });
                                }
                                _ => unreachable!(),
                            }
                        }
                        if work_under != 0 {
                            assert_eq!(work.failed_work(), Some(7 + exact_work));
                        }
                    }
                });
            },
        );
    }
}
