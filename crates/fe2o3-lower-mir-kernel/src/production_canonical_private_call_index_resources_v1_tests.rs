use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::mem::{size_of, size_of_val};

const INDEX_FLOOR: usize = 37;

// A component observation of the actual source entry, not a public proof hook.
pub(crate) fn read_test_private_index_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    donor: Option<&ProductionCanonicalScalarFixedPointOwnerV1>,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
) -> CsResultV1<()> {
    cpc_scope_v1(budget, |budget| {
        owner
            .original
            .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(cpc_with_source_v1(
                    view,
                    budget,
                    |source, coverage, _, budget| {
                        let mut origins = CpcOriginsV1::derive(source, budget)?;
                        if (13..=15).contains(&fault) || fault == 254 {
                            let mut transport =
                                CpcTransportV1::original(&origins, coverage, budget)?;
                            let mut lineage = cs_lineage_with_observer_v1(
                                owner,
                                source.inventory,
                                &mut transport,
                                budget,
                            )?;
                            let (output, receipt) =
                                CanonicalKirInventoryV1::derive(owner.output(), budget)?;
                            budget.reserve_storage(receipt.retained_storage())?;
                            match fault {
                                13 => {
                                    lineage.functions.pop().unwrap();
                                }
                                14 => lineage.functions[1] = lineage.functions[0],
                                15 => lineage.functions[0] = CsFunctionV1(u32::MAX),
                                254 => {}
                                _ => unreachable!(),
                            }
                            if fault == 254 {
                                let before = budget.work();
                                let floor = budget.storage();
                                let index = CpcFinalCallIndexV1::new(
                                    source.inventory,
                                    &output,
                                    &lineage,
                                    budget,
                                )?;
                                let expected = 1
                                    + source.inventory.functions().len()
                                    + 5 * output.functions().len()
                                    + output.operations().len()
                                    + 7 * output.calls().len();
                                assert_eq!(budget.work() - before, expected);
                                let bytes = size_of::<CpcFinalCallIndexV1>()
                                    + size_of::<Option<usize>>()
                                        * (index.functions.capacity() + index.calls.capacity());
                                assert_eq!(budget.storage(), floor + bytes);
                                for (ordinal, original) in lineage.functions.iter().enumerate() {
                                    assert_eq!(index.functions[original.0 as usize], Some(ordinal));
                                }
                                for (ordinal, call) in output.calls().iter().enumerate() {
                                    let operation =
                                        cs_operation_v1(&output, call.coordinate, budget)?;
                                    assert_eq!(index.calls[operation], Some(ordinal));
                                }
                                drop(index);
                                budget.release_storage(bytes)?;
                                assert_eq!(budget.storage(), floor);
                                return Ok(());
                            }
                            return CpcFinalCallIndexV1::new(
                                source.inventory,
                                &output,
                                &lineage,
                                budget,
                            )
                            .map(drop);
                        }
                        match fault {
                            0 => {
                                origins.index.associations.pop().unwrap();
                            }
                            1 => origins.index.associations[1] = origins.index.associations[0],
                            2 => origins.index.associations[0].0 = u64::MAX,
                            3 => origins.index.associations[0].1 = usize::MAX,
                            4 => {
                                let coordinate = origins.operations[0].coordinate;
                                let ordinal =
                                    cs_operation_v1(source.inventory, coordinate, budget)?;
                                origins.index.operations[ordinal] = None;
                            }
                            5 => {
                                let coordinate = origins.operations[1].coordinate;
                                let ordinal =
                                    cs_operation_v1(source.inventory, coordinate, budget)?;
                                origins.index.operations[ordinal] = Some(0);
                            }
                            6 => {
                                origins.index.aliases.pop().unwrap();
                            }
                            7 => origins.index.aliases[1] = origins.index.aliases[0],
                            8 => origins.index.aliases[0].1 = usize::MAX,
                            9 => origins.index.ranges[0].end = usize::MAX,
                            10 => {
                                return donor
                                    .expect("paid byte-identical donor")
                                    .original
                                    .with_checked_canonical_ranked_source_v1(
                                        budget,
                                        |other, budget| {
                                            Ok(cpc_with_source_v1(
                                                other,
                                                budget,
                                                |foreign, _, _, budget| {
                                                    let caller = foreign.calls.calls[0].site.caller;
                                                    assert_eq!(
                                                        cpc_key_v1(caller),
                                                        cpc_key_v1(
                                                            source.calls.calls[0].site.caller
                                                        )
                                                    );
                                                    assert!(!std::ptr::eq(
                                                        caller,
                                                        source.calls.calls[0].site.caller
                                                    ));
                                                    origins
                                                        .index
                                                        .caller(source, caller, budget)
                                                        .map(|_| ())
                                                },
                                            ))
                                        },
                                    )?;
                            }
                            11 => origins.index.aliases[0].2 = usize::MAX,
                            12 => {
                                origins.index.operations.pop().unwrap();
                            }
                            255 => {}
                            _ => panic!("unknown component fault"),
                        }
                        let before = budget.work();
                        origins.check(source, budget)?;
                        // Literal source recurrence, not a successful measured baseline.
                        // check: 6+3G+3O+4A+P+4C + sum(13+2*search_depth(G)).
                        let g = source.calls.groups.len();
                        let o = source.inventory.operations().len();
                        let a = origins.aliases.len();
                        let p = origins.operations.len();
                        let c = origins.calls.len();
                        let depth = usize::BITS as usize - g.leading_zeros() as usize;
                        let upper = 6 + 3 * g + 3 * o + 4 * a + p + 4 * c + c * (13 + 2 * depth);
                        assert!(budget.work() - before <= upper);
                        Ok(())
                    },
                ))
            })?
    })
}

#[test]
fn three_key_heapsort_has_literal_exact_work_and_every_one_unit_cut() {
    // [3,1,2]: build sift=5; extract end2=4; extract end1=2.
    for allowed in 0..=11 {
        let mut rows = [3usize, 1, 2];
        let floor = INDEX_FLOOR + size_of_val(&rows);
        let mut work = Work::new(allowed);
        {
            let mut budget = ArgumentBudgetV1::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            let result = cpc_sort_v1(&mut rows, |n| *n, &mut budget);
            assert_eq!(result.is_ok(), allowed == 11);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (allowed, floor, floor)
            );
            assert_eq!(budget.failed_storage(), None);
            if allowed == 11 {
                assert_eq!(rows, [1, 2, 3]);
            }
        }
        assert_eq!(work.failed_work(), (allowed < 11).then_some(allowed + 1));
    }
}

#[test]
fn binary_lookup_exact_hits_misses_and_each_denied_compare_keep_paid_backing() {
    let rows = [1usize, 2, 3];
    for (key, expected, cost) in [
        (0, None, 4),
        (1, Some(0), 4),
        (2, Some(1), 2),
        (3, Some(2), 4),
        (4, None, 4),
    ] {
        for allowed in 0..=cost {
            let floor = INDEX_FLOOR + size_of_val(&rows);
            let mut work = Work::new(allowed);
            {
                let mut budget = ArgumentBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = cpc_find_v1(&rows, key, |n| *n, &mut budget);
                if allowed == cost {
                    assert_eq!(result.unwrap(), expected);
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionCanonicalScalarSourceErrorV1::Resource(_))
                    ));
                }
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (allowed, floor, floor)
                );
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!(work.failed_work(), (allowed < cost).then_some(allowed + 1));
        }
    }
}

#[test]
fn all_lookup_keys_scale_by_logarithmic_depth_without_cartesian_scratch() {
    for exponent in 1..=10 {
        let count = (1usize << exponent) - 1;
        let mut rows = Vec::<usize>::new();
        rows.try_reserve_exact(count).unwrap();
        rows.extend(0..count);
        let floor = INDEX_FLOOR + size_of_val(&rows) + rows.capacity() * size_of::<usize>();
        for key in 0..=count {
            let mut work = Work::new(2 * exponent);
            let mut budget = ArgumentBudgetV1::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            assert_eq!(
                cpc_find_v1(&rows, key, |n| *n, &mut budget).unwrap(),
                (key < count).then_some(key)
            );
            assert!(budget.work() <= 2 * exponent);
            assert_eq!(
                (
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (floor, floor, None)
            );
        }
    }
}

#[test]
fn heapsort_scaling_uses_paid_rosters_and_source_derived_logarithmic_bounds() {
    for count in [0usize, 1, 3, 7, 31, 127] {
        for order in 0..3 {
            let mut rows = Vec::<usize>::new();
            rows.try_reserve_exact(count).unwrap();
            rows.extend(0..count);
            if order == 1 {
                rows.reverse();
            }
            if order == 2 && count > 0 {
                rows.rotate_left(count / 2);
            }
            let floor = INDEX_FLOOR + size_of_val(&rows) + rows.capacity() * size_of::<usize>();
            // Each sift has at most log2(n) descents at six debits each,
            // then one terminal debit. Extraction swaps each cost one.
            let bound = if count < 2 {
                0
            } else {
                let height = usize::BITS as usize - 1 - count.leading_zeros() as usize;
                (count / 2 + count - 1) * (6 * height + 1) + count - 1
            };
            let mut work = Work::new(bound);
            let mut budget = ArgumentBudgetV1::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            cpc_sort_v1(&mut rows, |n| *n, &mut budget).unwrap();
            assert!(
                rows.iter()
                    .enumerate()
                    .all(|(ordinal, key)| ordinal == *key)
            );
            assert!(budget.work() <= bound);
            assert_eq!(
                (
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (floor, floor, None)
            );
        }
    }
}

fn alias_rows() -> [ProductionCanonicalPrivateCallSiteV1; 3] {
    [(2, 3), (0, 1), (2, 0)].map(|(operation, caller)| ProductionCanonicalPrivateCallSiteV1 {
        operation,
        caller,
        root: SemanticFunctionIdV1::from_index(caller as u32),
        source_call: operation,
        callee: 4,
    })
}
fn empty_alias_index(budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<CpcCallIndexV1> {
    budget.reserve_storage(size_of::<CpcCallIndexV1>())?;
    let aliases = cs_vec_v1(3, budget)?;
    let mut ranges = cs_vec_v1(3, budget)?;
    budget.charge_work(3)?;
    ranges.resize(3, 0..0);
    Ok(CpcCallIndexV1 {
        associations: Vec::new(),
        operations: Vec::new(),
        aliases,
        ranges,
    })
}
fn alias_index_premises() {
    assert_eq!(size_of::<usize>(), 8, "review guarded 64-bit ABI premises");
    assert_eq!(size_of::<CpcCallIndexV1>(), 96);
    assert_eq!(size_of::<(usize, usize, usize)>(), 24);
    assert_eq!(size_of::<std::ops::Range<usize>>(), 16);
    let mut aliases = Vec::<(usize, usize, usize)>::new();
    aliases.try_reserve_exact(3).unwrap();
    let mut ranges = Vec::<std::ops::Range<usize>>::new();
    ranges.try_reserve_exact(3).unwrap();
    assert_eq!(
        (aliases.capacity(), ranges.capacity()),
        (3, 3),
        "allocator capacity premise, not a successful production cost sample"
    );
}

#[test]
fn call_alias_fanout_exact_work_and_all_masked_prefixes_are_source_derived() {
    alias_index_premises();
    let rows = alias_rows();
    let floor = INDEX_FLOOR + size_of_val(&rows);
    let paid = floor + 96 + 72 + 48;
    for allowed in 0..=38 {
        let mut work = Work::new(3 + allowed);
        let accepted = if (23..32).contains(&allowed) {
            23 + 3 * ((allowed - 23) / 3)
        } else {
            allowed
        };
        let denied = if accepted < 23 || accepted >= 32 {
            accepted + 1
        } else {
            accepted + 3
        };
        {
            let mut budget = ArgumentBudgetV1::new(&mut work, paid);
            budget.reserve_storage(floor).unwrap();
            let mut index = empty_alias_index(&mut budget).unwrap();
            // finish: pushes3 + heapsort11 + fill8 + check(1+9+3+3)=38.
            let result = index.finish(&rows, &mut budget);
            assert_eq!(result.is_ok(), allowed == 38);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (3 + accepted, paid, paid)
            );
            assert_eq!(budget.failed_storage(), None);
            if allowed == 38 {
                assert_eq!(index.aliases, [(0, 1, 1), (2, 0, 2), (2, 3, 0)]);
                assert_eq!(index.ranges, [0..1, 1..1, 1..3]);
            }
            drop(index);
            budget.release_storage(paid - floor).unwrap();
            assert_eq!(budget.storage(), floor);
        }
        assert_eq!(work.failed_work(), (allowed < 38).then_some(3 + denied));
    }
}

#[test]
fn call_alias_index_header_capacities_and_fill_have_exact_first_denials() {
    alias_index_premises();
    let floor = INDEX_FLOOR + size_of::<[ProductionCanonicalPrivateCallSiteV1; 3]>();
    for (accepted, request) in [(0, 96), (96, 72), (168, 48)] {
        let mut work = Work::new(3);
        let mut budget = ArgumentBudgetV1::new(&mut work, floor + accepted + request - 1);
        budget.reserve_storage(floor).unwrap();
        assert!(empty_alias_index(&mut budget).is_err());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (0, floor + accepted, floor + accepted)
        );
        assert_eq!(budget.failed_storage(), Some(floor + accepted + request));
        // A failed constructor has already destroyed all partially allocated Vecs.
        budget.release_storage(accepted).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    for allowed in 0..=3 {
        let mut work = Work::new(allowed);
        {
            let mut budget = ArgumentBudgetV1::new(&mut work, floor + 216);
            budget.reserve_storage(floor).unwrap();
            let index = empty_alias_index(&mut budget);
            assert_eq!(index.is_ok(), allowed == 3);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (if allowed == 3 { 3 } else { 0 }, floor + 216, floor + 216)
            );
            drop(index);
            budget.release_storage(216).unwrap();
            assert_eq!(budget.storage(), floor);
        }
        assert_eq!(work.failed_work(), (allowed < 3).then_some(3));
    }
}

#[test]
fn call_alias_range_validation_refuses_missing_duplicate_and_foreign_rows() {
    let rows = alias_rows();
    for fault in 0..7 {
        let floor = INDEX_FLOOR + size_of_val(&rows);
        let mut work = Work::new(1 << 20);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1 << 20);
        budget.reserve_storage(floor).unwrap();
        let mut index = empty_alias_index(&mut budget).unwrap();
        index.finish(&rows, &mut budget).unwrap();
        match fault {
            0 => {
                index.aliases.pop().unwrap();
            }
            1 => index.aliases[1] = index.aliases[0],
            2 => index.aliases[1].1 = 99,
            3 => index.aliases[1].2 = usize::MAX,
            4 => index.ranges[0] = 0..0,
            5 => index.ranges[2] = 1..2,
            6 => index.ranges[1] = 1..3,
            _ => unreachable!(),
        }
        assert!(matches!(
            index.check_aliases(&rows, 3, &mut budget),
            Err(ProductionCanonicalScalarSourceErrorV1::Invalid(_))
        ));
        let retained = size_of::<CpcCallIndexV1>()
            + index.aliases.capacity() * size_of::<(usize, usize, usize)>()
            + index.ranges.capacity() * size_of::<std::ops::Range<usize>>();
        assert_eq!(budget.storage(), floor + retained);
        drop(index);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}
