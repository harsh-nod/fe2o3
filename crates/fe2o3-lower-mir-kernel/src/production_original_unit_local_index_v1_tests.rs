//! Scalability of the exact private row algorithms, not synthetic source authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirFunctionCoordinateV1 as Function,
};

fn coordinate(root: usize, local: usize) -> Coordinate {
    Coordinate {
        block: Block {
            function: Function(root as u32),
            block: local as u32,
        },
        operation: 0,
    }
}

fn rows<'a>(names: &'a [String], calls: usize, budget: &mut Budget<'_>) -> Rows<'a> {
    budget.reserve_storage(size_of::<Rows<'_>>()).unwrap();
    let mut rows = Rows {
        roots: unit_local_vec_v1(names.len(), budget).unwrap(),
        calls: unit_local_vec_v1(names.len() * calls, budget).unwrap(),
    };
    for (ordinal, name) in names.iter().enumerate().rev() {
        unit_local_push_v1(&mut rows.roots, RootRow { name, ordinal }, budget).unwrap();
        for local in (0..calls).rev() {
            unit_local_push_v1(
                &mut rows.calls,
                CallRow {
                    coordinate: coordinate(ordinal, local),
                    root: ordinal,
                    local,
                },
                budget,
            )
            .unwrap();
        }
    }
    rows
}

#[test]
fn original_n_index_lookup_work_is_logarithmic_for_many_roots_and_repeated_calls() {
    let mut previous = None;
    for count in [8usize, 64, 512] {
        let names = (0..count)
            .map(|index| format!("root{index:06}"))
            .collect::<Vec<_>>();
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 64 << 20);
        budget.reserve_storage(17).unwrap();
        let lookup = super::super::super::scoped(&mut budget, |budget| {
            let mut rows = rows(&names, 8, budget);
            rows.sort(budget)?;
            let before = budget.work();
            for (ordinal, name) in names.iter().enumerate() {
                assert_eq!(rows.root(name, budget)?.unwrap().ordinal, ordinal);
                for local in 0..8 {
                    let found = rows.call(coordinate(ordinal, local), budget)?.unwrap();
                    assert_eq!((found.root, found.local), (ordinal, local));
                }
            }
            let lookup = budget.work() - before;
            let root_bound = count * (count.ilog2() as usize + 1) * (2 * names[0].len() + 2);
            let calls = 8 * count;
            let call_bound = calls * (calls.ilog2() as usize + 1) * 4;
            assert!(
                lookup <= root_bound + call_bound,
                "{lookup} > indexed bound"
            );
            assert!(rows.root("absent", budget)?.is_none());
            assert!(rows.call(coordinate(count, 0), budget)?.is_none());
            Ok(lookup)
        })
        .unwrap();
        assert_eq!(budget.storage(), 17);
        if let Some(previous) = previous {
            // Eight times more roots/calls costs less than sixteen times the
            // lookup work; a per-query full scan would cost about sixty-four.
            assert!(lookup < previous * 16);
        }
        previous = Some(lookup);
    }
}

#[test]
fn original_n_index_rejects_duplicate_names_and_occurrences_before_lookup() {
    for duplicate_root in [false, true] {
        let names = vec!["root000000".to_owned(), "root000001".to_owned()];
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 1 << 20);
        budget.reserve_storage(23).unwrap();
        let result = super::super::super::scoped(&mut budget, |budget| {
            let mut rows = rows(&names, 2, budget);
            if duplicate_root {
                rows.roots[0].name = rows.roots[1].name;
            } else {
                rows.calls[0].coordinate = rows.calls[1].coordinate;
            }
            rows.sort(budget)
        });
        assert!(
            matches!(result, Err(E::CallRelation(actual)) if actual == if duplicate_root {
            "duplicate original root"
        } else { "duplicate original call coordinate" })
        );
        assert_eq!(budget.storage(), 23);
    }
}
