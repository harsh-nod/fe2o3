use super::*;
include!("literal_equality_v1_tests.rs");
use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
use dialect_kernel::{DIALECT_NAME, register_dialect};
use pliron::{
    basic_block::BasicBlock, builtin::op_interfaces::OneRegionInterface, dialect::DialectName,
    linked_list::ContainsLinkedList, parsable::parse_from_str,
};

const DIRECT: &str = include_str!("../tests/bounds-transport-lit/direct.pliron");
const DIAMOND: &str = include_str!("../tests/bounds-transport-lit/diamond.pliron");
const PARALLEL: &str = include_str!("../tests/bounds-transport-lit/parallel.pliron");
const CYCLE: &str = include_str!("../tests/bounds-transport-lit/cycle.pliron");
const CHANGING: &str = include_str!("../tests/bounds-transport-lit/changing.pliron");
const SPLIT: &str = include_str!("../tests/bounds-transport-lit/split.pliron");
const VIEW_FORWARDING: &str = include_str!("../tests/bounds-transport-lit/view-forwarding.pliron");
const VIEW_ASYMMETRIC: &str = include_str!("../tests/bounds-transport-lit/view-asymmetric.pliron");

fn parsed(source: &str) -> (Context, FuncOp) {
    let mut context = Context::new();
    fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let text = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, &text).unwrap();
    verify_operation(operation, &context).unwrap();
    assert!(Operation::is_op::<FuncOp>(operation, &context));
    (context, FuncOp::from_operation(operation))
}

fn with_graph<T>(source: &str, query: impl FnOnce(&BoundsEdgeTransportV1<'_>) -> T) -> T {
    let (context, function) = parsed(source);
    let blocks = function
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .collect::<Vec<_>>();
    let mut predecessors = vec![Vec::new(); blocks.len()];
    for (block, pointer) in blocks.iter().enumerate() {
        let terminator = pointer.deref(&context).get_terminator(&context).unwrap();
        for (successor, target) in terminator.deref(&context).successors().enumerate() {
            let target = blocks
                .iter()
                .position(|pointer| *pointer == target)
                .unwrap();
            predecessors[target].push(PredecessorEdge {
                block,
                successor,
                guard_fact: None,
            });
        }
    }
    query(&BoundsEdgeTransportV1 {
        context: &context,
        blocks: &blocks,
        predecessors: &predecessors,
    })
}

fn arguments_fact(graph: &BoundsEdgeTransportV1<'_>, block: usize) -> LessThanFact {
    let block = graph.blocks[block].deref(graph.context);
    LessThanFact {
        lhs: IndexExpr::Value(block.get_argument(0)),
        rhs: IndexExpr::Value(block.get_argument(1)),
    }
}

#[test]
fn native_switch_transport_requires_each_case_and_default_to_preserve_the_guard() {
    // Exercises the demand solver on native-verified IR, not production identity
    // admission (whose switch resource/attribute integration is separate).
    const SOURCE: &str = r#"
builtin.func @native_switch_bounds: builtin.function <(kernel.index, kernel.index, builtin.integer ui128) -> ()>
{
  ^entry(i: kernel.index, n: kernel.index, s: builtin.integer ui128):
    kernel.index_lt_br_args (i, n, i, n) [^dispatch, ^exit] []: <(kernel.index, kernel.index, kernel.index, kernel.index) -> ()>
  ^dispatch(q: kernel.index, r: kernel.index):
    gpu.switch_v3 (s, q, r, q, r, q, r) [^access, ^access, ^access] [gpu_switch_kind: gpu.switch_key_kind_v3 LegacyU64, gpu_switch_cases: gpu.switch_case_bits_v3 [18446744073709551615, 0], gpu_switch_offsets: gpu.switch_successor_offsets_v3 [0, 2, 4, 6], operand_segment_sizes: builtin.operand_segment_sizes [1, 6]]: <(builtin.integer ui128, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>
  ^access(x: kernel.index, m: kernel.index):
    kernel.return () [] []: <() -> ()>
  ^exit():
    kernel.return () [] []: <() -> ()>
}
"#;
    for (tuple, proven) in [
        ("(s, q, r, q, r, q, r)", true),
        ("(s, r, q, q, r, q, r)", false),
        ("(s, q, r, r, q, q, r)", false),
        ("(s, q, r, q, r, r, q)", false),
    ] {
        let source = SOURCE.replace("(s, q, r, q, r, q, r)", tuple);
        with_graph(&source, |graph| {
            assert_eq!(
                graph
                    .proves(
                        2,
                        arguments_fact(graph, 2),
                        &mut RankedBoundsBudget::default()
                    )
                    .unwrap(),
                proven
            );
        });
    }
}

fn assert_bounds(source: &str, clean: bool) {
    let (context, function) = parsed(source);
    let report = run_pliron_ranked_bounds_check_v1(&context, &function);
    assert_eq!(report.is_clean(), clean, "{source}\n{report:?}");
    if !clean {
        assert!(
            report
                .findings()
                .iter()
                .any(|finding| matches!(finding, RankedBoundsFindingV1::UnprovedBound { .. })),
            "negative must reach the actual bound: {report:?}"
        );
    }
}

#[test]
fn textual_bounds_transport_fixtures_use_verified_real_ir() {
    for source in [
        DIRECT,
        DIAMOND,
        PARALLEL,
        CYCLE,
        CHANGING,
        SPLIT,
        VIEW_FORWARDING,
        VIEW_ASYMMETRIC,
    ] {
        assert_eq!(
            source
                .lines()
                .filter(|line| *line == "// RUN: fe2o3-pliron-lit --passes=memory-bounds %s")
                .count(),
            1
        );
        let clean = source.lines().any(|line| line == "// EXPECT: CLEAN");
        let unproved = source.lines().any(|line| line == "// EXPECT: UNPROVED");
        assert_ne!(clean, unproved);
        assert_bounds(source, clean);
    }
}

#[test]
fn different_join_values_need_each_edges_own_guard() {
    assert_bounds(DIAMOND, true);
    let bypass = DIAMOND.replace(
        "kernel.index_lt_br_args (j, n, j, n) [^access, ^exit] []: <(kernel.index, kernel.index, kernel.index, kernel.index) -> ()>",
        "kernel.br_args (j, n) [^access] []: <(kernel.index, kernel.index) -> ()>",
    );
    assert_ne!(bypass, DIAMOND);
    assert_bounds(&bypass, false);
    let swapped = DIRECT.replace("(i, n, i, n)", "(i, n, n, i)");
    assert_ne!(swapped, DIRECT);
    assert_bounds(&swapped, false);
}

#[test]
fn forwarded_ranked_view_dimensions_follow_exact_ssa_values() {
    with_graph(VIEW_FORWARDING, |graph| {
        let access = graph.blocks[2].deref(graph.context);
        let index = access.get_argument(0);
        let view = access.get_argument(1);
        let view_type = ranked_view_type(view, graph.context).unwrap();
        let extent = extent_expr(view, &view_type.deref(graph.context), 1, graph.context);
        assert_eq!(extent, IndexExpr::Dimension { view, dimension: 1 });
        assert!(
            graph
                .proves(
                    2,
                    LessThanFact {
                        lhs: IndexExpr::Value(index),
                        rhs: extent,
                    },
                    &mut RankedBoundsBudget::default()
                )
                .unwrap()
        );
    });
    assert_bounds(VIEW_FORWARDING, true);
}

#[test]
fn a_different_forwarded_view_cannot_borrow_the_guarded_extent() {
    let changed = VIEW_FORWARDING.replace(
        "kernel.index_lt_br_args (i, limit, i, source_view)",
        "kernel.index_lt_br_args (i, limit, i, other_view)",
    );
    assert_ne!(changed, VIEW_FORWARDING);
    assert_bounds(&changed, false);
    let (context, function) = parsed(&changed);
    assert!(matches!(
        run_pliron_ranked_bounds_check_v1(&context, &function).findings(),
        [RankedBoundsFindingV1::UnprovedBound {
            block: 2,
            operation: 0,
            dimension: 1,
            ..
        },]
    ));
}

#[test]
fn ranked_view_transport_respects_asymmetric_successor_segments() {
    let original = "kernel.index_eq_br_args (selector, x, carried, x, x, selector, carried) [^left, ^right] []";
    for terminator in [
        original,
        "kernel.index_lt_br_args (selector, x, carried, x, x, selector, carried) [^left, ^right] []",
        "kernel.analysis_split (selector, x, carried, x, x, selector, carried) [^left, ^right] [kernel_analysis_split_control_count: kernel.analysis_split_control_count 2]",
    ] {
        let source = VIEW_ASYMMETRIC.replace(original, terminator);
        assert!(source.contains(terminator));
        assert_bounds(&source, true);
        // Only the false edge changes: its first index is now an independent
        // selector, while the two-argument true edge keeps its exact payload.
        let hostile = source.replace(
            "(selector, x, carried, x, x, selector, carried)",
            "(selector, x, carried, x, selector, x, carried)",
        );
        assert_ne!(hostile, source);
        assert_bounds(&hostile, false);
    }
}

#[test]
fn parallel_true_and_false_edges_do_not_collapse() {
    with_graph(PARALLEL, |graph| {
        assert_eq!(graph.predecessors[1].len(), 2);
        assert_eq!(graph.predecessors[1][0].successor, 0);
        assert_eq!(graph.predecessors[1][1].successor, 1);
        assert!(
            !graph
                .proves(
                    1,
                    arguments_fact(graph, 1),
                    &mut RankedBoundsBudget::default()
                )
                .unwrap()
        );
    });
}

#[test]
fn changing_loop_result_is_not_a_previous_iteration_fact() {
    assert_bounds(CYCLE, true);
    assert_bounds(CHANGING, false);
    let (context, function) = parsed(CHANGING);
    let report = run_pliron_ranked_bounds_check_v1(&context, &function);
    assert!(matches!(
        report.findings(),
        [RankedBoundsFindingV1::UnprovedBound {
            block: 1,
            operation: 1,
            ..
        }]
    ));
}

#[test]
fn entry_has_an_implicit_unguarded_predecessor() {
    with_graph(CYCLE, |graph| {
        assert!(
            !graph
                .proves(
                    0,
                    arguments_fact(graph, 0),
                    &mut RankedBoundsBudget::default()
                )
                .unwrap()
        );
    });
    // Supplying an explicit loop edge to entry cannot replace that root edge.
    with_graph(PARALLEL, |graph| {
        let mut predecessors = graph.predecessors.to_vec();
        predecessors[0].push(PredecessorEdge {
            block: 0,
            successor: 0,
            guard_fact: Some(0),
        });
        let changed = BoundsEdgeTransportV1 {
            predecessors: &predecessors,
            ..*graph
        };
        assert!(
            !changed
                .proves(
                    0,
                    arguments_fact(graph, 0),
                    &mut RankedBoundsBudget::default()
                )
                .unwrap()
        );
    });
}

fn chain(depth: usize) -> String {
    use std::fmt::Write as _;
    assert!(depth != 0);
    let mut text = String::from(
        "builtin.func @chain: builtin.function <(kernel.index, kernel.index) -> ()> {\n^entry(i: kernel.index, n: kernel.index):\nkernel.index_lt_br_args (i,n,i,n) [^b0,^exit] []: <(kernel.index,kernel.index,kernel.index,kernel.index)->()>\n",
    );
    for block in 0..depth {
        writeln!(
            text,
            "^b{block}(x{block}: kernel.index, n{block}: kernel.index):"
        )
        .unwrap();
        if block + 1 == depth {
            writeln!(text, "view = kernel.ranked_view (n{block}) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index)->(kernel.ranked_view <32,false,[0]>)>;").unwrap();
            writeln!(text, "kernel.access (view,x{block}) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[0]>,kernel.index)->()>;").unwrap();
            writeln!(text, "kernel.return () [] []: <()->()>").unwrap();
        } else {
            writeln!(
                text,
                "kernel.br_args (x{block},n{block}) [^b{}] []: <(kernel.index,kernel.index)->()>",
                block + 1
            )
            .unwrap();
        }
    }
    text.push_str("^exit():\nkernel.return () [] []: <()->()>\n}");
    text
}

#[test]
fn deep_forwarding_and_shared_paths_terminate_without_global_unions() {
    for depth in [1, 2, 9, 64, 128] {
        assert_bounds(&chain(depth), true);
    }
    assert_bounds(SPLIT, true);
}

#[test]
fn literal_query_work_and_storage_boundaries_preserve_denied_prefixes() {
    assert!(
        std::mem::size_of::<BoundsTransportObligationV1>() <= 16 * std::mem::size_of::<usize>()
    );
    assert_eq!(MAX_RANKED_MEMORY_RANK, 8);
    // Initial owner/enqueue 24; pop+definition checks 22; edge 1;
    // pullback 16; two (tag4 + position6 + types8 + canonical40); guard80; compare16.
    // Total 275. Vec owner3 + first requested capacity4 * row16 = 67.
    with_graph(DIRECT, |graph| {
        let fact = arguments_fact(graph, 1);
        let mut exact = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - 275,
            storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - 67,
            ..RankedBoundsBudget::default()
        };
        assert!(graph.proves(1, fact, &mut exact).unwrap());
        assert_eq!(exact.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
        assert_eq!(exact.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
        let mut work_under = RankedBoundsBudget {
            work_units: MAX_RANKED_BOUNDS_WORK_UNITS - 274,
            ..RankedBoundsBudget::default()
        };
        assert_eq!(
            graph.proves(1, fact, &mut work_under),
            Err(bounds_transport_failure_v1(
                "analysis work unit",
                MAX_RANKED_BOUNDS_WORK_UNITS,
                MAX_RANKED_BOUNDS_WORK_UNITS + 1,
            ))
        );
        assert_eq!(work_under.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 15);
        assert_eq!(work_under.storage_items, 67);
        let mut storage_under = RankedBoundsBudget {
            storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - 66,
            ..RankedBoundsBudget::default()
        };
        assert_eq!(
            graph.proves(1, fact, &mut storage_under),
            Err(bounds_transport_failure_v1(
                "analysis storage item",
                MAX_RANKED_BOUNDS_STORAGE_ITEMS,
                MAX_RANKED_BOUNDS_STORAGE_ITEMS + 1,
            ))
        );
        assert_eq!(storage_under.work_units, 2);
        assert_eq!(
            storage_under.storage_items,
            MAX_RANKED_BOUNDS_STORAGE_ITEMS - 63
        );
    });
}

#[test]
fn queue_duplicates_and_obligation_ceiling_are_prepaid() {
    let make = |block| BoundsTransportObligationV1 {
        block,
        fact: LessThanFact {
            lhs: IndexExpr::Constant(0),
            rhs: IndexExpr::Constant(1),
        },
    };
    let mut visited = Vec::with_capacity(4);
    visited.push(make(0));
    let mut budget = RankedBoundsBudget::default();
    bounds_transport_enqueue_v1(&mut visited, make(0), &mut budget).unwrap();
    assert_eq!(
        (budget.work_units, budget.storage_items, visited.len()),
        (18, 0, 1)
    );
    bounds_transport_enqueue_v1(&mut visited, make(1), &mut budget).unwrap();
    assert_eq!(
        (budget.work_units, budget.storage_items, visited.len()),
        (53, 0, 2)
    );
    let mut full = (0..MAX_RANKED_BOUNDS_FACTS).map(make).collect::<Vec<_>>();
    let mut budget = RankedBoundsBudget::default();
    assert_eq!(
        bounds_transport_enqueue_v1(&mut full, make(MAX_RANKED_BOUNDS_FACTS), &mut budget),
        Err(bounds_transport_failure_v1(
            "transport obligation",
            1_024,
            1_025,
        ))
    );
    assert_eq!(
        (budget.work_units, budget.storage_items, full.len()),
        (17_409, 0, 1_024)
    );
    let mut work_under = RankedBoundsBudget {
        work_units: MAX_RANKED_BOUNDS_WORK_UNITS - 17_408,
        ..RankedBoundsBudget::default()
    };
    assert_eq!(
        bounds_transport_enqueue_v1(&mut full, make(MAX_RANKED_BOUNDS_FACTS), &mut work_under),
        Err(bounds_transport_failure_v1(
            "analysis work unit",
            MAX_RANKED_BOUNDS_WORK_UNITS,
            MAX_RANKED_BOUNDS_WORK_UNITS + 1
        ))
    );
    assert_eq!(work_under.work_units, MAX_RANKED_BOUNDS_WORK_UNITS - 17_408);
    assert_eq!(full.len(), 1_024);
}

#[test]
fn every_query_work_prefix_matches_an_independent_charge_sequence() {
    const CHARGES: [usize; 20] = [
        1, 1, 5, 17, 18, 2, 2, 1, 16, 4, 6, 8, 40, 4, 6, 8, 40, 40, 40, 16,
    ];
    assert_eq!(CHARGES.iter().sum::<usize>(), 275);
    with_graph(DIRECT, |graph| {
        for limit in 0..=275 {
            let initial = MAX_RANKED_BOUNDS_WORK_UNITS - limit;
            let mut budget = RankedBoundsBudget {
                work_units: initial,
                ..RankedBoundsBudget::default()
            };
            let result = graph.proves(1, arguments_fact(graph, 1), &mut budget);
            let mut accepted = 0;
            let mut attempted = None;
            for charge in CHARGES {
                if accepted + charge > limit {
                    attempted = Some(accepted + charge);
                    break;
                }
                accepted += charge;
            }
            if let Some(attempted) = attempted {
                assert_eq!(
                    result,
                    Err(bounds_transport_failure_v1(
                        "analysis work unit",
                        MAX_RANKED_BOUNDS_WORK_UNITS,
                        initial + attempted
                    )),
                    "limit {limit}"
                );
            } else {
                assert_eq!(result, Ok(true));
            }
            assert_eq!(budget.work_units, initial + accepted, "limit {limit}");
            assert_eq!(
                budget.storage_items,
                if limit == 0 {
                    0
                } else if limit == 1 {
                    3
                } else {
                    67
                }
            );
        }
    });
}

#[test]
fn checked_arithmetic_and_small_input_envelope_are_literal() {
    assert!(bounds_transport_product_v1(usize::MAX, 2).is_err());
    assert!(bounds_transport_sum_v1(usize::MAX, 1).is_err());
    let census = ProductionAnalysisInputCensusV1 {
        blocks: 1,
        block_arguments: 1,
        ranked_accesses: 1,
        operands: 1,
        ..ProductionAnalysisInputCensusV1::default()
    };
    // V1 gives eighteen canonical atoms, S324, Q1, E0: 19+59*324=19135;
    // retained admission bound: 3+64*324=20739. No max-fuel reservation here.
    assert_eq!(
        bounds_transport_resource_bound_v1(census),
        Ok((19_135, 20_739))
    );
    assert!(
        bounds_transport_resource_bound_v1(ProductionAnalysisInputCensusV1 {
            block_arguments: usize::MAX,
            ..census
        })
        .is_err()
    );
}

#[test]
fn actual_identity_census_admits_exact_bound_and_rejects_one_under() {
    let (context, function) = parsed(DIRECT);
    let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
    let capture = provider
        .capture_with_resource_limits_v1(ProductionAnalysisResourceLimitsV1::new(
            usize::MAX,
            usize::MAX,
        ))
        .ok()
        .unwrap();
    let census = capture.input_census;
    assert_eq!(
        (
            census.blocks,
            census.operations,
            census.results,
            census.block_arguments,
            census.ranked_accesses
        ),
        (3, 5, 1, 4, 1)
    );
    let bound = preflight_ranked_bounds_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
    )
    .unwrap();
    let exact = ProductionAnalysisResourceLimitsV1::new(
        bound.work_upper_bound(),
        bound.peak_storage_upper_bound(),
    );
    assert_eq!(
        preflight_ranked_bounds_resource_upper_bound_v1(census, exact),
        Ok(bound)
    );
    assert!(
        preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(
                bound.work_upper_bound() - 1,
                bound.peak_storage_upper_bound()
            )
        )
        .is_err()
    );
    assert!(
        preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(
                bound.work_upper_bound(),
                bound.peak_storage_upper_bound() - 1
            )
        )
        .is_err()
    );
    assert!(run_pliron_ranked_bounds_check_v1(&context, &function).is_clean());
}

#[test]
fn entry_only_arguments_keep_the_legacy_solver_but_have_a_conservative_envelope() {
    let source = "builtin.func @entry_only: builtin.function <(kernel.index,kernel.index)->()> {\n^entry(i: kernel.index,n: kernel.index):\nview = kernel.ranked_view () [] [kernel_memory_space: kernel.memory_space Global]: <()->(kernel.ranked_view <32,false,[16]>)>;\nzero = kernel.index_constant () [] [kernel_index_value: kernel.index_value 0]: <()->(kernel.index)>;\nkernel.access (view,zero) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[16]>,kernel.index)->()>;\nkernel.return () [] []: <()->()>\n}";
    let (context, function) = parsed(source);
    let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
    let capture = provider
        .capture_with_resource_limits_v1(
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .ok()
        .unwrap();
    assert_eq!(
        (
            capture.input_census.blocks,
            capture.input_census.block_arguments
        ),
        (1, 2)
    );
    assert!(
        bounds_transport_resource_bound_v1(capture.input_census)
            .unwrap()
            .0
            > 0
    );
    preflight_ranked_bounds_resource_upper_bound_v1(
        capture.input_census,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    assert!(run_pliron_ranked_bounds_check_v1(&context, &function).is_clean());
}

#[test]
fn unreachable_predecessors_remain_an_explicit_finding() {
    let source = DIRECT.replace("  ^exit():", "  ^dead():\n    di = kernel.index_unknown () [] []: <()->(kernel.index)>;\n    dn = kernel.index_unknown () [] []: <()->(kernel.index)>;\n    kernel.br_args (di,dn) [^access] []: <(kernel.index,kernel.index)->()>\n  ^exit():");
    let (context, function) = parsed(&source);
    let report = run_pliron_ranked_bounds_check_v1(&context, &function);
    assert!(!report.is_clean());
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| matches!(finding, RankedBoundsFindingV1::UnreachableBlock { .. }))
    );
}

#[test]
fn malformed_edge_and_stale_argument_do_not_publish_a_proof() {
    with_graph(DIRECT, |graph| {
        let mut predecessors = graph.predecessors.to_vec();
        predecessors[1][0].successor = 1;
        let changed = BoundsEdgeTransportV1 {
            predecessors: &predecessors,
            ..*graph
        };
        assert_eq!(
            changed.proves(
                1,
                arguments_fact(graph, 1),
                &mut RankedBoundsBudget::default()
            ),
            Err(RankedBoundsFindingV1::StructuralVerificationFailed)
        );
    });
    let unused = DIRECT.replace(
        "    view = kernel.ranked_view (m) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,false,[0]>)>;\n    kernel.access (view, x) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[0]>, kernel.index) -> ()>;\n", "",
    );
    assert_ne!(unused, DIRECT);
    with_graph(&unused, |graph| {
        let fact = arguments_fact(graph, 1);
        BasicBlock::remove_argument(graph.blocks[1], graph.context, 1);
        assert_eq!(
            graph.proves(1, fact, &mut RankedBoundsBudget::default()),
            Err(RankedBoundsFindingV1::StructuralVerificationFailed)
        );
    });
}

#[test]
fn a_failed_query_does_not_seed_the_next_query() {
    with_graph(PARALLEL, |graph| {
        let fact = arguments_fact(graph, 1);
        let mut budget = RankedBoundsBudget::default();
        for _ in 0..3 {
            assert!(!graph.proves(1, fact, &mut budget).unwrap());
        }
    });
    let access = "    kernel.access (view, x) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,false,[0]>, kernel.index) -> ()>;";
    let twice = PARALLEL.replace(access, &format!("{access}\n{access}"));
    assert_ne!(twice, PARALLEL);
    let (context, function) = parsed(&twice);
    let report = run_pliron_ranked_bounds_check_v1(&context, &function);
    assert!(matches!(
        report.findings(),
        [
            RankedBoundsFindingV1::UnprovedBound {
                block: 1,
                operation: 1,
                ..
            },
            RankedBoundsFindingV1::UnprovedBound {
                block: 1,
                operation: 2,
                ..
            },
        ]
    ));
}

#[test]
fn foreign_owner_and_mistyped_edges_never_enter_production_identity() {
    use crate::production_analysis::pliron_pass_contract::{
        PlironPassPreservationErrorV1, begin_production_pliron_pass_contract_session_v1,
    };
    use dialect_kernel::IndexType;
    use pliron::builtin::types::{FunctionType, UnitType};
    use pliron::r#type::TypeHandle;
    for foreign_type in [false, true] {
        let (mut context, function) = parsed(DIRECT);
        assert!(run_pliron_ranked_bounds_check_v1(&context, &function).is_clean());
        let argument_type: TypeHandle = if foreign_type {
            UnitType::get(&context).into()
        } else {
            IndexType::get(&context).into()
        };
        let ty = FunctionType::get(&context, vec![argument_type], vec![]);
        let foreign = FuncOp::new(&mut context, "foreign_owner".try_into().unwrap(), ty);
        let ret = ReturnOp::new(&mut context);
        ret.get_operation()
            .insert_at_back(foreign.get_entry_block(&context), &context);
        verify_operation(foreign.get_operation(), &context).unwrap();
        let value = foreign
            .get_entry_block(&context)
            .deref(&context)
            .get_argument(0);
        let terminator = function
            .get_entry_block(&context)
            .deref(&context)
            .get_terminator(&context)
            .unwrap();
        Operation::replace_operand(terminator, &context, 2, value);
        if foreign_type {
            assert!(verify_operation(function.get_operation(), &context).is_err());
            assert!(matches!(
                run_pliron_ranked_bounds_check_v1(&context, &function).findings(),
                [RankedBoundsFindingV1::StructuralVerificationFailed]
            ));
        } else {
            // Pliron visits local definitions for dominance; the closed
            // production identity additionally rejects external operands.
            verify_operation(function.get_operation(), &context).unwrap();
            let error =
                crate::derive_pliron_ir_structural_identity_v1(&context, &function).unwrap_err();
            assert!(matches!(
                error,
                crate::PlironIrIdentityErrorV1::ExternalOperand {
                    location: crate::PlironPreserveLocationV1::Operation {
                        block: 0,
                        operation: 0,
                        ..
                    },
                    operand: 2,
                    ..
                }
            ));
            let error = begin_production_pliron_pass_contract_session_v1(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
            )
            .err()
            .expect("a foreign operand cannot enter the production analysis session");
            assert!(matches!(
                error,
                PlironPassPreservationErrorV1::IdentityUnavailable {
                    source_code: "FE2O3-PRESERVE-003",
                    ..
                }
            ));
            // Sparse edge typing now requires a definition in this function,
            // so even the raw test helper rejects before the bounds query.
            assert_eq!(
                run_pliron_ranked_bounds_check_v1(&context, &function).findings(),
                [RankedBoundsFindingV1::SparseIndexAnalysisFailed {
                    detail: "edge operand is not defined in this function".to_owned(),
                }]
            );
        }
    }
}
