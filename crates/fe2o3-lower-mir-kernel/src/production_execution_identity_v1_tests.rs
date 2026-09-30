use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

fn equations(
    kinds: &[ExecutionIdentityEquationKindV1],
    dependencies: &[&[usize]],
) -> ExecutionIdentityEquationsV1 {
    assert_eq!(kinds.len(), dependencies.len());
    let mut graph = ExecutionIdentityEquationsV1::default();
    for (&kind, dependencies) in kinds.iter().zip(dependencies) {
        let first = graph.inputs.len();
        graph.inputs.extend_from_slice(dependencies);
        graph.rows.push(ExecutionIdentityEquationV1 {
            kind,
            inputs: first..graph.inputs.len(),
        });
    }
    graph
}

#[test]
fn identity_equations_solve_anchored_cycles_without_order_or_multiplicity_loss() {
    use ExecutionIdentityEquationKindV1::{Input, Merge, Producer};
    for (graph, expected) in [
        (equations(&[], &[]), vec![]),
        (equations(&[Input], &[&[]]), vec![0]),
        (equations(&[Input, Merge], &[&[], &[0, 1]]), vec![0, 0]),
        (equations(&[Merge, Input], &[&[0, 1], &[]]), vec![1, 1]),
        (
            equations(&[Input, Producer, Merge], &[&[], &[0], &[1, 2]]),
            vec![0, 1, 1],
        ),
        (
            equations(&[Input, Merge, Merge], &[&[], &[2, 0, 0], &[1]]),
            vec![0, 0, 0],
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(73).unwrap();
        let classes = graph.solve(&mut budget).unwrap();
        assert_eq!(classes, expected);
        let retained = classes.capacity() * std::mem::size_of::<usize>();
        assert_eq!(budget.storage(), 73 + retained);
        drop(classes);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 73);
    }
}

#[test]
fn identity_equations_reject_conflicting_unanchored_and_malformed_dependencies() {
    use ExecutionIdentityEquationKindV1::{Input, Merge, Producer};
    let mut uncovered = equations(&[Input], &[&[]]);
    uncovered.inputs.push(0);
    let mut overlapping = equations(&[Input, Merge], &[&[], &[0]]);
    overlapping.rows[1].inputs = 1..1;
    for graph in [
        equations(&[Input, Input, Merge], &[&[], &[], &[0, 1]]),
        equations(&[Input, Merge, Producer], &[&[], &[0, 2], &[1]]),
        equations(&[Merge, Merge], &[&[1], &[0]]),
        equations(&[Producer], &[&[0]]),
        equations(&[Merge], &[&[]]),
        equations(&[Input], &[&[0]]),
        equations(&[Input, Merge], &[&[], &[2]]),
        uncovered,
        overlapping,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(73).unwrap();
        assert!(matches!(
            graph.solve(&mut budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "nominal identity equations differ from their original source",
                ..
            })
        ));
        assert_eq!(budget.storage(), 73);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn identity_solver_has_independent_linear_exact_and_one_short_limits() {
    use ExecutionIdentityEquationKindV1::{Input, Merge, Producer};
    let word = std::mem::size_of::<usize>();
    let header = 6 * 3 * word;
    assert_eq!(header, 6 * std::mem::size_of::<Vec<usize>>());
    assert_eq!(std::mem::size_of::<std::ops::Range<usize>>(), 2 * word);
    for graph in [
        equations(&[], &[]),
        equations(&[Input], &[&[]]),
        equations(&[Input, Merge], &[&[], &[0, 1]]),
        equations(&[Input, Producer, Merge], &[&[], &[0], &[1, 2]]),
    ] {
        let n = graph.rows.len();
        let e = graph.inputs.len();
        // Six allocation visits=18. Per node: initialization3, roster3,
        // prefix2, reverse fill2, single dequeue2, final2. Per dependency:
        // initialization1, incoming count2, reverse fill3, notification4.
        let required = 18 + 14 * n + 10 * e;
        let peak = header + (6 * n + e) * word;
        for floor in [0, 73] {
            for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
                let prior = 7;
                let mut work =
                    CanonicalKernelIrWorkBudgetV1::new(prior + required - usize::from(work_short));
                let mut budget =
                    ArgumentBudgetV1::new(&mut work, floor + peak - usize::from(storage_short));
                budget.charge_work(prior).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = graph.solve(&mut budget);
                assert_eq!(result.is_ok(), !work_short && !storage_short);
                match result {
                    Ok(classes) => {
                        assert_eq!(classes.capacity(), n);
                        assert_eq!(budget.storage(), floor + n * word);
                        assert_eq!(budget.peak_storage(), floor + peak);
                        assert_eq!(budget.work(), prior + required);
                        drop(classes);
                        budget.release_storage(n * word).unwrap();
                    }
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_),
                    )) if work_short => {}
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_),
                    )) if storage_short => {}
                    Err(error) => panic!("unexpected identity limit: {error:?}"),
                }
                assert_eq!(budget.storage(), floor);
                assert_eq!(
                    budget.failed_storage(),
                    storage_short.then_some(floor + peak)
                );
                drop(budget);
                assert_eq!(work.failed_work(), work_short.then_some(prior + required));
            }
        }
    }
}

#[test]
fn identity_solver_matches_independent_set_saturation_for_every_three_node_graph() {
    use ExecutionIdentityEquationKindV1::{Input, Merge, Producer};

    fn oracle(graph: &ExecutionIdentityEquationsV1) -> Option<Vec<usize>> {
        let mut sets = vec![0u8; graph.rows.len()];
        loop {
            let before = sets.clone();
            for (index, row) in graph.rows.iter().enumerate() {
                let dependencies = &graph.inputs[row.inputs.clone()];
                let derived = match row.kind {
                    Input => 1u8 << index,
                    Producer if dependencies.iter().all(|input| before[*input] != 0) => {
                        1u8 << index
                    }
                    Producer => 0,
                    Merge => dependencies
                        .iter()
                        .fold(0, |set, input| set | before[*input]),
                };
                sets[index] |= derived;
            }
            if sets == before {
                break;
            }
        }
        sets.iter()
            .map(|set| (set.count_ones() == 1).then(|| set.trailing_zeros() as usize))
            .collect()
    }

    let mut checked = 0;
    for n in 0..=3usize {
        let mut choices = vec![(Input, vec![]), (Producer, vec![])];
        for source in 0..n {
            choices.push((Producer, vec![source]));
            choices.push((Merge, vec![source]));
            for second in 0..n {
                choices.push((Merge, vec![source, second]));
            }
        }
        for mut code in 0..choices.len().pow(n as u32) {
            let mut graph = ExecutionIdentityEquationsV1::default();
            for _ in 0..n {
                let (kind, inputs) = &choices[code % choices.len()];
                code /= choices.len();
                let first = graph.inputs.len();
                graph.inputs.extend_from_slice(inputs);
                graph.rows.push(ExecutionIdentityEquationV1 {
                    kind: *kind,
                    inputs: first..graph.inputs.len(),
                });
            }
            let expected = oracle(&graph);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
            budget.reserve_storage(73).unwrap();
            match (graph.solve(&mut budget), expected) {
                (Ok(actual), Some(expected)) => {
                    assert_eq!(actual, expected, "{:?}", graph.rows);
                    let bytes = actual.capacity() * std::mem::size_of::<usize>();
                    assert_eq!(budget.storage(), 73 + bytes);
                    drop(actual);
                    budget.release_storage(bytes).unwrap();
                }
                (
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "nominal identity equations differ from their original source",
                        ..
                    }),
                    None,
                ) => {}
                (actual, expected) => panic!("{actual:?} != {expected:?}: {:?}", graph.rows),
            }
            assert_eq!(budget.storage(), 73);
            assert_eq!(budget.failed_storage(), None);
            drop(budget);
            assert_eq!(work.failed_work(), None);
            checked += 1;
        }
    }
    assert_eq!(checked, 5_019);
}

#[test]
fn identity_solver_preserves_preexisting_first_denial_histories() {
    use ExecutionIdentityEquationKindV1::{Input, Merge};
    let graph = equations(&[Input, Merge], &[&[], &[0, 1]]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000);
    budget.reserve_storage(73).unwrap();
    assert!(budget.charge_work(1_001).is_err());
    assert!(budget.reserve_storage(1_001 - 73).is_err());
    assert_eq!(budget.storage(), 73);
    let classes = graph.solve(&mut budget).unwrap();
    assert_eq!(classes, vec![0, 0]);
    assert_eq!(budget.work(), 18 + 14 * 2 + 10 * 2);
    let retained = classes.capacity() * std::mem::size_of::<usize>();
    drop(classes);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 73);
    assert_eq!(budget.failed_storage(), Some(1_001));
    drop(budget);
    assert_eq!(work.failed_work(), Some(1_001));
}
