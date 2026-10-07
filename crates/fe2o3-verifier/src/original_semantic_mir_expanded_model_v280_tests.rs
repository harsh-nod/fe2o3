use super::super::source_function::tile_fixture_tests::run_fixture_with_plan;
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, ExecutionTileLayoutV1};

const LIMIT: usize = 512 * 1024 * 1024;

fn run(
    work: usize,
    storage: usize,
    consume: impl for<'m, 's, 'w> FnOnce(
        &ExpandedSupportModelV280<'m, 's>,
        &mut Budget<'w>,
    ) -> Result<((), usize)>,
) -> (Result<()>, usize, usize, usize) {
    run_width(work, storage, FormalIndexWidth::Bits64, consume)
}

fn run_width(
    work: usize,
    storage: usize,
    width: FormalIndexWidth,
    consume: impl for<'m, 's, 'w> FnOnce(
        &ExpandedSupportModelV280<'m, 's>,
        &mut Budget<'w>,
    ) -> Result<((), usize)>,
) -> (Result<()>, usize, usize, usize) {
    run_width_with_panic_observer(work, storage, width, consume, |_| {})
}

fn run_width_with_panic_observer(
    work: usize,
    storage: usize,
    width: FormalIndexWidth,
    consume: impl for<'m, 's, 'w> FnOnce(
        &ExpandedSupportModelV280<'m, 's>,
        &mut Budget<'w>,
    ) -> Result<((), usize)>,
    observe_panic: impl FnOnce(&(dyn std::any::Any + Send)),
) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_plan(
        ExecutionTileLayoutV1::Blocked,
        work,
        storage,
        |_, slots, tile, out| {
            let original = slots.correspondence(out)?;
            let source = original.source(out.budget)?;
            let launch = source.source_launch(out.budget)?.roots()[0];
            let runtime = [ExpandedSupportRuntimeV280 {
                source_root: launch.selected_root(),
                source_launch: launch.source_launch(),
                source_layout: launch.layout(),
                physical: ExplicitLaunchExtent::Exact {
                    rank: launch.source_rank(),
                    extents: std::array::from_fn(|axis| {
                        u64::from(launch.source_launch().exact_workgroup().unwrap()[axis])
                            * u64::from(launch.source_launch().max_grid()[axis])
                    }),
                },
            }];
            let floor = out.budget.storage();
            let slot = std::ptr::from_ref(&*out.budget) as usize;
            let ledger = out.budget.work_ledger_identity_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_expanded_support_model_v280(
                    source,
                    original,
                    tile,
                    &runtime,
                    width,
                    EndiannessV2::Little,
                    out.budget,
                    consume,
                )
            }));
            assert_eq!(slot, std::ptr::from_ref(&*out.budget) as usize);
            assert!(ledger == out.budget.work_ledger_identity_v1());
            assert_eq!(out.budget.storage(), floor);
            let result = match result {
                Ok(result) => result,
                Err(payload) => {
                    observe_panic(payload.as_ref());
                    std::panic::resume_unwind(payload)
                }
            };
            let ((), bytes) = result??;
            assert_eq!(bytes, 0);
            Ok(())
        },
    )
}

fn inspect(
    model: &ExpandedSupportModelV280<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<((), usize)> {
    let text = model.generated_source(budget)?;
    assert!(!text.is_empty() && text.len() <= SOURCE_LIMIT);
    let text = std::str::from_utf8(text).unwrap();
    assert!(text.contains("spec fn invocation_tile_cursor_0_v180"));
    assert!(!text.contains("proof fn invocation_paired_"));
    let rows = model.census(budget)?;
    assert_eq!(rows.roots.len(), 1);
    assert!(!rows.instances.is_empty() && !rows.cuts.is_empty());
    assert_eq!(rows.roots[0].instances, 0..rows.instances.len());
    assert_eq!(rows.roots[0].cuts, 0..rows.cuts.len());
    let (mut calls, mut cuts) = (0, 0);
    for (ordinal, instance) in rows.instances.iter().enumerate() {
        budget.charge_work(1)?;
        assert_eq!((instance.root, instance.instance), (0, ordinal));
        assert_eq!(instance.calls.start, calls);
        assert_eq!(instance.cuts.start, cuts);
        calls = instance.calls.end;
        cuts = instance.cuts.end;
        if let Some((parent, block)) = instance.incoming {
            assert!(parent < ordinal);
            let mut found = false;
            for call in &rows.calls[rows.instances[parent].calls.clone()] {
                budget.charge_work(1)?;
                found |=
                    call.caller == parent && call.block == block && call.child == Some(ordinal);
            }
            assert!(found);
        } else {
            assert_eq!(ordinal, 0);
        }
        for (block, pc) in instance.cuts.clone().enumerate() {
            budget.charge_work(1)?;
            let cut = &rows.cuts[pc];
            assert_eq!(
                (cut.root, cut.instance, cut.block),
                (0, ordinal, block as u32)
            );
            if !instance.active {
                assert!(cut.candidates.is_empty());
            }
        }
    }
    assert_eq!((calls, cuts), (rows.calls.len(), rows.cuts.len()));
    let (mut candidates, mut edges) = (0, 0);
    for (pc, cut) in rows.cuts.iter().enumerate() {
        budget.charge_work(1)?;
        assert_eq!(cut.candidates.start, candidates);
        assert_eq!(cut.zero_edges.start, edges);
        candidates = cut.candidates.end;
        edges = cut.zero_edges.end;
        for &(from, to) in &rows.zero_edges[cut.zero_edges.clone()] {
            budget.charge_work(1)?;
            assert_eq!(from, pc);
            assert!(cut.zero_rank > rows.cuts[to].zero_rank);
        }
    }
    assert_eq!(
        (candidates, edges),
        (rows.candidates.len(), rows.zero_edges.len())
    );
    Ok(((), 0))
}

#[test]
fn expanded_support_borrows_complete_static_rows_and_no_paired_proof() {
    run(LIMIT, LIMIT, inspect).0.unwrap();
}

#[test]
fn expanded_support_complete_generation_has_exact_and_one_short_resources() {
    let baseline = run(LIMIT, LIMIT, inspect);
    baseline.0.unwrap();
    let exact = run(baseline.1, baseline.3, inspect);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (baseline.1, baseline.2, baseline.3)
    );
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    assert!(matches!(run(baseline.1 - 1, baseline.3, inspect).0,
        Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
    assert!(matches!(run(baseline.1, baseline.3 - 1, inspect).0,
        Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
}

#[test]
fn expanded_support_transfers_complete_payload_after_scratch_refund() {
    run_fixture_with_plan(
        ExecutionTileLayoutV1::Blocked,
        LIMIT,
        LIMIT,
        |_, slots, tile, out| {
            let original = slots.correspondence(out)?;
            let source = original.source(out.budget)?;
            let launch = source.source_launch(out.budget)?.roots()[0];
            let runtime = [ExpandedSupportRuntimeV280 {
                source_root: launch.selected_root(),
                source_launch: launch.source_launch(),
                source_layout: launch.layout(),
                physical: ExplicitLaunchExtent::Exact {
                    rank: launch.source_rank(),
                    extents: std::array::from_fn(|axis| {
                        u64::from(launch.source_launch().exact_workgroup().unwrap()[axis])
                            * u64::from(launch.source_launch().max_grid()[axis])
                    }),
                },
            }];
            let floor = out.budget.storage();
            let (payload, retained) = with_expanded_support_model_v280(
                source,
                original,
                tile,
                &runtime,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out.budget,
                |_, budget| {
                    budget.reserve_storage(17)?;
                    Ok::<_, Error>((Box::new([7u8; 17]), 17))
                },
            )??;
            assert_eq!(*payload, [7; 17]);
            assert_eq!(retained, 17 + size_of::<Box<[u8; 17]>>());
            assert_eq!(out.budget.storage(), floor + retained);
            drop(payload);
            out.budget.release_storage(retained)?;
            assert_eq!(out.budget.storage(), floor);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn expanded_support_borrowed_queries_refuse_foreign_account() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    let mut checked = false;
    let result = run(LIMIT, LIMIT, |model, budget| {
        let mut work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut work, LIMIT);
        foreign.reserve_storage(budget.storage())?;
        let before = budget.work();
        assert!(matches!(
            model.generated_source(&mut foreign),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(budget.work(), before);
        assert!(matches!(
            model.census(budget),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        checked = true;
        Err(Error::Statement("foreign account refused"))
    });
    assert!(checked);
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn expanded_support_preserves_consumer_error_payload_and_refuses_misreported_credit() {
    assert!(matches!(
        run(LIMIT, LIMIT, |_, _| Err(Error::Statement(
            "original model consumer error"
        )))
        .0,
        Err(Error::Statement("original model consumer error"))
    ));
    assert!(run(LIMIT, LIMIT, |_, _| Ok(((), 1))).0.is_err());
    assert!(
        run(LIMIT, LIMIT, |_, budget| {
            budget.reserve_storage(1)?;
            Ok(((), 0))
        })
        .0
        .is_err()
    );
    assert!(
        run(LIMIT, LIMIT, |model, budget| {
            budget.release_storage(1)?;
            assert!(model.census(budget).is_err());
            Err(Error::Statement("model floor reduced"))
        })
        .0
        .is_err()
    );
}

#[test]
fn expanded_support_runtime_rejects_mutated_physical_and_source_rows() {
    run_fixture_with_plan(
        ExecutionTileLayoutV1::Blocked,
        LIMIT,
        LIMIT,
        |_, slots, _, out| {
            let source = slots.correspondence(out)?.source(out.budget)?;
            let retained = source.source_launch(out.budget)?.roots()[0];
            let extents = std::array::from_fn(|axis| {
                u64::from(retained.source_launch().exact_workgroup().unwrap()[axis])
                    * u64::from(retained.source_launch().max_grid()[axis])
            });
            let mut row = ExpandedSupportRuntimeV280 {
                source_root: retained.selected_root(),
                source_launch: retained.source_launch(),
                source_layout: retained.layout(),
                physical: ExplicitLaunchExtent::Exact {
                    rank: retained.source_rank(),
                    extents,
                },
            };
            row.checked_launch(&retained, out.budget)?;
            row.physical = ExplicitLaunchExtent::Exact {
                rank: retained.source_rank(),
                extents: [extents[0] + 1, extents[1], extents[2]],
            };
            assert!(row.checked_launch(&retained, out.budget).is_err());
            row.physical = ExplicitLaunchExtent::Exact {
                rank: retained.source_rank(),
                extents,
            };
            row.source_root =
                SemanticFunctionIdV1::from_index(retained.selected_root().index() + 1);
            assert!(row.checked_launch(&retained, out.budget).is_err());
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn expanded_support_unwind_restores_original_floor_and_pending_drop_keeps_error() {
    let mut reached = false;
    let mut original_payload = false;
    let result = catch_unwind(AssertUnwindSafe(|| {
        run_width_with_panic_observer(
            LIMIT,
            LIMIT,
            FormalIndexWidth::Bits64,
            |_, _| {
                reached = true;
                std::panic::panic_any(280u32)
            },
            |payload| {
                assert_eq!(payload.downcast_ref::<u32>(), Some(&280));
                original_payload = true;
            },
        )
    }));
    assert!(reached && original_payload);
    // The existing neutral-optimizer fixture converts the rethrown payload only
    // after the bridge's original payload and account restoration were checked.
    assert_eq!(
        result
            .err()
            .unwrap()
            .downcast_ref::<String>()
            .map(String::as_str),
        Some("original tile fixture preparation: Adoption(Panicked)")
    );
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("callback destructor");
        }
    }
    let bomb = Bomb;
    let result = run_width(LIMIT, LIMIT, FormalIndexWidth::Unknown, move |_, _| {
        drop(bomb);
        Ok(((), 0))
    });
    assert!(result.0.is_err());
}

#[test]
fn expanded_support_runtime_domain_keeps_exact_32_bit_ceiling() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 0);
    let check = super::super::expanded_generation::check_runtime_domain_v280;
    check(
        FormalIndexWidth::Bits32,
        1,
        [u64::from(u32::MAX), 1, 1],
        &mut budget,
    )
    .unwrap();
    for (rank, extents) in [
        (1, [u64::from(u32::MAX) + 1, 1, 1]),
        (1, [64 * u64::from(u32::MAX), 1, 1]),
        (1, [0, 1, 1]),
        (1, [64, 2, 1]),
    ] {
        assert!(check(FormalIndexWidth::Bits32, rank, extents, &mut budget).is_err());
    }
}
