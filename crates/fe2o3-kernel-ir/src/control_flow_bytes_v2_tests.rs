mod byte_accounting_v2 {
    use super::*;
    use std::mem::size_of;
    type Error = CanonicalKernelIrVerificationResourceErrorV1;
    const FLOOR: usize = 7;

    fn bytes<T>(count: usize) -> usize {
        size_of::<Vec<T>>() + count * size_of::<T>()
    }

    fn frame() -> usize {
        type Owner = ByteAccountedIndexedControlFlowV2<'static, 'static>;
        size_of::<Owner>()
            + size_of::<std::thread::Result<Result<IndexedControlFlow, MeteredControlFlowErrorV1>>>(
            )
            + size_of::<Result<Owner, MeteredControlFlowErrorV1>>()
            + size_of::<ControlFlowResourcesV1<'static, 'static>>()
            + size_of::<(
                &Function,
                ControlFlowLimits,
                &mut CanonicalKernelIrVerificationResourceBudgetV1<'static>,
            )>()
    }

    fn radix_frame<T>() -> usize {
        size_of::<std::thread::Result<Result<(), Error>>>()
            + size_of::<(
                &mut [T],
                &mut CanonicalKernelIrVerificationResourceBudgetV1<'static>,
            )>()
    }

    // Independently list simultaneous owners in each chain phase. Nested row
    // Vec headers are conservatively paid in both the outer backing and each
    // independently allocated inner row; no graph-derived candidate count is used.
    fn chain_storage(blocks: usize) -> (usize, usize) {
        let edges = blocks - 1;
        let rows = bytes::<Vec<usize>>(blocks)
            + blocks * size_of::<Vec<usize>>()
            + edges * size_of::<usize>();
        let prefix = bytes::<BlockId>(blocks) + bytes::<(BlockId, usize)>(blocks);
        let base = prefix
            + bytes::<IndexedControlFlowEdge>(edges)
            + bytes::<std::ops::Range<usize>>(blocks)
            + 3 * rows;
        let reach = bytes::<bool>(blocks);
        let indices = bytes::<usize>(blocks);
        let dominators = bytes::<Option<usize>>(blocks);
        let intervals = 2 * bytes::<u32>(blocks);
        let empty = bytes::<BlockId>(0);
        let radix = radix_frame::<(BlockId, usize)>()
            + if blocks < 2 {
                0
            } else {
                bytes::<(BlockId, usize)>(blocks) + size_of::<[usize; 256]>()
            };
        let peaks = [
            prefix + radix,
            base + indices,
            base + reach + indices,
            base + 2 * reach + indices + bytes::<(usize, usize)>(blocks),
            base + reach + 2 * indices + dominators,
            base + reach + dominators + indices + rows,
            base + reach + dominators + rows + intervals + bytes::<(usize, bool)>(blocks),
            base + reach + intervals + 2 * indices + rows,
            base + reach + intervals + 2 * indices + rows + reach + empty,
        ];
        let retained = size_of::<ByteAccountedIndexedControlFlowV2<'static, 'static>>()
            + base
            + reach
            + intervals
            + empty;
        (retained, frame() + peaks.into_iter().max().unwrap())
    }

    #[test]
    fn cfg_bytes_chain_has_independent_exact_work_peak_and_retained_capacities() {
        for blocks in [1, 2, 4, 17, 64, 1_024] {
            let function = metered_cfg_chain_v1(blocks);
            // One constructor entry and two nested scratch-capacity scans.
            let expected_work = metered_cfg_chain_work_v1(blocks) + 1 + 2 * blocks;
            let (retained, peak) = chain_storage(blocks);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + peak);
            budget.reserve_storage(FLOOR).unwrap();
            let owner = analyze_control_flow_with_byte_budget_v2(
                &function,
                ControlFlowLimits::DEFAULT,
                &mut budget,
            )
            .unwrap();
            assert_eq!(
                owner.indexed_v2(&function, &budget).unwrap(),
                &analyze_control_flow(&function).unwrap()
            );
            assert_eq!(budget.work(), expected_work, "blocks={blocks}");
            assert_eq!(budget.storage(), FLOOR + retained, "blocks={blocks}");
            assert_eq!(budget.peak_storage(), FLOOR + peak, "blocks={blocks}");
            owner.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn cfg_bytes_chain_one_short_work_and_storage_drop_scratch_before_refund() {
        for blocks in [1, 2, 4, 17, 64] {
            let function = metered_cfg_chain_v1(blocks);
            let total = metered_cfg_chain_work_v1(blocks) + 1 + 2 * blocks;
            let (_, peak) = chain_storage(blocks);
            for work_short in [false, true] {
                let work_limit = total - usize::from(work_short);
                let storage_limit = FLOOR + peak - usize::from(!work_short);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
                budget.reserve_storage(FLOOR).unwrap();
                let error = analyze_control_flow_with_byte_budget_v2(
                    &function,
                    ControlFlowLimits::DEFAULT,
                    &mut budget,
                )
                .err()
                .unwrap();
                if work_short {
                    assert!(
                        matches!(error, MeteredControlFlowErrorV1::Resource(Error::Work(error)) if error.actual() == total && error.limit() == work_limit)
                    );
                    assert_eq!(budget.failed_work(), Some(total));
                } else {
                    assert!(
                        matches!(error, MeteredControlFlowErrorV1::Resource(Error::Storage(error)) if error.actual() == FLOOR + peak && error.limit() == storage_limit)
                    );
                    assert_eq!(budget.failed_storage(), Some(FLOOR + peak));
                }
                assert_eq!(budget.storage(), FLOOR);
                let accepted_work = budget.work();
                assert_eq!(
                    analyze_control_flow_with_byte_budget_v2(
                        &function,
                        ControlFlowLimits::DEFAULT,
                        &mut budget
                    )
                    .err()
                    .unwrap(),
                    error
                );
                assert_eq!(budget.work(), accepted_work);
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }

    fn row_size_case<T: Copy + Default>() {
        let expected = bytes::<T>(3);
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                FLOOR + expected - usize::from(short),
            );
            budget.reserve_storage(FLOOR).unwrap();
            let mut resources = ControlFlowResourcesV1 {
                budget: Some(&mut budget),
                storage: ControlFlowStorageV2::TypedBytes,
            };
            let result = resources.filled(3, T::default(), usize::MAX);
            if short {
                assert!(
                    matches!(result, Err(MeteredControlFlowErrorV1::Resource(Error::Storage(error))) if error.actual() == FLOOR + expected)
                );
            } else {
                let rows = result.unwrap();
                assert_eq!(rows.len(), 3);
                assert!(rows.capacity() >= 3);
                resources.free(rows, usize::MAX, usize::MAX).unwrap();
            }
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.work(), if short { 1 } else { 4 });
        }
    }

    #[test]
    fn cfg_bytes_zero_sized_and_differently_sized_rows_ignore_legacy_cell_widths() {
        #[derive(Clone, Copy, Default)]
        #[repr(align(64))]
        struct Aligned([u8; 1]);
        row_size_case::<()>();
        row_size_case::<u8>();
        row_size_case::<u64>();
        row_size_case::<[u8; 31]>();
        row_size_case::<Aligned>();
    }

    #[test]
    fn cfg_bytes_real_excess_capacity_is_admitted_before_use_or_rejected_exactly() {
        for short in [false, true] {
            let mut rows = Vec::<u64>::with_capacity(17);
            let actual = size_of::<Vec<u64>>() + rows.capacity() * size_of::<u64>();
            let requested = bytes::<u64>(1);
            assert!(actual > requested);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                FLOOR + actual - usize::from(short),
            );
            budget.reserve_storage(FLOOR + requested).unwrap();
            let result = admit_control_flow_capacity_v2(&rows, 1, &mut budget);
            if short {
                assert!(
                    matches!(result, Err(Error::Storage(error)) if error.actual() == FLOOR + actual)
                );
                assert!(rows.is_empty());
                assert_eq!(budget.storage(), FLOOR + requested);
            } else {
                result.unwrap();
                rows.push(9);
                assert_eq!(budget.storage(), FLOOR + actual);
            }
            drop(rows);
            budget.rollback_storage(FLOOR).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn cfg_bytes_old_and_new_vectors_coexist_until_old_owner_is_dropped() {
        let old_bytes = bytes::<u64>(4);
        let new_bytes = bytes::<u64>(8);
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                FLOOR + old_bytes + new_bytes - usize::from(short),
            );
            budget.reserve_storage(FLOOR).unwrap();
            let old = allocate_control_flow_bytes_v2::<u64>(4, &mut budget).unwrap();
            let new = allocate_control_flow_bytes_v2::<u64>(8, &mut budget);
            if short {
                assert!(
                    matches!(new, Err(Error::Storage(error)) if error.actual() == FLOOR + old_bytes + new_bytes)
                );
                assert_eq!(budget.storage(), FLOOR + old_bytes);
                drop(old);
                budget.release_storage(old_bytes).unwrap();
            } else {
                let new = new.unwrap();
                assert_eq!(budget.storage(), FLOOR + old_bytes + new_bytes);
                drop(old);
                budget.release_storage(old_bytes).unwrap();
                assert_eq!(budget.storage(), FLOOR + new_bytes);
                drop(new);
                budget.release_storage(new_bytes).unwrap();
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn cfg_bytes_nested_row_capacities_and_headers_are_paid_and_retired() {
        let exact = bytes::<Vec<usize>>(3) + 3 * size_of::<Vec<usize>>() + 4 * size_of::<usize>();
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(13);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                FLOOR + exact - usize::from(short),
            );
            budget.reserve_storage(FLOOR).unwrap();
            let mut resources = ControlFlowResourcesV1 {
                budget: Some(&mut budget),
                storage: ControlFlowStorageV2::TypedBytes,
            };
            let result = resources.rows(&[0, 3, 1]);
            if short {
                assert!(
                    matches!(result, Err(MeteredControlFlowErrorV1::Resource(Error::Storage(error))) if error.actual() == FLOOR + exact)
                );
                budget.rollback_storage(FLOOR).unwrap();
            } else {
                let rows = result.unwrap();
                assert_eq!(
                    rows.iter().map(Vec::capacity).collect::<Vec<_>>(),
                    [0, 3, 1]
                );
                resources.free_rows(rows, usize::MAX).unwrap();
                assert_eq!(budget.work(), 13);
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn cfg_bytes_radix_pays_input_scratch_bucket_overlap_with_exact_boundaries() {
        use crate::verification_index_v1::verification_radix_sort_u32_bytes_v2 as sort;
        type Row = (u32, u64);
        let input_bytes = bytes::<Row>(3);
        let exact =
            input_bytes + radix_frame::<Row>() + bytes::<Row>(3) + size_of::<[usize; 256]>();
        let total_work = 3 + 4 * (3 * 3 + 2 * 256) + 3;
        for mode in 0..3 {
            let mut input: Vec<Row> = vec![(9, 0), (1, 1), (1, 2)];
            let mut work = CanonicalKernelIrWorkBudgetV1::new(total_work - usize::from(mode == 1));
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                FLOOR + exact - usize::from(mode == 2),
            );
            budget.reserve_storage(FLOOR + input_bytes).unwrap();
            let result = sort(&mut input, &mut budget, |row| row.0);
            match mode {
                0 => {
                    result.unwrap();
                    assert_eq!(input, [(1, 1), (1, 2), (9, 0)]);
                    assert_eq!(budget.peak_storage(), FLOOR + exact);
                    assert_eq!(budget.work(), total_work);
                }
                1 => assert!(
                    matches!(result, Err(Error::Work(error)) if error.actual() == total_work)
                ),
                2 => assert!(
                    matches!(result, Err(Error::Storage(error)) if error.actual() == FLOOR + exact)
                ),
                _ => unreachable!(),
            }
            assert_eq!(budget.storage(), FLOOR + input_bytes);
            drop(input);
            budget.release_storage(input_bytes).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn cfg_bytes_radix_unwind_retires_owned_scratch_without_refunding_input() {
        use crate::verification_index_v1::verification_radix_sort_u32_bytes_v2 as sort;
        use std::{
            cell::Cell,
            panic::{AssertUnwindSafe, catch_unwind},
        };
        let mut input = vec![(2_u32, 0_usize), (1, 1)];
        let input_bytes = bytes::<(u32, usize)>(2);
        let calls = Cell::new(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 10_000);
        budget.reserve_storage(FLOOR + input_bytes).unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            sort(&mut input, &mut budget, |row| {
                calls.set(calls.get() + 1);
                assert!(calls.get() < 3, "panic after scratch allocation");
                row.0
            })
        }));
        assert!(outcome.is_err());
        assert_eq!(calls.get(), 3);
        assert_eq!(budget.storage(), FLOOR + input_bytes);
        assert!(budget.peak_storage() > FLOOR + input_bytes + size_of::<[usize; 256]>());
        drop(outcome);
        drop(input);
        budget.release_storage(input_bytes).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }

    #[test]
    fn cfg_bytes_identity_source_and_floor_checks_do_not_bless_substitutions() {
        let function = metered_cfg_chain_v1(2);
        let other = metered_cfg_chain_v1(2);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = analyze_control_flow_with_byte_budget_v2(
            &function,
            ControlFlowLimits::DEFAULT,
            &mut budget,
        )
        .unwrap();
        let retained = budget.storage();
        assert_eq!(owner.indexed_v2(&other, &budget), Err(Error::Accounting));
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut foreign =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut foreign_work, 100_000);
        foreign.reserve_storage(retained).unwrap();
        foreign.charge_work(1).unwrap_err();
        assert_eq!(
            owner.indexed_v2(&function, &foreign),
            Err(Error::Accounting)
        );
        budget.release_storage(1).unwrap();
        assert_eq!(owner.indexed_v2(&function, &budget), Err(Error::Accounting));
        let before = budget.storage();
        assert_eq!(owner.release(&mut budget), Err(Error::Accounting));
        assert_eq!(budget.storage(), before);
        assert_eq!(foreign.storage(), retained);
        budget.rollback_storage(FLOOR).unwrap();
    }

    #[test]
    fn cfg_bytes_moved_original_ledger_and_same_slot_replacement_are_both_rejected() {
        let function = metered_cfg_chain_v1(1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = analyze_control_flow_with_byte_budget_v2(
            &function,
            ControlFlowLimits::DEFAULT,
            &mut budget,
        )
        .unwrap();
        let retained = budget.storage();
        let mut other_work = CanonicalKernelIrWorkBudgetV1::new(1_000);
        let replacement =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut other_work, 100_000);
        let mut moved = std::mem::replace(&mut budget, replacement);
        budget.reserve_storage(retained).unwrap();
        assert_eq!(owner.indexed_v2(&function, &moved), Err(Error::Accounting));
        assert_eq!(owner.indexed_v2(&function, &budget), Err(Error::Accounting));
        assert_eq!(owner.release(&mut budget), Err(Error::Accounting));
        assert_eq!(budget.storage(), retained);
        assert_eq!(moved.storage(), retained);
        moved.rollback_storage(FLOOR).unwrap();
    }

    #[test]
    fn cfg_bytes_live_query_rejects_prior_denial_but_cleanup_retires_only_owned_bytes() {
        let function = metered_cfg_chain_v1(1);
        for storage_denial in [false, true] {
            for undercut in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(106);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
                budget.reserve_storage(FLOOR).unwrap();
                let owner = analyze_control_flow_with_byte_budget_v2(
                    &function,
                    ControlFlowLimits::DEFAULT,
                    &mut budget,
                )
                .unwrap();
                budget.reserve_storage(19).unwrap();
                let expected = if storage_denial {
                    budget.reserve_storage(100_000).unwrap_err()
                } else {
                    budget.charge_work(1).unwrap_err()
                };
                assert_eq!(owner.indexed_v2(&function, &budget), Err(expected));
                if undercut {
                    budget.release_storage(20).unwrap();
                    let damaged = budget.storage();
                    assert_eq!(owner.indexed_v2(&function, &budget), Err(expected));
                    assert_eq!(owner.release(&mut budget), Err(Error::Accounting));
                    assert_eq!(budget.storage(), damaged);
                    budget.rollback_storage(FLOOR).unwrap();
                } else {
                    owner.release(&mut budget).unwrap();
                    assert_eq!(budget.storage(), FLOOR + 19);
                }
                assert_eq!(control_flow_prior_denial_v2(&budget), Err(expected));
            }
        }
    }

    #[test]
    fn cfg_bytes_existing_semantic_errors_and_limits_leave_exact_input_floor() {
        let mut missing = metered_cfg_chain_v1(1);
        missing.body.as_mut().unwrap().blocks[0].terminator = None;
        let mut duplicate = metered_cfg_chain_v1(2);
        duplicate.body.as_mut().unwrap().blocks[1].id = BlockId(1);
        let mut unknown = metered_cfg_chain_v1(2);
        unknown.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(99),
            arguments: vec![],
        });
        for function in [metered_cfg_chain_v1(0), missing, duplicate, unknown] {
            let expected = analyze_control_flow(&function).unwrap_err();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
            budget.reserve_storage(FLOOR).unwrap();
            assert_eq!(
                analyze_control_flow_with_byte_budget_v2(
                    &function,
                    ControlFlowLimits::DEFAULT,
                    &mut budget
                )
                .err()
                .unwrap(),
                MeteredControlFlowErrorV1::ControlFlow(expected)
            );
            assert_eq!(budget.storage(), FLOOR);
        }
        let function = metered_cfg_chain_v1(8);
        for limits in [
            ControlFlowLimits {
                blocks: 7,
                ..ControlFlowLimits::DEFAULT
            },
            ControlFlowLimits {
                edges: 6,
                ..ControlFlowLimits::DEFAULT
            },
            ControlFlowLimits {
                analysis_work: 1,
                ..ControlFlowLimits::DEFAULT
            },
        ] {
            let expected = analyze_control_flow_with_limits(&function, limits).unwrap_err();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
            budget.reserve_storage(FLOOR).unwrap();
            assert_eq!(
                analyze_control_flow_with_byte_budget_v2(&function, limits, &mut budget)
                    .err()
                    .unwrap(),
                MeteredControlFlowErrorV1::ControlFlow(expected)
            );
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn cfg_bytes_general_loops_unreachable_blocks_and_duplicate_edges_match_legacy() {
        for edges in [
            vec![vec![1, 2], vec![3], vec![3], vec![]],
            vec![vec![1], vec![2, 3], vec![1], vec![], vec![4]],
            vec![vec![1, 2], vec![2, 3], vec![1, 3], vec![]],
            vec![vec![1, 1, 1], vec![]],
        ] {
            let blocks = edges
                .into_iter()
                .enumerate()
                .map(|(ordinal, targets)| {
                    let mut block = BasicBlock::new(BlockId(ordinal as u32));
                    block.terminator = Some(if let Some((&first, rest)) = targets.split_first() {
                        Terminator::Switch {
                            selector: ValueId(0),
                            cases: rest
                                .iter()
                                .enumerate()
                                .map(|(index, target)| SwitchCase {
                                    value: index as u64,
                                    target: BlockId(*target),
                                    arguments: vec![],
                                })
                                .collect(),
                            default_target: BlockId(first),
                            default_arguments: vec![],
                        }
                    } else {
                        Terminator::Return { values: vec![] }
                    });
                    block
                })
                .collect();
            let function = metered_cfg_function_v1(blocks);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
            budget.reserve_storage(FLOOR).unwrap();
            let owner = analyze_control_flow_with_byte_budget_v2(
                &function,
                ControlFlowLimits::DEFAULT,
                &mut budget,
            )
            .unwrap();
            assert_eq!(owner.flow, analyze_control_flow(&function).unwrap());
            owner.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
