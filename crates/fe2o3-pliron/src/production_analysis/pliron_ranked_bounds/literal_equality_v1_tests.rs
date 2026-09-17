const LITERAL_EQUALITY_SOURCE: &str = r#"
builtin.func @literal_equality: builtin.function <() -> ()>
{
  ^entry():
    i = kernel.index_unknown () [] []: <() -> (kernel.index)>;
    n = kernel.index_unknown () [] []: <() -> (kernel.index)>;
    sibling = kernel.index_unknown () [] []: <() -> (kernel.index)>;
    equal_limit = kernel.index_constant () [] [kernel_index_value: kernel.index_value 128]: <() -> (kernel.index)>;
    bound_limit = kernel.index_constant () [] [kernel_index_value: kernel.index_value 128]: <() -> (kernel.index)>;
    view = kernel.ranked_view (n) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;
    kernel.index_eq_br (n, equal_limit) [^bound, ^exit] []: <(kernel.index, kernel.index) -> ()>
  ^bound():
    kernel.index_lt_br (i, bound_limit) [^access, ^exit] []: <(kernel.index, kernel.index) -> ()>
  ^access():
    kernel.access (view, i) [] [kernel_access_kind: kernel.access_kind AtomicWrite]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> ()>;
    kernel.br () [^exit] []: <() -> ()>
  ^exit():
    kernel.return () [] []: <() -> ()>
}
"#;

fn literal_equality_fact(graph: &BoundsEdgeTransportV1<'_>) -> LessThanFact {
    let entry = graph.blocks[0].deref(graph.context);
    let values = entry
        .iter(graph.context)
        .take(2)
        .map(|op| op.deref(graph.context).get_result(0))
        .collect::<Vec<_>>();
    LessThanFact {
        lhs: IndexExpr::Value(values[0]),
        rhs: IndexExpr::Value(values[1]),
    }
}

#[test]
fn literal_equality_rank_one_requires_exact_true_equality_and_less_edges() {
    assert_bounds(LITERAL_EQUALITY_SOURCE, true);
    assert_bounds(
        &LITERAL_EQUALITY_SOURCE.replace("(n, equal_limit)", "(equal_limit, n)"),
        true,
    );
    for (before, after) in [
        ("kernel.index_value 128", "kernel.index_value 127"),
        ("(n, equal_limit)", "(sibling, equal_limit)"),
        ("[^bound, ^exit]", "[^exit, ^bound]"),
        ("[^access, ^exit]", "[^exit, ^access]"),
        ("[^bound, ^exit]", "[^bound, ^bound]"),
        ("(i, bound_limit)", "(sibling, bound_limit)"),
    ] {
        // Only the equality constant changes in the first mutation.
        let changed = LITERAL_EQUALITY_SOURCE.replacen(before, after, 1);
        assert_ne!(changed, LITERAL_EQUALITY_SOURCE);
        assert_bounds(&changed, false);
    }
    let bypass = LITERAL_EQUALITY_SOURCE.replace(
        "kernel.index_eq_br (n, equal_limit) [^bound, ^exit] []: <(kernel.index, kernel.index) -> ()>",
        "kernel.br () [^bound] []: <() -> ()>",
    );
    assert_bounds(&bypass, false);
}

#[test]
fn literal_equality_does_not_change_legacy_transport_authority() {
    with_graph(LITERAL_EQUALITY_SOURCE, |graph| {
        let fact = literal_equality_fact(graph);
        assert!(
            !graph
                .proves(2, fact, &mut RankedBoundsBudget::default())
                .unwrap()
        );
        assert!(
            graph
                .proves_relation(2, fact, true, &mut RankedBoundsBudget::default())
                .unwrap()
        );
    });
}

#[test]
fn literal_equality_parallel_edges_and_phi_arguments_keep_exact_ordinals() {
    let source = LITERAL_EQUALITY_SOURCE
        .replace("kernel.index_eq_br (n, equal_limit) [^bound, ^exit] []: <(kernel.index, kernel.index) -> ()>",
            "kernel.index_eq_br_args (n, equal_limit, i, n) [^bound, ^exit] []: <(kernel.index, kernel.index, kernel.index, kernel.index) -> ()>")
        .replace("^bound():", "^bound(x: kernel.index, extent: kernel.index):")
        .replace("kernel.index_lt_br (i, bound_limit) [^access, ^exit] []: <(kernel.index, kernel.index) -> ()>",
            "kernel.index_lt_br_args (x, bound_limit, x, extent) [^access, ^exit] []: <(kernel.index, kernel.index, kernel.index, kernel.index) -> ()>")
        .replace("^access():", "^access(y: kernel.index, size: kernel.index):");
    with_graph(&source, |graph| {
        assert!(
            graph
                .proves_relation(
                    2,
                    arguments_fact(graph, 2),
                    true,
                    &mut RankedBoundsBudget::default()
                )
                .unwrap()
        );
    });
    for source in [
        source.replace("(n, equal_limit, i, n)", "(n, equal_limit, i, sibling)"),
        source.replace("(x, bound_limit, x, extent)", "(x, bound_limit, extent, x)"),
        source.replace("(n, equal_limit, i, n) [^bound, ^exit] []: <(kernel.index, kernel.index, kernel.index, kernel.index)",
            "(n, equal_limit, i, n, i, n) [^bound, ^bound] []: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index)"),
    ] {
        with_graph(&source, |graph| {
            assert!(!graph.proves_relation(2, arguments_fact(graph, 2), true, &mut RankedBoundsBudget::default()).unwrap());
        });
    }
}

#[test]
fn literal_equality_cycles_cannot_create_an_entry_or_fresh_definition_fact() {
    let source = LITERAL_EQUALITY_SOURCE
        .replace("kernel.index_eq_br (n, equal_limit) [^bound, ^exit] []: <(kernel.index, kernel.index) -> ()>",
            "kernel.br () [^bound] []: <() -> ()>")
        .replace("kernel.br () [^exit] []: <() -> ()>",
            "kernel.index_eq_br (n, equal_limit) [^bound, ^exit] []: <(kernel.index, kernel.index) -> ()>");
    assert_bounds(&source, false);
    let fresh = source.replace(
        "    n = kernel.index_unknown () [] []: <() -> (kernel.index)>;\n", "")
        .replace("    view = kernel.ranked_view (n) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;\n", "")
        .replace("  ^bound():", "  ^bound():\n    n = kernel.index_unknown () [] []: <() -> (kernel.index)>;\n    view = kernel.ranked_view (n) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;");
    assert_bounds(&fresh, false);
    let entry = LITERAL_EQUALITY_SOURCE.replace(
        "kernel.br () [^exit] []: <() -> ()>",
        "kernel.index_eq_br (n, equal_limit) [^entry, ^exit] []: <(kernel.index, kernel.index) -> ()>");
    with_graph(&entry, |graph| {
        let fact = literal_equality_fact(graph);
        assert!(
            !graph
                .proves_equal_literal(
                    0,
                    LessThanFact {
                        lhs: fact.rhs,
                        rhs: IndexExpr::Constant(128)
                    },
                    &mut RankedBoundsBudget::default()
                )
                .unwrap()
        );
    });
}

#[test]
fn literal_equality_nested_queries_share_exact_work_and_storage_limits() {
    with_graph(LITERAL_EQUALITY_SOURCE, |graph| {
        let fact = literal_equality_fact(graph);
        let mut measured = RankedBoundsBudget::default();
        assert!(graph.proves_relation(2, fact, true, &mut measured).unwrap());
        assert!(
            measured.storage_items >= 134,
            "both query buffers must be charged"
        );
        for (work_short, storage_short, success) in [(0, 0, true), (1, 0, false), (0, 1, false)] {
            let mut budget = RankedBoundsBudget {
                work_units: MAX_RANKED_BOUNDS_WORK_UNITS - measured.work_units + work_short,
                storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - measured.storage_items
                    + storage_short,
                ..RankedBoundsBudget::default()
            };
            let result = graph.proves_relation(2, fact, true, &mut budget);
            if success {
                assert_eq!(result, Ok(true));
                assert_eq!(budget.work_units, MAX_RANKED_BOUNDS_WORK_UNITS);
                assert_eq!(budget.storage_items, MAX_RANKED_BOUNDS_STORAGE_ITEMS);
            } else {
                assert!(matches!(
                    result,
                    Err(RankedBoundsFindingV1::ResourceLimitExceeded { .. })
                ));
            }
        }
    });
}

#[test]
fn literal_equality_preflight_reserves_no_argument_cfg_and_exact_limits() {
    let (context, function) = parsed(LITERAL_EQUALITY_SOURCE);
    let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
    let capture = provider
        .capture_with_resource_limits_v1(ProductionAnalysisResourceLimitsV1::new(
            usize::MAX,
            usize::MAX,
        ))
        .ok()
        .unwrap();
    let census = capture.input_census;
    assert_eq!(census.block_arguments, 0);
    assert_eq!(bounds_transport_resource_bound_v1(census).unwrap(), (0, 0));
    assert!(bounds_literal_equality_resource_bound_v1(census).0 > 0);
    let bound = preflight_ranked_bounds_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    for (work_short, storage_short, success) in [(0, 0, true), (1, 0, false), (0, 1, false)] {
        let result = preflight_ranked_bounds_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(
                bound.work_upper_bound() - work_short,
                bound.peak_storage_upper_bound() - storage_short,
            ),
        );
        assert_eq!(result.is_ok(), success);
    }
}
