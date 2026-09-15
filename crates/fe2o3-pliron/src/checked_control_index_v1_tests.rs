use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockControlV1, CanonicalKirEdgeControlV1, CanonicalKirEdgePlacementV1,
    CanonicalKirOutputUseV1, CheckedCanonicalKirControlIndexV1 as Control,
};
use fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument;

#[path = "checked_control_index_budget_v1_tests.rs"]
mod exact_budgets;

#[test]
fn actual_duplicate_successors_keep_control_operand_and_argument_occurrences_distinct() {
    with_transition(
        &duplicate_edges(None, false),
        |observed, input, output, budget| {
            let checked_storage = {
                let (checked, checked_storage) =
                    check(input, output, observed.occurrences().candidate(), budget).unwrap();
                budget
                    .reserve_storage(checked_storage.retained_storage())
                    .unwrap();
                let floor = budget.storage();
                let (control, storage) = Control::derive(&checked, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert!(std::ptr::eq(control.input(), input));
                assert!(std::ptr::eq(control.output(), output));
                let expected = size_of::<Control<'_, '_, '_>>()
                    + input.blocks().len() * size_of::<CanonicalKirBlockControlV1>()
                    + input.edges().len() * size_of::<CanonicalKirEdgeControlV1>()
                    + input.uses().len() * size_of::<Option<CanonicalKirOutputUseV1>>()
                    + input.edge_arguments().len() * size_of::<Option<EdgeArgument>>();
                assert_eq!(storage.retained_storage(), expected);
                let entry = input.blocks()[0].coordinate;
                let fact = control.block(entry, budget).unwrap();
                assert!(fact.reachable);
                assert_eq!(fact.selected_successor, None);
                let one = control.edge(input.edges()[0].coordinate, budget).unwrap();
                let two = control.edge(input.edges()[1].coordinate, budget).unwrap();
                assert!(one.executable && two.executable);
                assert_ne!(one.placement, two.placement);
                assert!(matches!(
                    one.placement,
                    CanonicalKirEdgePlacementV1::Retained(_)
                ));
                assert!(matches!(
                    two.placement,
                    CanonicalKirEdgePlacementV1::Retained(_)
                ));
                let actual = control
                    .operand(
                        UseCoordinate::TerminatorOperand {
                            block: entry,
                            operand: 0,
                        },
                        budget,
                    )
                    .unwrap()
                    .unwrap();
                let used = output
                    .uses()
                    .iter()
                    .find(|row| row.coordinate == actual.coordinate)
                    .unwrap();
                assert_eq!(
                    actual.definition,
                    output.definitions()[used.definition].coordinate
                );
                for original in input.edge_arguments() {
                    let mapped = control.edge_argument(original.coordinate, budget).unwrap();
                    if original.coordinate.argument == 0 {
                        let mapped = mapped.unwrap();
                        let edge = control.edge(original.coordinate.edge, budget).unwrap();
                        assert_eq!(
                            edge.placement,
                            CanonicalKirEdgePlacementV1::Retained(mapped.edge)
                        );
                        let row = output
                            .edge_arguments()
                            .iter()
                            .find(|row| row.coordinate == mapped)
                            .unwrap();
                        assert_eq!(row.coordinate.argument, 0);
                    } else {
                        assert_eq!(mapped, None);
                    }
                }
                drop(control);
                budget.release_storage(storage.retained_storage()).unwrap();
                checked_storage
            };
            budget
                .release_storage(checked_storage.retained_storage())
                .unwrap();
        },
    );
}

#[test]
fn actual_constant_selection_distinguishes_nonexecutable_edge_and_merge_connector() {
    for condition in [true, false] {
        with_transition(
            &duplicate_edges(Some(condition), false),
            |observed, input, output, budget| {
                let checked_storage = {
                    let (checked, receipt) =
                        check(input, output, observed.occurrences().candidate(), budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    let (control, storage) = Control::derive(&checked, budget).unwrap();
                    budget.reserve_storage(storage.retained_storage()).unwrap();
                    let selected = usize::from(!condition);
                    let fact = control.block(input.blocks()[0].coordinate, budget).unwrap();
                    assert_eq!(
                        fact.selected_successor,
                        Some(input.edges()[selected].coordinate)
                    );
                    let chosen = control
                        .edge(input.edges()[selected].coordinate, budget)
                        .unwrap();
                    let omitted = control
                        .edge(input.edges()[1 - selected].coordinate, budget)
                        .unwrap();
                    assert!(chosen.executable);
                    assert!(matches!(
                        chosen.placement,
                        CanonicalKirEdgePlacementV1::InternalConnector(_)
                    ));
                    assert!(!omitted.executable);
                    assert_eq!(omitted.placement, CanonicalKirEdgePlacementV1::Omitted);
                    assert!(
                        control
                            .operand(
                                UseCoordinate::TerminatorOperand {
                                    block: input.blocks()[0].coordinate,
                                    operand: 0
                                },
                                budget
                            )
                            .unwrap()
                            .is_none()
                    );
                    drop(control);
                    budget.release_storage(storage.retained_storage()).unwrap();
                    receipt
                };
                budget
                    .release_storage(checked_storage.retained_storage())
                    .unwrap();
            },
        );
    }
}

#[test]
fn control_index_and_queries_reject_budget_exhaustion_without_changing_borrowed_floor() {
    with_transition(
        &duplicate_edges(None, false),
        |observed, input, output, budget| {
            let checked_storage = {
                let (checked, receipt) =
                    check(input, output, observed.occurrences().candidate(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = budget.storage();
                for (work_limit, storage_limit) in [
                    (7, STORAGE),
                    (WORK, floor + size_of::<Control<'_, '_, '_>>() - 1),
                ] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                    let mut bounded = Budget::new(&mut work, storage_limit);
                    bounded.charge_work(7).unwrap();
                    bounded.reserve_storage(floor).unwrap();
                    assert!(Control::derive(&checked, &mut bounded).is_err());
                    assert_eq!(bounded.storage(), floor);
                }
                let (control, storage) = Control::derive(&checked, budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                let mut bounded = Budget::new(&mut work, STORAGE);
                bounded.reserve_storage(budget.storage()).unwrap();
                let query_floor = bounded.storage();
                assert!(
                    control
                        .block(input.blocks()[0].coordinate, &mut bounded)
                        .is_err()
                );
                assert_eq!(bounded.storage(), query_floor);
                drop(control);
                budget.release_storage(storage.retained_storage()).unwrap();
                receipt
            };
            budget
                .release_storage(checked_storage.retained_storage())
                .unwrap();
        },
    );
}
