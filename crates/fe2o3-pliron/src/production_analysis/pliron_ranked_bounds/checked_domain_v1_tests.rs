include!("checked_domain_fixture_v1_tests.rs");
include!("checked_domain_immediate_v1_tests.rs");
include!("checked_domain_worklist_v1_tests.rs");
include!("checked_domain_memo_v1_tests.rs");
include!("checked_domain_fold_v1_tests.rs");
include!("checked_domain_terminator_v1_tests.rs");

const ROW: CheckedDomainGeometryV1 = CheckedDomainGeometryV1::RowStriped([8, 2]);
const TILE: CheckedDomainGeometryV1 = CheckedDomainGeometryV1::Tiled([8, 4, 4, 2]);

fn assert_domain(source: &str, expected: bool) {
    with_domain_graph(source, |graph, function| {
        let actual = domain_query(graph, &mut RankedBoundsBudget::default());
        assert_eq!(actual, Ok(expected), "{source}");
        if expected {
            let report = run_pliron_ranked_bounds_check_v1(graph.context, function);
            assert!(report.is_clean(), "{report:?}\n{source}");
        }
    });
}

#[test]
fn checked_domain_real_runtime_families_reach_the_following_effect() {
    for geometry in [ROW, TILE] {
        assert_domain(&domain_source(geometry, FixtureOptions::default()), true);
    }
}

#[test]
fn checked_domain_forwarded_tuple_shared_shapes_and_commutation() {
    for geometry in [ROW, TILE] {
        assert_domain(
            &domain_source(
                geometry,
                FixtureOptions {
                    forwarded: true,
                    commute: true,
                    ..FixtureOptions::default()
                },
            ),
            true,
        );
    }
}

#[test]
fn checked_domain_component_and_definition_entry_guards_are_required() {
    assert_domain(
        &domain_source(
            ROW,
            FixtureOptions {
                omit_component: true,
                ..FixtureOptions::default()
            },
        ),
        false,
    );
    // The later c<E edge cannot retroactively define the earlier c*L.
    assert_domain(
        &domain_source(
            ROW,
            FixtureOptions {
                early_product: true,
                ..FixtureOptions::default()
            },
        ),
        false,
    );
}

#[test]
fn checked_domain_swapped_tuple_and_unmatched_extent_are_not_equations() {
    let source = domain_source(
        ROW,
        FixtureOptions {
            forwarded: true,
            ..FixtureOptions::default()
        },
    );
    let swapped = source.replace(
        "kernel.br_args (v, c, r, n, s, x)",
        "kernel.br_args (v, c, n, r, s, x)",
    );
    assert_ne!(swapped, source);
    assert_domain(&swapped, false);
    let extent = source.replace(
        "kernel.br_args (v, c, r, n, s, x)",
        "kernel.br_args (v, c, r, n, s, r)",
    );
    assert_ne!(extent, source);
    assert_domain(&extent, false);
    let geometry = source.replace(
        "kernel_lanes_per_row: kernel.index_value 8",
        "kernel_lanes_per_row: kernel.index_value 16",
    );
    assert_ne!(geometry, source);
    assert_domain(&geometry, false);
}

#[test]
fn checked_domain_ordered_arithmetic_and_divisor_guard_are_not_optional() {
    let source = domain_source(ROW, FixtureOptions::default());
    let swapped = source.replace(
        "kernel.index_binary (v, constant2)",
        "kernel.index_binary (constant2, v)",
    );
    assert_ne!(swapped, source);
    assert_domain(&swapped, false);
    // Keep the CFG and all later facts, but send both outcomes past S>0.
    let mut lines = source.lines().map(str::to_owned).collect::<Vec<_>>();
    let line = lines
        .iter_mut()
        .find(|line| line.contains("kernel.index_lt_br (constant0, s)"))
        .unwrap();
    let target = line
        .split("[^")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .to_owned();
    *line = line.replace(", ^exit]", &format!(", ^{target}]"));
    let missing = lines.join("\n");
    assert_ne!(missing, source);
    assert_domain(&missing, false);
}

#[test]
fn checked_domain_each_parallel_successor_payload_is_checked() {
    let source = domain_source(
        ROW,
        FixtureOptions {
            forwarded: true,
            ..FixtureOptions::default()
        },
    );
    let original = "kernel.br_args (v, c, r, n, s, x) [^access] []: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>";
    let parallel = "kernel.index_eq_br_args (v, c, v, c, r, n, s, x, v, c, r, n, s, x) [^access, ^access] []: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>";
    let unchanged = source.replace(original, parallel);
    assert_ne!(unchanged, source);
    assert_domain(&unchanged, true);
    let changed = unchanged.replace(
        "(v, c, v, c, r, n, s, x, v, c, r, n, s, x)",
        "(v, c, v, c, r, n, s, x, v, c, n, r, s, x)",
    );
    assert_ne!(changed, unchanged);
    assert_domain(&changed, false);
}

#[test]
fn checked_domain_dimension_guards_keep_the_exact_view_extent_owner() {
    let source = domain_source(ROW, FixtureOptions::default());
    let entry = "x: kernel.index):\n";
    let with_dimension = source.replace(entry,
        "x: kernel.index):\n    metadata = kernel.ranked_view (x) [] [kernel_memory_space: kernel.memory_space Global, kernel_allocation_origin: kernel.allocation_origin 1, kernel_noalias_class: kernel.noalias_class 1]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;\n    dimension_x = kernel.dim (metadata) [] [kernel_dimension: kernel.dimension 0]: <(kernel.ranked_view <32,true,[0]>) -> (kernel.index)>;\n")
        .replace(", x) [^", ", dimension_x) [^");
    assert_ne!(with_dimension, source);
    assert_domain(&with_dimension, true);
    let different_extent = with_dimension.replace(
        "metadata = kernel.ranked_view (x)",
        "metadata = kernel.ranked_view (r)",
    );
    assert_ne!(different_extent, with_dimension);
    assert_domain(&different_extent, false);
}

#[test]
fn checked_domain_same_name_partial_leaf_cannot_prove_its_own_definedness() {
    let source = domain_source(ROW, FixtureOptions::default());
    // Replace the entry invocation by an identical SSA value on both sides of
    // every equation, but define that value by an unguarded partial Divide.
    let source = source.replace("^entry(v: kernel.index,", "^entry(input: kernel.index,")
        .replace("x: kernel.index):\n", "x: kernel.index):\n    v = kernel.index_binary (input, c) [] [kernel_index_binary_kind: kernel.index_binary_kind Divide]: <(kernel.index, kernel.index) -> (kernel.index)>;\n");
    assert_domain(&source, false);
}

#[test]
fn checked_domain_forwarding_cycles_keep_the_guarded_initial_boundary() {
    let source = domain_source(
        ROW,
        FixtureOptions {
            forwarded: true,
            ..FixtureOptions::default()
        },
    );
    let exit = "    kernel.br () [^exit] []: <() -> ()>\n\n  ^exit():";
    let cycle = "    kernel.index_eq_br_args (av, ac, av, ac, ar, an, ast, ax) [^access, ^exit] []: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>\n\n  ^exit():";
    let invariant = source.replace(exit, cycle);
    assert_ne!(invariant, source);
    assert_domain(&invariant, true);
    let changed = invariant.replace("    kernel.index_eq_br_args (av, ac, av, ac, ar, an, ast, ax)",
        "    one = kernel.index_constant () [] [kernel_index_value: kernel.index_value 1]: <() -> (kernel.index)>;\n    next_v = kernel.index_binary (av, one) [] [kernel_index_binary_kind: kernel.index_binary_kind Add]: <(kernel.index, kernel.index) -> (kernel.index)>;\n    kernel.index_eq_br_args (av, ac, next_v, ac, ar, an, ast, ax)");
    assert_ne!(changed, invariant);
    assert_domain(&changed, false);
}

#[test]
fn checked_domain_owned_leaf_rejects_another_function_in_the_same_context() {
    let source = domain_source(ROW, FixtureOptions::default());
    let mut context = Context::new();
    fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    let first = parse_from_str(Operation::top_level_parser(), &mut context, &source).unwrap();
    let second = parse_from_str(
        Operation::top_level_parser(),
        &mut context,
        &source.replace("@checked_domain", "@other_owner"),
    )
    .unwrap();
    verify_operation(first, &context).unwrap();
    verify_operation(second, &context).unwrap();
    let first = FuncOp::from_operation(first);
    let second = FuncOp::from_operation(second);
    let blocks = first
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .collect::<Vec<_>>();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &blocks,
        predecessors: &[],
    };
    let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
        .unwrap()
        .unwrap();
    let proof = CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default()).unwrap();
    let foreign = second
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .next()
        .unwrap()
        .deref(&context)
        .get_argument(0);
    assert_eq!(
        proof.owned_value(foreign, &mut RankedBoundsBudget::default()),
        Err(RankedBoundsFindingV1::StructuralVerificationFailed)
    );
    assert_eq!(
        proof.owned_value(
            blocks[0].deref(&context).get_argument(0),
            &mut RankedBoundsBudget::default()
        ),
        Ok(blocks[0])
    );
}

fn wide_reference(geometry: CheckedDomainGeometryV1, values: [u64; 6]) -> Option<u64> {
    let [v, c, r, n, s, x] = values.map(u128::from);
    let maximum = u128::from(u64::MAX);
    let (row, column) = match geometry {
        CheckedDomainGeometryV1::RowStriped([lanes, elements]) => {
            let (lanes, elements) = (u128::from(lanes), u128::from(elements));
            if lanes == 0 || elements == 0 || c >= elements {
                return None;
            }
            (v / lanes, c * lanes + v % lanes)
        }
        CheckedDomainGeometryV1::Tiled([lanes, height, width, elements]) => {
            let (lanes, height, width, elements) = (
                u128::from(lanes),
                u128::from(height),
                u128::from(width),
                u128::from(elements),
            );
            if lanes == 0 || height == 0 || width == 0 || elements == 0 || c >= elements {
                return None;
            }
            let rounded = n + width - 1;
            if rounded > maximum {
                return None;
            }
            let tiles = rounded / width;
            if tiles == 0 {
                return None;
            }
            let tile = v / lanes;
            let lane = v % lanes;
            let local_row = lane / width * elements + c;
            let local_column = lane % width;
            (
                tile / tiles * height + local_row,
                tile % tiles * width + local_column,
            )
        }
    };
    if row > maximum || column > maximum || row >= r || column >= n || n > s || s == 0 {
        return None;
    }
    let result = row * s + column;
    (result <= maximum && result < x).then_some(result as u64)
}

fn constant_domain(geometry: CheckedDomainGeometryV1, values: [u64; 6]) -> bool {
    let context = Context::new();
    let graph = BoundsEdgeTransportV1 {
        context: &context,
        blocks: &[],
        predecessors: &[],
    };
    let mut budget = RankedBoundsBudget::default();
    let Some(dag) = CheckedDomainDagV1::build(geometry, &mut budget).unwrap() else {
        return false;
    };
    let mut proof =
        CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default()).unwrap();
    let folded = proof
        .fold(values.map(IndexExpr::Constant), &mut budget)
        .unwrap();
    proof.dag.formulas[..proof.dag.formula_count]
        .iter()
        .enumerate()
        .all(|(id, formula)| {
            let mut residual = CheckedDomainResidualV1::new(id as u8, &proof.dag);
            for (i, atom) in formula.atoms.iter().enumerate() {
                if residual.used() & (1 << i) == 0 {
                    continue;
                }
                let Some((lhs, rhs)) =
                    folded[usize::from(atom.lhs)].zip(folded[usize::from(atom.rhs)])
                else {
                    return false;
                };
                residual.observe(i, lhs < rhs);
            }
            residual.proven()
        })
}

#[test]
fn checked_domain_unsigned_wide_oracle_covers_invalid_and_boundary_domains() {
    let edges = [0, 1, 2, 7, 8, 15, 16, u64::MAX / 2, u64::MAX - 3, u64::MAX];
    for geometry in [ROW, TILE, CheckedDomainGeometryV1::Tiled([4, 16, 1, 4])] {
        for invocation in edges {
            for component in [0, 1, 2, u64::MAX] {
                for [rows, columns, stride, extent] in [
                    [16, 16, 16, 256],
                    [0, 16, 16, 256],
                    [16, 0, 16, 256],
                    [16, 16, 0, 256],
                    [16, 16, 15, 256],
                    [16, 16, 16, 0],
                    [16, 3, 4, 64],
                    [u64::MAX, 1, 1, u64::MAX],
                    [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
                    [u64::MAX, u64::MAX - 3, u64::MAX - 3, u64::MAX],
                ] {
                    let values = [invocation, component, rows, columns, stride, extent];
                    assert_eq!(
                        constant_domain(geometry, values),
                        wide_reference(geometry, values).is_some(),
                        "{geometry:?} {values:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn checked_domain_carry_retains_the_quotient_tail_clause() {
    let formula = CheckedDomainFormulaV1 {
        atoms: [CheckedDomainAtomV1 { lhs: 0, rhs: 0 }; 3],
        positive: [1, 0],
        negative: [0, 7],
        clauses: 3,
        depth: 0,
    };
    for divisor in [1, 2, 3, 8, u64::MAX] {
        let quotient = u64::MAX / divisor;
        let remainder = u64::MAX % divisor;
        for a in [
            quotient.saturating_sub(1),
            quotient,
            quotient.saturating_add(1),
        ] {
            for c in [0, remainder, divisor - 1] {
                let mut residual = CheckedDomainResidualV1 {
                    formula: 0,
                    positive: formula.positive,
                    negative: formula.negative,
                    clauses: formula.clauses,
                };
                for (atom, truth) in [a < quotient, quotient < a, remainder < c]
                    .into_iter()
                    .enumerate()
                {
                    residual.observe(atom, truth);
                }
                let expected =
                    u128::from(a) * u128::from(divisor) + u128::from(c) <= u128::from(u64::MAX);
                assert_eq!(residual.proven(), expected, "{a}*{divisor}+{c}");
            }
        }
    }
}

#[test]
fn checked_domain_invalid_geometry_is_not_authority() {
    for geometry in [
        CheckedDomainGeometryV1::RowStriped([0, 1]),
        CheckedDomainGeometryV1::RowStriped([u64::MAX, 2]),
        CheckedDomainGeometryV1::Tiled([8, 4, 0, 2]),
        CheckedDomainGeometryV1::Tiled([8, 5, 4, 2]),
        CheckedDomainGeometryV1::Tiled([2, u64::MAX, 1, u64::MAX]),
    ] {
        let mut budget = RankedBoundsBudget::default();
        assert!(
            CheckedDomainDagV1::build(geometry, &mut budget)
                .unwrap()
                .is_none()
        );
        assert_eq!((budget.work_units, budget.storage_items), (20, 0));
    }
    // Preserve the dialect's complete geometry predicate, including this
    // representable row identity despite overflow of both area products.
    assert!(
        CheckedDomainDagV1::build(
            CheckedDomainGeometryV1::Tiled([u64::MAX, u64::MAX, u64::MAX, u64::MAX,]),
            &mut RankedBoundsBudget::default()
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn checked_domain_constants_fold_without_equating_invalid_checked_results() {
    for geometry in [ROW, TILE] {
        for values in [
            [3, 1, 16, 16, 16, 256],
            [u64::MAX, 1, 16, 16, 16, 256],
            [3, 1, 16, 16, 0, 256],
        ] {
            let expected = wide_reference(geometry, values).is_some();
            assert_domain(
                &domain_source(
                    geometry,
                    FixtureOptions {
                        constants: Some(values),
                        ..FixtureOptions::default()
                    },
                ),
                expected,
            );
        }
    }
}

#[test]
fn checked_domain_zero_argument_function_still_has_an_admitted_query() {
    let source = domain_source(
        ROW,
        FixtureOptions {
            constants: Some([3, 1, 16, 16, 16, 256]),
            ..FixtureOptions::default()
        },
    );
    let no_arguments = source.replace("builtin.function <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>", "builtin.function <() -> ()>")
        .replace("^entry(v: kernel.index, c: kernel.index, r: kernel.index, n: kernel.index, s: kernel.index, x: kernel.index):", "^entry():");
    assert_ne!(no_arguments, source);
    assert_domain(&no_arguments, true);
    let census = ProductionAnalysisInputCensusV1 {
        ranked_accesses: 1,
        operands: 1,
        ..ProductionAnalysisInputCensusV1::default()
    };
    // Runtime work cap plus the largest possible denied-prefix envelope:
    // 1 + 1024*128 + 912*32 + 2611*2 + 6*B0 + 5 + 64*768*4 + 40*4 + 256.
    assert_eq!(
        checked_domain_resource_bound_v1(census),
        Ok((8_751_116, 131_072))
    );
    assert_eq!(
        checked_domain_resource_bound_v1(ProductionAnalysisInputCensusV1::default()),
        Ok((0, 0))
    );
    assert!(
        checked_domain_resource_bound_v1(ProductionAnalysisInputCensusV1 {
            results: usize::MAX,
            block_arguments: 1,
            ..census
        })
        .is_err()
    );
}

#[test]
fn checked_domain_state_admission_has_literal_failure_prefixes() {
    let state = CheckedDomainStateV1 {
        block: 0,
        environment: [IndexExpr::Constant(0); 6],
        goal: CheckedDomainGoalV1::Defined(IndexExpr::Constant(0)),
    };
    let mut budget = RankedBoundsBudget::default();
    let mut visited = Vec::new();
    checked_domain_enqueue_v1(&mut visited, state, &mut budget).unwrap();
    assert_eq!(
        (budget.work_units, budget.storage_items, visited.len()),
        (135, 256, 1)
    );
    checked_domain_enqueue_v1(&mut visited, state, &mut budget).unwrap();
    assert_eq!(
        (budget.work_units, budget.storage_items, visited.len()),
        (201, 256, 1)
    );
    let mut denied = RankedBoundsBudget {
        work_units: MAX_RANKED_BOUNDS_WORK_UNITS - 134,
        ..RankedBoundsBudget::default()
    };
    let mut empty = Vec::new();
    assert!(
        matches!(checked_domain_enqueue_v1(&mut empty, state, &mut denied), Err(RankedBoundsFindingV1::ResourceLimitExceeded { resource: "analysis work unit", actual, .. }) if actual == MAX_RANKED_BOUNDS_WORK_UNITS + 1)
    );
    assert_eq!(
        (denied.work_units, denied.storage_items, empty.len()),
        (MAX_RANKED_BOUNDS_WORK_UNITS - 128, 256, 0)
    );
    let mut denied = RankedBoundsBudget {
        storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - 255,
        ..RankedBoundsBudget::default()
    };
    let mut empty = Vec::new();
    assert!(
        matches!(checked_domain_enqueue_v1(&mut empty, state, &mut denied), Err(RankedBoundsFindingV1::ResourceLimitExceeded { resource: "analysis storage item", actual, .. }) if actual == MAX_RANKED_BOUNDS_STORAGE_ITEMS + 1)
    );
    assert_eq!(
        (
            denied.work_units,
            denied.storage_items,
            empty.len(),
            empty.capacity()
        ),
        (1, MAX_RANKED_BOUNDS_STORAGE_ITEMS - 255, 0, 0)
    );
}

#[test]
fn checked_domain_fixed_payloads_and_publication_cap_are_explicit() {
    let word = core::mem::size_of::<usize>();
    assert!(
        core::mem::size_of::<CheckedDomainStateV1>().div_ceil(word)
            <= CHECKED_DOMAIN_STATE_ITEMS_V1
    );
    assert!(
        core::mem::size_of::<Option<CheckedDomainMemoV1>>().div_ceil(word)
            <= CHECKED_DOMAIN_STATE_ITEMS_V1
    );
    assert!(
        core::mem::size_of::<CheckedDomainDagV1>().div_ceil(word) <= CHECKED_DOMAIN_DAG_ITEMS_V1
    );
    assert!(
        6 * core::mem::size_of::<CheckedDomainStateV1>().div_ceil(word)
            + 2 * core::mem::size_of::<[Option<u64>; CHECKED_DOMAIN_NODES_V1]>().div_ceil(word)
            + 144
            <= CHECKED_DOMAIN_FRAME_ITEMS_V1
    );
    assert!(
        core::mem::size_of::<CheckedDomainProofV1<'_, '_>>().div_ceil(word)
            <= CHECKED_DOMAIN_DAG_ITEMS_V1 + CHECKED_DOMAIN_QUERY_ITEMS_V1
    );
    assert!(
        core::mem::size_of::<Option<pliron::context::Ptr<Operation>>>().div_ceil(word)
            <= CHECKED_DOMAIN_TERMINATOR_ROW_ITEMS_V1
    );
    assert_eq!(
        core::mem::size_of::<Vec<CheckedDomainStateV1>>().div_ceil(word),
        3
    );
    let state = CheckedDomainStateV1 {
        block: 0,
        environment: [IndexExpr::Constant(0); 6],
        goal: CheckedDomainGoalV1::Defined(IndexExpr::Constant(0)),
    };
    let mut visited = (0..MAX_RANKED_BOUNDS_FACTS)
        .map(|block| CheckedDomainStateV1 { block, ..state })
        .collect::<Vec<_>>();
    let mut budget = RankedBoundsBudget {
        storage_items: MAX_RANKED_BOUNDS_FACTS * CHECKED_DOMAIN_STATE_ITEMS_V1,
        ..RankedBoundsBudget::default()
    };
    assert!(
        matches!(checked_domain_enqueue_v1(&mut visited, CheckedDomainStateV1 { block: MAX_RANKED_BOUNDS_FACTS, ..state }, &mut budget),
        Err(RankedBoundsFindingV1::ResourceLimitExceeded { resource: "checked domain obligation", actual, .. }) if actual == MAX_RANKED_BOUNDS_FACTS + 1)
    );
    assert_eq!(
        (budget.work_units, budget.storage_items, visited.len()),
        (66_561, 65_536, 1_024)
    );
}

#[test]
fn checked_domain_forwarded_dimension_checks_metadata_before_constant_folding() {
    let source = r#"builtin.func @dimension_owner: builtin.function <() -> ()>
{
  ^entry():
    size = kernel.index_constant () [] [kernel_index_value: kernel.index_value 256]: <() -> (kernel.index)>;
    metadata = kernel.ranked_view (size) [] [kernel_memory_space: kernel.memory_space Global, kernel_allocation_origin: kernel.allocation_origin 0, kernel_noalias_class: kernel.noalias_class 0]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;
    dimension = kernel.dim (metadata) [] [kernel_dimension: kernel.dimension 0]: <(kernel.ranked_view <32,true,[0]>) -> (kernel.index)>;
    kernel.br_args (size, size, size, size, size, dimension) [^target] []: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>
  ^target(a: kernel.index, b: kernel.index, c: kernel.index, d: kernel.index, e: kernel.index, f: kernel.index):
    kernel.return () [] []: <() -> ()>
}
"#;
    for foreign_view in [false, true] {
        let mut context = Context::new();
        fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let first = parse_from_str(Operation::top_level_parser(), &mut context, source).unwrap();
        let second = parse_from_str(
            Operation::top_level_parser(),
            &mut context,
            &source.replace("@dimension_owner", "@foreign_dimension_owner"),
        )
        .unwrap();
        verify_operation(first, &context).unwrap();
        verify_operation(second, &context).unwrap();
        let first = FuncOp::from_operation(first);
        let second = FuncOp::from_operation(second);
        let blocks = first
            .get_region(&context)
            .deref(&context)
            .iter(&context)
            .collect::<Vec<_>>();
        assert_eq!(blocks.len(), 2);
        let foreign_entry = second
            .get_region(&context)
            .deref(&context)
            .iter(&context)
            .next()
            .unwrap();
        let local_dimension = blocks[0]
            .deref(&context)
            .iter(&context)
            .find(|operation| Operation::is_op::<DimensionOp>(*operation, &context))
            .unwrap();
        let local_view = blocks[0]
            .deref(&context)
            .iter(&context)
            .find(|operation| Operation::is_op::<RankedViewOp>(*operation, &context))
            .unwrap();
        let foreign_view_op = foreign_entry
            .deref(&context)
            .iter(&context)
            .find(|operation| Operation::is_op::<RankedViewOp>(*operation, &context))
            .unwrap();
        let foreign_constant = foreign_entry
            .deref(&context)
            .iter(&context)
            .find(|operation| Operation::is_op::<IndexConstantOp>(*operation, &context))
            .unwrap();
        let predecessors = vec![
            Vec::new(),
            vec![PredecessorEdge {
                block: 0,
                successor: 0,
                guard_fact: None,
            }],
        ];
        let graph = BoundsEdgeTransportV1 {
            context: &context,
            blocks: &blocks,
            predecessors: &predecessors,
        };
        let environment = core::array::from_fn(|index| {
            IndexExpr::Value(blocks[1].deref(&context).get_argument(index))
        });
        let state = CheckedDomainStateV1 {
            block: 1,
            environment,
            goal: CheckedDomainGoalV1::Equal {
                actual: environment[5],
                expected: 5,
            },
        };
        let edge = &predecessors[1][0];
        let query = || {
            let dag = CheckedDomainDagV1::build(ROW, &mut RankedBoundsBudget::default())
                .unwrap()
                .unwrap();
            let mut proof =
                CheckedDomainProofV1::new(&graph, dag, &mut RankedBoundsBudget::default()).unwrap();
            proof.pull_environment(state, edge, &mut RankedBoundsBudget::default())
        };
        assert_eq!(
            query(),
            Ok((
                [IndexExpr::Constant(256); 6],
                Some(IndexExpr::Constant(256))
            ))
        );
        if foreign_view {
            let replacement = foreign_view_op.deref(&context).get_result(0);
            Operation::replace_operand(local_dimension, &context, 0, replacement);
        } else {
            let replacement = foreign_constant.deref(&context).get_result(0);
            Operation::replace_operand(local_view, &context, 0, replacement);
        }
        // The existing production identity boundary already rejects this IR.
        // The direct helper must independently reject before folding metadata.
        assert!(matches!(
            crate::derive_pliron_ir_structural_identity_v1(&context, &first),
            Err(crate::PlironIrIdentityErrorV1::ExternalOperand { .. })
        ));
        assert_eq!(
            query(),
            Err(RankedBoundsFindingV1::StructuralVerificationFailed)
        );
    }
}

#[test]
fn checked_domain_real_query_replays_at_exact_work_and_storage_without_slack() {
    let source = domain_source(
        ROW,
        FixtureOptions {
            constants: Some([3, 1, 16, 16, 16, 256]),
            ..FixtureOptions::default()
        },
    );
    with_domain_graph(&source, |graph, _| {
        let mut baseline = RankedBoundsBudget::default();
        assert_eq!(domain_query(graph, &mut baseline), Ok(true));
        // Row DAG: N18/F7/depth8. Fold owner160 cells/320 work;
        // terminator owner3*B11+3=36 cells, plus72 init/drop and5 control work.
        // The seven identical projected environments use one704 miss and
        // six202 hits: 53354+320+704+6*202-7*512+77 = 52083 work.
        // DAG912 + query2611 + terminators33 + frames8*768 + queue256 = 9956.
        assert_eq!(graph.blocks.len(), 11);
        assert_eq!(
            (baseline.work_units, baseline.storage_items),
            (52_083, 9_956)
        );
        for (work, storage) in [
            (baseline.work_units, baseline.storage_items),
            (baseline.work_units - 1, baseline.storage_items),
            (baseline.work_units, baseline.storage_items - 1),
        ] {
            let mut meter = RankedBoundsBudget {
                work_units: MAX_RANKED_BOUNDS_WORK_UNITS - work,
                storage_items: MAX_RANKED_BOUNDS_STORAGE_ITEMS - storage,
                ..RankedBoundsBudget::default()
            };
            let result = domain_query(graph, &mut meter);
            if work == baseline.work_units && storage == baseline.storage_items {
                assert_eq!(result, Ok(true));
                assert_eq!(
                    (meter.work_units, meter.storage_items),
                    (
                        MAX_RANKED_BOUNDS_WORK_UNITS,
                        MAX_RANKED_BOUNDS_STORAGE_ITEMS
                    )
                );
            } else {
                assert!(matches!(
                    result,
                    Err(RankedBoundsFindingV1::ResourceLimitExceeded { .. })
                ));
            }
        }
    });
}
