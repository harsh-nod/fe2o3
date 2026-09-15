use super::*;
use super::super::{WorkingCells, MAX_CELLS};

fn replace_entry(
    original: SemanticFunctionDeclV1,
    terminator: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    let mut blocks = original.blocks().to_vec();
    let first = &blocks[0];
    blocks[0] = SemanticBasicBlockV1::new(first.identity(), first.source(),
        first.statements().to_vec(), SemanticTerminatorV1::new(first.source(), terminator)).unwrap();
    SemanticFunctionDeclV1::new(original.identity(), original.role(),
        original.item_definition_identity(), original.monomorphization_identity(),
        original.generic_type_arguments_identity(), original.const_generic_arguments_identity(),
        original.source(), original.abi().clone(), original.locals().to_vec(), original.entry(), blocks,
    ).unwrap()
}

fn cases() -> Vec<(&'static str, SemanticFunctionDeclV1)> {
    let mut cases: Vec<_> = [
        ("ordinary", Mutation::None), ("swapped", Mutation::Swap),
        ("signed", Mutation::Signed), ("width", Mutation::Width),
        ("overwritten", Mutation::Overwrite), ("escaped", Mutation::Escape),
        ("storage_dead", Mutation::Storage), ("wrong_variant", Mutation::WrongVariant),
        ("bypass", Mutation::Bypass), ("backedge", Mutation::Backedge),
        ("call", Mutation::UnknownCall), ("parallel_call_cleanup", Mutation::Unwind),
        ("different_return", Mutation::DifferentReturn),
    ].into_iter().map(|(name, mutation)| (name, fixture(mutation))).collect();
    for (name, terminator) in [
        ("parallel_switch", SemanticTerminatorKindV1::SwitchInt {
            discriminant: constant(U32, 0),
            targets: SemanticSwitchTargetsV1::new(vec![
                SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 1)),
                SemanticSwitchTargetV1::new(1, edge(SemanticEdgeRoleV1::SwitchValue, 1)),
            ], edge(SemanticEdgeRoleV1::SwitchOtherwise, 1)).unwrap(),
        }),
        ("false_edge", SemanticTerminatorKindV1::FalseEdge {
            real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
            imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 5),
        }),
        ("assert_cleanup", SemanticTerminatorKindV1::Assert {
            condition: constant(BOOL, 1), expected: true,
            message: SemanticAssertMessageV1::NullPointerDereference,
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
            unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::AssertUnwind, 5)),
        }),
        ("entry_cycle", goto(0)),
        ("invalid_target", goto(500)),
    ] { cases.push((name, replace_entry(fixture(Mutation::None), terminator))); }
    cases
}

#[derive(Debug, Eq, PartialEq)]
struct Observation {
    work: usize,
    live: usize,
    peak: usize,
    range: Option<(u128, u128)>,
    error: Option<&'static str>,
}

fn observe(function: &SemanticFunctionDeclV1, limit: usize, inherited: usize, mut work: usize) -> Observation {
    let types = types();
    let mut cells = WorkingCells::new(limit);
    cells.reserve(inherited).unwrap();
    let result = HelperResultRangesV1::analyze_with_cells(&types, function, &mut work, &mut cells);
    let (range, error, live) = match result {
        Ok(facts) => {
            let range = facts.at(&types, function, use_operand(function), 8, 1)
                .map(|range| (range.minimum, range.maximum));
            let retained = facts.operands.retained_cells().unwrap();
            assert_eq!(cells.live, inherited + retained);
            let live = cells.live;
            drop(facts);
            cells.release(retained);
            (range, None, live)
        }
        Err(ProductionRankedProjectionErrorV1::Unsupported(error)
            | ProductionRankedProjectionErrorV1::Incomplete(error)) => (None, Some(error), cells.live),
        Err(error) => panic!("unexpected helper error: {error:?}"),
    };
    assert_eq!(cells.live, inherited, "helper scratch or returned facts leaked a cell reservation");
    Observation { work, live, peak: cells.peak, range, error }
}

// Produced by the unchanged pre-split engine, not the candidate engine.
const GOLDEN: &[(&str, usize, usize, usize, Option<(u128, u128)>, Option<&str>)] = &[
    ("ordinary", 1355, 131, 1909, Some((1, 4)), None),
    ("swapped", 1355, 131, 1909, Some((4, 4294967295)), None),
    ("signed", 990, 67, 1810, Some((0, 18446744073709551615)), None),
    ("width", 1355, 131, 1909, Some((1, 4294967295)), None),
    ("overwritten", 1368, 131, 1909, Some((4294967295, 4294967295)), None),
    ("escaped", 996, 67, 1810, Some((0, 18446744073709551615)), None),
    ("storage_dead", 1268, 131, 1903, Some((0, 18446744073709551615)), None),
    ("wrong_variant", 1339, 131, 1907, Some((0, 18446744073709551615)), None),
    ("bypass", 1290, 131, 1905, Some((1, 4294967295)), None),
    ("backedge", 331, 19, 1738, None, None),
    ("call", 1347, 131, 1909, Some((0, 18446744073709551615)), None),
    ("parallel_call_cleanup", 857, 67, 1814, Some((0, 18446744073709551615)), None),
    ("different_return", 1359, 131, 1909, Some((18446744073709551615, 18446744073709551615)), None),
    ("parallel_switch", 1414, 131, 1913, Some((0, 4)), None),
    ("false_edge", 778, 67, 1803, Some((0, 18446744073709551615)), None),
    ("assert_cleanup", 781, 67, 1803, Some((0, 18446744073709551615)), None),
    ("entry_cycle", 60, 3, 1690, None, None),
    ("invalid_target", 34, 0, 1643, None, Some("helper-result range edge outside retained function")),
];

#[test]
fn helper_borrowed_edges_preserve_frozen_cell_work_and_range_observations() {
    let cases = cases();
    assert_eq!(cases.len(), GOLDEN.len());
    for ((name, function), &(label, work, live, peak, range, error)) in cases.iter().zip(GOLDEN) {
        assert_eq!(*name, label);
        let original = function.clone();
        assert_eq!(observe(function, MAX_CELLS, 0, 0), Observation { work, live, peak, range, error }, "{name}");
        assert_eq!(function, &original);
    }
}

#[test]
fn helper_borrowed_edges_keep_exact_storage_boundaries_and_inherited_cells() {
    for ((name, function), &(_, _, _, peak, _, error)) in cases().iter().zip(GOLDEN) {
        if error.is_some() { continue; }
        for inherited in [0, 17] {
            let exact = observe(function, inherited + peak, inherited, 0);
            assert_eq!(exact.error, None, "{name}");
            assert_eq!(exact.peak, inherited + peak);
            let short = observe(function, inherited + peak - 1, inherited, 0);
            assert_eq!(short.error, Some("helper-result range working storage limit"), "{name}");
            assert_eq!(short.live, inherited);
        }
    }
}

#[test]
fn helper_borrowed_edges_keep_exact_shared_work_failure_without_refund() {
    for ((name, function), &(_, work, _, _, _, error)) in cases().iter().zip(GOLDEN) {
        if error.is_some() { continue; }
        let exact = observe(function, MAX_CELLS, 17, MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - work);
        assert_eq!(exact.error, None, "{name}");
        assert_eq!(exact.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        let short = observe(function, MAX_CELLS, 17, MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - work + 1);
        assert_eq!(short.error, Some("uniform induction CFG analysis exceeds its work limit"), "{name}");
        assert!(short.work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        assert_eq!(short.live, 17);
    }
}
