//! Relation-only algorithm regressions; these fixtures admit no lifecycle witness.
//! Owner-backed tests check snapshot/lifecycle custody separately. Neither suite
//! establishes semantic equivalence.

use super::*;

fn coordinate(index: usize) -> Coordinate {
    Coordinate::Operation {
        function: 0,
        block: 0,
        operation: u32::try_from(index).unwrap(),
    }
}

fn operation(index: usize, source: bool) -> Node {
    Node {
        kind: Kind::Operation,
        input: source.then(|| Endpoint::Operation(coordinate(index))),
        result_index: None,
    }
}

fn replacement(from: usize, to: usize) -> Event {
    Event {
        pass: 0,
        change: Change::Replace(u32::try_from(from).unwrap(), u32::try_from(to).unwrap()),
    }
}

#[test]
fn long_singleton_chain_is_expanded_once_not_once_per_source() {
    const N: usize = 4096;
    let nodes = (0..N).map(|i| operation(i, true)).collect::<Vec<_>>();
    let events = (0..N - 1)
        .map(|i| replacement(i, i + 1))
        .collect::<Vec<_>>();
    let end = Endpoint::Operation(coordinate(N - 1));
    let mut terminal = vec![None; N];
    terminal[N - 1] = Some(end);
    let mut counts = RelationTraversalCountsV12::default();

    let (rows, targets, synthesized) =
        derive_relations_with_counts_v12(&nodes, &events, &terminal, N, &mut counts).unwrap();

    assert_eq!(counts.suffix_nodes, N);
    assert_eq!(counts.suffix_edges, N - 1);
    assert_eq!(counts.search_nodes, N);
    assert_eq!(counts.search_edges, 0);
    assert_eq!(rows.len(), N);
    assert_eq!(targets, vec![end; N]);
    assert!(synthesized.is_empty());
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.source(), coordinate(index));
        assert_eq!(row.targets, index..index + 1);
        assert_eq!(row.identity_survived(), index == N - 1);
        assert_eq!(row.disposition(), KirOptimizationDispositionV12::Merged);
    }
    assert_eq!(
        derive_relations(&nodes, &events, &terminal, N - 1),
        Err(KirOptimizationMapErrorV12::Limit)
    );
}

#[test]
fn many_sources_share_a_layered_singleton_diamond_summary() {
    const SOURCES: usize = 128;
    const LAYERS: usize = 64;
    const END: usize = SOURCES + 2 * LAYERS;
    let nodes = (0..=END)
        .map(|i| operation(i, i < SOURCES))
        .collect::<Vec<_>>();
    let mut events = Vec::new();
    for source in 0..SOURCES {
        events.push(replacement(source, SOURCES));
        events.push(replacement(source, SOURCES + 1));
    }
    for layer in 0..LAYERS - 1 {
        let first = SOURCES + 2 * layer;
        for from in [first, first + 1] {
            for to in [first + 2, first + 3] {
                events.push(replacement(from, to));
            }
        }
    }
    events.push(replacement(END - 2, END));
    events.push(replacement(END - 1, END));
    let end = Endpoint::Operation(coordinate(END));
    let mut terminal = vec![None; nodes.len()];
    terminal[END] = Some(end);
    let mut counts = RelationTraversalCountsV12::default();

    let (rows, targets, synthesized) =
        derive_relations_with_counts_v12(&nodes, &events, &terminal, SOURCES, &mut counts).unwrap();

    assert_eq!(counts.suffix_nodes, SOURCES + 2 * LAYERS + 1);
    assert_eq!(counts.suffix_edges, 2 * SOURCES + 4 * (LAYERS - 1) + 2);
    assert_eq!(counts.search_nodes, SOURCES);
    assert_eq!(counts.search_edges, 0);
    assert_eq!(rows.len(), SOURCES);
    assert_eq!(targets, vec![end; SOURCES]);
    assert!(synthesized.is_empty());
}

#[test]
fn empty_suffixes_do_not_repeat_search_or_consume_target_capacity() {
    const N: usize = 2048;
    let nodes = (0..N).map(|i| operation(i, true)).collect::<Vec<_>>();
    let events = (0..N - 1)
        .map(|i| replacement(i, i + 1))
        .collect::<Vec<_>>();
    let mut counts = RelationTraversalCountsV12::default();

    let (rows, targets, synthesized) =
        derive_relations_with_counts_v12(&nodes, &events, &vec![None; N], 0, &mut counts).unwrap();

    assert_eq!(counts.suffix_nodes, N);
    assert_eq!(counts.suffix_edges, N - 1);
    assert_eq!(counts.search_nodes, N);
    assert_eq!(counts.search_edges, 0);
    assert_eq!(rows.len(), N);
    assert!(rows.iter().all(|row| {
        row.disposition() == KirOptimizationDispositionV12::Eliminated
            && !row.identity_survived()
            && row.targets.is_empty()
    }));
    assert!(targets.is_empty());
    assert!(synthesized.is_empty());
}

#[test]
fn cached_and_direct_paths_deduplicate_before_the_exact_target_limit() {
    let nodes = (0..4).map(|i| operation(i, i == 0)).collect::<Vec<_>>();
    let terminal = [
        Some(Endpoint::Operation(coordinate(0))),
        None,
        None,
        Some(Endpoint::Operation(coordinate(3))),
    ];
    let expected = vec![
        Endpoint::Operation(coordinate(0)),
        Endpoint::Operation(coordinate(3)),
    ];
    let mut previous = None;
    // The source is Multiple because its own live identity survives. Exercise
    // both a direct leaf before a cached path and the reverse visitation order.
    for successors in [[1, 3], [3, 1]] {
        let events = [
            replacement(0, successors[0]),
            replacement(0, successors[1]),
            replacement(1, 2),
            replacement(2, 3),
        ];
        let mut counts = RelationTraversalCountsV12::default();
        let actual =
            derive_relations_with_counts_v12(&nodes, &events, &terminal, 2, &mut counts).unwrap();

        assert_eq!(counts.suffix_nodes, 4);
        assert_eq!(counts.suffix_edges, 4);
        assert_eq!(counts.search_edges, 2);
        assert!(counts.search_nodes <= 3);
        assert_eq!(actual.0.len(), 1);
        assert_eq!(
            actual.0[0].disposition(),
            KirOptimizationDispositionV12::Retained
        );
        assert!(actual.0[0].identity_survived());
        assert_eq!(actual.1, expected);
        assert!(actual.2.is_empty());
        if let Some(previous) = &previous {
            assert_eq!(&actual, previous);
        }
        previous = Some(actual);
        assert_eq!(
            derive_relations(&nodes, &events, &terminal, 1),
            Err(KirOptimizationMapErrorV12::Limit)
        );
    }
}

#[test]
fn multiple_live_intermediates_keep_every_endpoint_and_fallback_edge() {
    let nodes = [operation(0, true), operation(1, false), operation(2, false)];
    let events = [replacement(0, 1), replacement(1, 2)];
    let terminal = [
        None,
        Some(Endpoint::Operation(coordinate(1))),
        Some(Endpoint::Operation(coordinate(2))),
    ];
    let mut counts = RelationTraversalCountsV12::default();

    let (rows, targets, synthesized) =
        derive_relations_with_counts_v12(&nodes, &events, &terminal, 2, &mut counts).unwrap();

    assert_eq!(counts.suffix_nodes, 3);
    assert_eq!(counts.suffix_edges, 2);
    assert_eq!(counts.search_nodes, 3);
    assert_eq!(counts.search_edges, 2);
    assert_eq!(
        targets,
        vec![
            Endpoint::Operation(coordinate(1)),
            Endpoint::Operation(coordinate(2)),
        ]
    );
    assert_eq!(
        rows[0].disposition(),
        KirOptimizationDispositionV12::Replaced
    );
    assert!(!rows[0].identity_survived());
    assert!(synthesized.is_empty());
}

#[test]
fn suffix_join_is_empty_singleton_or_multiple_without_endpoint_sets() {
    let empty = 4;
    let multiple = 5;
    for (left, right, expected) in [
        (empty, empty, empty),
        (empty, 1, 1),
        (1, empty, 1),
        (1, 1, 1),
        (1, 2, multiple),
        (multiple, empty, multiple),
        (empty, multiple, multiple),
        (multiple, 1, multiple),
        (1, multiple, multiple),
        (multiple, multiple, multiple),
    ] {
        assert_eq!(
            merge_terminal_suffix_v12(left, right, empty, multiple),
            expected
        );
    }
}

#[test]
fn mixed_kind_suffixes_keep_live_operations_and_convergent_result_paths() {
    let a = Endpoint::Operation(coordinate(0));
    let b = Endpoint::Operation(coordinate(3));
    let result = Endpoint::Result {
        operation: coordinate(5),
        result: 0,
    };
    let original_a_result = Endpoint::Result {
        operation: coordinate(0),
        result: 0,
    };
    let original_b_result = Endpoint::Result {
        operation: coordinate(3),
        result: 0,
    };
    let result_node = |producer: u32, input| Node {
        kind: Kind::Value {
            producer: Some(producer),
        },
        input,
        result_index: Some(0),
    };
    let nodes = vec![
        operation(0, true),
        result_node(0, Some(original_a_result)),
        operation(2, false),
        operation(3, true),
        result_node(3, Some(original_b_result)),
        operation(5, false),
        result_node(5, None),
        operation(7, false),
        result_node(7, None),
    ];
    let terminal = [
        Some(a),
        Some(original_a_result),
        None,
        Some(b),
        Some(original_b_result),
        Some(Endpoint::Operation(coordinate(5))),
        Some(result),
        None,
        None,
    ];

    for cached_first in [false, true] {
        // Operations: A -> erased intermediate -> live B.
        // Values: both producers' results converge at result 6, with one path
        // through the erased result 8. Neither unchanged source result is an
        // operation relation endpoint.
        let (a_replacement, b_replacement) = if cached_first { (8, 6) } else { (6, 8) };
        let events = [
            replacement(0, 2),
            replacement(2, 3),
            replacement(1, a_replacement),
            replacement(4, b_replacement),
            replacement(8, 6),
        ];
        // The explicit-edge Kahn queue is [0, 1, 4, 5, 7, 2, 8, 3, 6].
        // Operation 3 precedes value 6 but follows value 8. A single reverse
        // traversal would visit operation 3 before its implicit value-8
        // successor when cached_first is false. Values-first summarization
        // must retain that result even in B's independent source row.
        let mut counts = RelationTraversalCountsV12::default();
        let (rows, targets, synthesized) =
            derive_relations_with_counts_v12(&nodes, &events, &terminal, 5, &mut counts).unwrap();

        assert_eq!(counts.suffix_nodes, 9);
        // Five explicit edges plus the three implicit producer-result paths.
        assert_eq!(counts.suffix_edges, 8);
        assert_eq!(counts.search_nodes, if cached_first { 6 } else { 7 });
        assert_eq!(counts.search_edges, 5);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].source(), coordinate(0));
        assert_eq!(rows[1].source(), coordinate(3));
        assert!(
            rows.iter()
                .all(|row| row.identity_survived() && !row.moved())
        );
        assert!(
            rows.iter()
                .all(|row| row.disposition() == KirOptimizationDispositionV12::Merged)
        );
        assert_eq!(targets, vec![a, b, result, b, result]);
        assert_eq!(&targets[rows[0].targets.clone()], &[a, b, result]);
        assert_eq!(&targets[rows[1].targets.clone()], &[b, result]);
        assert!(synthesized.is_empty());
        assert_eq!(
            derive_relations(&nodes, &events, &terminal, 4),
            Err(KirOptimizationMapErrorV12::Limit)
        );

        // Also pin the first row's exact cap. Once its three distinct ends
        // are emitted, a cached/direct duplicate must not attempt a fourth
        // append. Treat B and its result as generated, not source nodes.
        let mut one_source = nodes.clone();
        one_source[3].input = None;
        one_source[4].input = None;
        let (single_rows, single_targets, single_synthesized) =
            derive_relations(&one_source, &events, &terminal, 3).unwrap();
        assert_eq!(single_rows.len(), 1);
        assert_eq!(
            single_rows[0].disposition(),
            KirOptimizationDispositionV12::Retained
        );
        assert!(single_rows[0].identity_survived());
        assert_eq!(single_targets, vec![a, b, result]);
        assert!(single_synthesized.is_empty());
        assert_eq!(
            derive_relations(&one_source, &events, &terminal, 2),
            Err(KirOptimizationMapErrorV12::Limit)
        );
    }
}
