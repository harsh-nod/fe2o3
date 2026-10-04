//! Graph/worklist tests only; these do not establish rustc source admission.

use super::*;

fn local(index: usize) -> Local {
    Local::from_usize(index)
}

fn locals(indices: &[usize]) -> BTreeSet<Local> {
    indices.iter().copied().map(local).collect()
}

fn aliases(edges: &[(usize, usize)]) -> BTreeMap<Local, Local> {
    edges
        .iter()
        .map(|&(destination, source)| (local(destination), local(source)))
        .collect()
}

fn graph(
    writes: &[(usize, Option<usize>)],
    charge: &mut impl FnMut(usize) -> Result<(), ClosureProfileErrorV1>,
) -> Result<AliasGraph, ClosureProfileErrorV1> {
    let mut graph = AliasGraph::default();
    for &(destination, source) in writes {
        graph.record_write(local(destination), source.map(local), charge)?;
    }
    Ok(graph)
}

#[test]
fn reversed_edges_flatten_values_and_references_to_exact_roots() {
    let mut writes = [
        (5, Some(4)),
        (4, Some(3)),
        (3, Some(2)),
        (2, Some(1)),
        (8, Some(7)),
    ];
    let values = aliases(&[(2, 1), (3, 2)]);
    let roots = locals(&[1, 7]);
    let expected = aliases(&[(2, 1), (3, 1), (4, 1), (5, 1), (8, 7)]);
    let mut costs = Vec::new();
    for _ in 0..2 {
        let mut cost = 0;
        let mut charge = |amount| {
            cost += amount;
            Ok(())
        };
        let result = graph(&writes, &mut charge)
            .unwrap()
            .resolve(&roots, &values, &mut charge);
        assert_eq!(result.unwrap(), expected);
        costs.push(cost);
        writes.reverse();
    }
    assert_eq!(costs[0], costs[1]);
}

#[test]
fn value_edges_remain_direct_after_cycle_validation() {
    let mut charge = |_| Ok(());
    let graph = graph(&[(3, Some(2)), (2, Some(1))], &mut charge).unwrap();
    assert_eq!(
        graph
            .finish_values(&locals(&[1, 2, 3]), &mut charge)
            .unwrap(),
        aliases(&[(2, 1), (3, 2)])
    );
}

#[test]
fn duplicate_alias_and_non_alias_writes_are_rejected_in_either_order() {
    for mut writes in [
        [(2, Some(1)), (2, Some(1))],
        [(2, Some(1)), (2, Some(3))],
        [(2, Some(1)), (2, None)],
    ] {
        for _ in 0..2 {
            let mut charge = |_| Ok(());
            let graph = graph(&writes, &mut charge).unwrap();
            let error = graph
                .resolve(&locals(&[1, 3]), &BTreeMap::new(), &mut charge)
                .unwrap_err();
            assert!(error.to_string().contains("assigned more than once"));
            assert!(
                graph
                    .finish_values(&locals(&[1, 2, 3]), &mut charge)
                    .is_err()
            );
            writes.reverse();
        }
    }
}

#[test]
fn unresolved_value_cycles_and_their_dependents_are_rejected() {
    for writes in [
        vec![(2, Some(2))],
        vec![(2, Some(3)), (3, Some(2)), (4, Some(2))],
    ] {
        let mut charge = |_| Ok(());
        let graph = graph(&writes, &mut charge).unwrap();
        let error = graph
            .finish_values(&locals(&[1, 2, 3, 4]), &mut charge)
            .unwrap_err();
        assert!(error.to_string().contains("cycle"));
        let values = writes
            .iter()
            .map(|&(destination, source)| (local(destination), local(source.unwrap())))
            .collect();
        assert!(graph.resolve(&locals(&[1]), &values, &mut charge).is_err());
    }
}

#[test]
fn unrelated_ordinary_cycles_and_repeated_writes_are_ignored() {
    let mut charge = |_| Ok(());
    let graph = graph(
        &[
            (2, Some(1)),
            (10, Some(11)),
            (11, Some(10)),
            (12, Some(13)),
            (12, Some(13)),
            (0, Some(10)),
        ],
        &mut charge,
    )
    .unwrap();
    assert_eq!(
        graph
            .resolve(&locals(&[1]), &BTreeMap::new(), &mut charge)
            .unwrap(),
        aliases(&[(2, 1)])
    );
}

#[test]
fn alias_writes_cannot_overwrite_a_root_even_from_an_unresolved_source() {
    let mut charge = |_| Ok(());
    let graph = graph(&[(1, Some(7))], &mut charge).unwrap();
    assert!(
        graph
            .resolve(&locals(&[1, 7]), &BTreeMap::new(), &mut charge)
            .is_err()
    );
    assert!(
        graph
            .resolve(&locals(&[1]), &BTreeMap::new(), &mut charge)
            .is_err()
    );
}

#[test]
fn forwarding_values_or_references_to_return_local_is_rejected() {
    let mut charge = |_| Ok(());
    let graph = graph(&[(0, Some(1))], &mut charge).unwrap();
    let error = graph
        .resolve(&locals(&[1]), &BTreeMap::new(), &mut charge)
        .unwrap_err();
    assert!(error.to_string().contains("returning a forwarded closure"));
    assert!(graph.finish_values(&locals(&[0, 1]), &mut charge).is_err());
}

#[test]
fn required_value_edges_cannot_resolve_to_a_different_root() {
    let mut charge = |_| Ok(());
    let graph = graph(&[(2, Some(1))], &mut charge).unwrap();
    assert!(
        graph
            .resolve(&locals(&[1, 7]), &aliases(&[(2, 7)]), &mut charge)
            .is_err()
    );
    assert!(
        graph
            .resolve(&locals(&[1]), &aliases(&[(3, 1)]), &mut charge)
            .is_err()
    );
}

#[test]
fn graph_work_accepts_exact_charge_and_propagates_one_short_failure() {
    fn run(
        charge: &mut impl FnMut(usize) -> Result<(), ClosureProfileErrorV1>,
    ) -> Result<BTreeMap<Local, Local>, ClosureProfileErrorV1> {
        let values = graph(&[(3, Some(2)), (2, Some(1))], charge)?
            .finish_values(&locals(&[1, 2, 3]), charge)?;
        graph(
            &[(5, Some(4)), (4, Some(3)), (3, Some(2)), (2, Some(1))],
            charge,
        )?
        .resolve(&locals(&[1]), &values, charge)
    }
    let mut total = 0;
    let expected = run(&mut |amount| {
        total += amount;
        Ok(())
    })
    .unwrap();
    assert_eq!(total, 64);
    let exhausted = ClosureProfileErrorV1::new("test charge exhausted");
    for limit in [total, total - 1] {
        let mut remaining = limit;
        let result = run(&mut |amount| {
            remaining = remaining
                .checked_sub(amount)
                .ok_or_else(|| exhausted.clone())?;
            Ok(())
        });
        if limit == total {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(remaining, 0);
        } else {
            assert_eq!(result.unwrap_err(), exhausted);
        }
    }
}

#[test]
fn failed_charge_precedes_write_and_edge_allocations() {
    let mut graph = AliasGraph::default();
    let result = graph.record_write(local(2), Some(local(1)), &mut |_| {
        Err(ClosureProfileErrorV1::new("test charge exhausted"))
    });
    assert!(result.is_err());
    assert!(graph.writes.is_empty());
    assert!(graph.outgoing.is_empty());

    let mut remaining = 1usize;
    let result = graph.record_write(local(2), Some(local(1)), &mut |amount| {
        remaining = remaining
            .checked_sub(amount)
            .ok_or_else(|| ClosureProfileErrorV1::new("test charge exhausted"))?;
        Ok(())
    });
    assert!(result.is_err());
    assert!(graph.outgoing.is_empty());
}
