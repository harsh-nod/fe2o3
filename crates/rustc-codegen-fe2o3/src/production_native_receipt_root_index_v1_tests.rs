use super::super::super::{E, R, scoped};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
    },
};

fn scan(names: &[&str], wanted: &str) -> RootNameMatchV1 {
    let mut found = None;
    for (ordinal, name) in names.iter().enumerate() {
        if *name == wanted && found.replace(ordinal).is_some() {
            return RootNameMatchV1::Duplicate;
        }
    }
    found.map_or(RootNameMatchV1::Missing, RootNameMatchV1::Unique)
}

#[test]
fn receipt_root_index_matches_complete_scan_for_all_short_name_lists() {
    let alphabet = ["", "a", "A", "aa"];
    for length in 0..=5 {
        for mut encoding in 0..4usize.pow(length) {
            let names: Vec<_> = (0..length)
                .map(|_| {
                    let name = alphabet[encoding % alphabet.len()];
                    encoding /= alphabet.len();
                    name
                })
                .collect();
            let mut work = Work::new(1_000_000);
            let mut budget = Budget::new(&mut work, 100_000);
            budget.reserve_storage(31).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            scoped(&mut budget, |budget| {
                let index = ExactRootNameIndexV1::build(names.iter().copied(), budget)?;
                for wanted in ["", "a", "A", "aa", "aaa", "z"] {
                    assert_eq!(index.find(wanted, budget)?, scan(&names, wanted));
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), 31);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn receipt_root_index_preserves_four_independent_physical_axes() {
    let names = ["beta", "alpha", "delta", "gamma"];
    let axes = [[0, 1, 2, 3], [3, 0, 2, 1], [1, 3, 0, 2], [2, 0, 3, 1]];
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(37).unwrap();
    scoped(&mut budget, |budget| {
        let mut indexes = Vec::new();
        // This Vec belongs only to the test harness. Production uses four locals.
        for axis in &axes {
            indexes.push(ExactRootNameIndexV1::build(
                axis.iter().map(|i| names[*i]),
                budget,
            )?);
        }
        for name in names {
            for (axis, index) in axes.iter().zip(&indexes) {
                let expected = axis.iter().position(|i| names[*i] == name).unwrap();
                assert_eq!(index.find(name, budget)?, RootNameMatchV1::Unique(expected));
            }
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn receipt_root_index_exact_and_short_build_and_hit_miss_duplicate_budgets() {
    for (names, wanted, expected) in [
        (vec!["d", "c", "b", "a"], "c", RootNameMatchV1::Unique(1)),
        (vec!["d", "c", "b", "a"], "z", RootNameMatchV1::Missing),
        (vec!["b", "a", "a", "a"], "a", RootNameMatchV1::Duplicate),
        (vec!["z", "z"], "a", RootNameMatchV1::Missing),
    ] {
        for query in [false, true] {
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(41).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = scoped(&mut budget, |budget| {
                    let index = ExactRootNameIndexV1::build(names.iter().copied(), budget)?;
                    assert_eq!(
                        budget.storage(),
                        41 + size_of::<ExactRootNameIndexV1<'_>>()
                            + index.rows.capacity() * size_of::<Row<'_>>()
                    );
                    if query {
                        assert_eq!(index.find(wanted, budget)?, expected);
                    }
                    Ok(())
                });
                assert_eq!(budget.storage(), 41);
                assert!(budget.work_ledger_identity_v1() == ledger);
                (result, budget.work(), budget.peak_storage())
            };
            let (result, work, peak) = run(100_000, 100_000);
            result.unwrap();
            run(work, peak).0.unwrap();
            assert!(matches!(
                run(work - 1, peak).0,
                Err(E::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                run(work, peak - 1).0,
                Err(E::Resource(Resource::Storage(_)))
            ));
        }
    }
}

#[test]
fn receipt_root_index_charges_full_string_lengths_without_a_new_name_cap() {
    for length in [1, 128, 4096] {
        let name = "x".repeat(length);
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        scoped(&mut budget, |budget| {
            let index = ExactRootNameIndexV1::build(std::iter::once(name.as_str()), budget)?;
            let before = budget.work();
            assert_eq!(index.find(&name, budget)?, RootNameMatchV1::Unique(0));
            assert_eq!(budget.work() - before, 2 * length + 6);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

struct Counted<'a> {
    names: std::slice::Iter<'a, &'a str>,
    advertised: usize,
    drops: Arc<AtomicUsize>,
}
impl<'a> Iterator for Counted<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        self.names.next().copied()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.advertised, Some(self.advertised))
    }
}
impl ExactSizeIterator for Counted<'_> {}
impl Drop for Counted<'_> {
    fn drop(&mut self) {
        self.drops.fetch_add(1, AtomicOrdering::SeqCst);
    }
}

#[test]
fn receipt_root_index_refuses_bad_count_without_growth_and_drops_partial_rows() {
    for advertised in [0, 1, 3, MAX_ROOTS + 1] {
        let names = ["a", "b"];
        let drops = Arc::new(AtomicUsize::new(0));
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(43).unwrap();
        let result = scoped(&mut budget, |budget| {
            let _index = ExactRootNameIndexV1::build(
                Counted {
                    names: names.iter(),
                    advertised,
                    drops: Arc::clone(&drops),
                },
                budget,
            )?;
            Ok(())
        });
        assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
        assert_eq!(drops.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(budget.storage(), 43);
        if advertised > MAX_ROOTS {
            assert_eq!(budget.peak_storage(), 43);
        }
    }
}

#[test]
fn receipt_root_index_scoped_refusal_panic_and_foreign_ledger_preserve_floors() {
    for exit in 0..3 {
        for replace in [false, true] {
            let names = ["d", "b", "a", "c"];
            let mut work = Work::new(100_000);
            let mut foreign_work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 100_000);
            let mut foreign = Budget::new(&mut foreign_work, 100_000);
            budget.reserve_storage(47).unwrap();
            foreign.reserve_storage(59).unwrap();
            foreign.charge_work(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let foreign_ledger = foreign.work_ledger_identity_v1();
            let mut index_floor = 0;
            let mut index_work = 0;
            let result: R<()> = scoped(&mut budget, |budget| {
                let _index = ExactRootNameIndexV1::build(names.iter().copied(), budget)?;
                index_floor = budget.storage();
                index_work = budget.work();
                if replace {
                    std::mem::swap(budget, &mut foreign);
                }
                match exit {
                    0 => Ok(()),
                    1 => Err(E::Mismatch("indexed refusal")),
                    _ => panic!("indexed panic"),
                }
            });
            assert!(if replace {
                matches!(result, Err(E::Resource(Resource::Accounting)))
            } else {
                match exit {
                    0 => result.is_ok(),
                    1 => matches!(result, Err(E::Mismatch("indexed refusal"))),
                    _ => matches!(result, Err(E::Panicked)),
                }
            });
            if replace {
                assert_eq!(budget.storage(), 59);
                assert_eq!(budget.work(), 7);
                assert!(budget.work_ledger_identity_v1() == foreign_ledger);
                assert_eq!(foreign.storage(), index_floor);
                assert_eq!(foreign.work(), index_work);
                assert!(foreign.work_ledger_identity_v1() == ledger);
            } else {
                assert_eq!(budget.storage(), 47);
                assert_eq!(budget.work(), index_work);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(foreign.storage(), 59);
                assert_eq!(foreign.work(), 7);
            }
        }
    }
}

#[test]
fn receipt_root_index_hostile_payload_cannot_skip_row_floor_cleanup() {
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            panic!("indexed payload destructor");
        }
    }
    let names = ["c", "b", "a"];
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(53).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut charged = 0;
    let result = catch_unwind(AssertUnwindSafe(|| {
        scoped::<()>(&mut budget, |budget| {
            let _index = ExactRootNameIndexV1::build(names.iter().copied(), budget)?;
            charged = budget.work();
            std::panic::panic_any(Payload)
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 53);
    assert_eq!(budget.work(), charged);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn receipt_root_index_work_scales_with_logarithmic_search_at_admitted_root_cap() {
    for tables in [2, 4] {
        for layout in 0..3 {
            let mut previous = None;
            for count in [1usize, 2, 8, 32, MAX_ROOTS] {
                let names: Vec<_> = (0..count)
                    .map(|i| format!("{}{:03}", "p".repeat(32), i))
                    .collect();
                let order: Vec<_> = (0..count)
                    .map(|i| match layout {
                        0 => i,
                        1 => count - i - 1,
                        _ => (i * 7) % count,
                    })
                    .collect();
                let length = names[0].len();
                let height = (usize::BITS - count.leading_zeros()) as usize;
                // At most 2K sift calls, each <= H+1 visits. A visit pays <=4
                // loop/swap units and <=2 name+ordinal comparisons (2L+3).
                let build_bound =
                    2 + 2 * count + 2 * count * (height + 1) * (4 + 2 * (2 * length + 3));
                let lookup_bound = count * ((height + 2) * (2 * length + 3) + 1);
                let mut work = Work::new(10_000_000);
                let mut budget = Budget::new(&mut work, 100_000);
                budget.reserve_storage(61).unwrap();
                let mut built = 0;
                let mut queried = 0;
                scoped(&mut budget, |budget| {
                    let mut indexes = Vec::new();
                    for _ in 0..tables {
                        let before = budget.work();
                        indexes.push(ExactRootNameIndexV1::build(
                            order.iter().map(|i| names[*i].as_str()),
                            budget,
                        )?);
                        built += budget.work() - before;
                    }
                    let before = budget.work();
                    for index in &indexes {
                        for (physical, name) in order.iter().enumerate() {
                            assert_eq!(
                                index.find(&names[*name], budget)?,
                                RootNameMatchV1::Unique(physical)
                            );
                        }
                    }
                    queried = budget.work() - before;
                    Ok(())
                })
                .unwrap();
                assert!(
                    built <= tables * build_bound,
                    "{count}: {built} > {}",
                    tables * build_bound
                );
                assert!(
                    queried <= tables * lookup_bound,
                    "{count}: {queried} > {}",
                    tables * lookup_bound
                );
                if let Some((old_count, old_work)) = previous
                    && count == old_count * 4
                {
                    assert!(
                        queried < old_work * 12,
                        "quadratic lookup growth at {count}"
                    );
                }
                previous = Some((count, queried));
                assert_eq!(budget.storage(), 61);
            }
        }
    }
}
