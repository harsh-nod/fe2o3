// This test-only layout oracle deliberately does not call the implementation's
// header/vector helpers or read a measured successful run as its expected value.
#[allow(dead_code)]
enum OptimizedAttachmentOracleV18 {
    Operation(ProductionOptimizedSourceOperationV18),
    Definition {
        input: OptimizedDefinition,
        outputs: fe2o3_kernel_ir::CanonicalKirTransitionRangeV1,
    },
    Block {
        input: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        control: fe2o3_kernel_analysis::CanonicalKirBlockControlV1,
    },
    Terminator {
        input: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        output: ProductionOptimizedSourceTerminatorV18,
    },
    Edge {
        input: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
        control: fe2o3_kernel_analysis::CanonicalKirEdgeControlV1,
    },
    Gap(ProductionOptimizedSourceGapV18),
    Use {
        input: OptimizedUse,
        output: Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>,
    },
    EdgeArgument {
        input: fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1,
        output: Option<fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1>,
    },
    OriginalRemovedCall,
    OriginalNoOutput,
}

fn optimized_correspondence_header_oracle_v18<T, E, F>(_: &F) -> usize {
    use std::mem::{align_of, size_of};
    use std::panic::AssertUnwindSafe;
    type Capture<'a, 'w, F> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'a, 'a, 'a, 'a>,
        &'a mut ArgumentBudgetV1<'w>,
        &'a std::cell::Cell<usize>,
        F,
    );
    type Entry<'a, F> = (
        fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV18<'a, 'a, 'a>,
        [Vec<usize>; 5],
        F,
    );
    type EntryResult<'a, F> = SourceOwnedResultV18<Entry<'a, F>>;
    type Invoke<'a, 'w, F> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        F,
    );
    type Payload = Box<dyn std::any::Any + Send>;
    let cleanup = size_of::<[Option<Payload>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + size_of::<AssertUnwindSafe<Payload>>()
        + 2 * size_of::<Result<(), Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    3 * size_of::<Capture<'_, '_, F>>()
        + 3 * align_of::<Capture<'_, '_, F>>()
        + 2 * size_of::<AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<Entry<'_, F>>()
        + 2 * size_of::<EntryResult<'_, F>>()
        + 2 * size_of::<std::thread::Result<EntryResult<'_, F>>>()
        + size_of::<AssertUnwindSafe<Entry<'_, F>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<std::cell::Cell<usize>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Invoke<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Invoke<'_, '_, F>>>()
        + size_of::<ProductionOptimizedSourceCorrespondenceV18<'_>>()
        + 5 * size_of::<Vec<usize>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<AssertUnwindSafe<Result<T, E>>>()
        + 3 * size_of::<usize>()
        + cleanup
}

fn optimized_source_retained_oracle_v18<F>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    checked: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>,
    consume: &F,
) -> usize {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockControlV1, CanonicalKirEdgeControlV1, CanonicalKirOutputUseV1,
        CheckedCanonicalKirControlIndexV18,
    };
    use fe2o3_kernel_ir::{
        CanonicalKirEdgeArgumentCoordinateV1, CanonicalKirOperationCoordinateV1,
    };
    let input = checked.input();
    let sites: usize = original
        .source
        .owner
        .inner
        .pending
        .roots
        .iter()
        .map(|root| root.coordinates.spans.rows.len())
        .sum();
    let headers =
        optimized_correspondence_header_oracle_v18::<(), ProductionSourceOwnedViewErrorV18, F>(
            consume,
        );
    let control = size_of::<CheckedCanonicalKirControlIndexV18<'_, '_, '_>>()
        + input.blocks().len() * size_of::<CanonicalKirBlockControlV1>()
        + input.edges().len() * size_of::<CanonicalKirEdgeControlV1>()
        + input.uses().len() * size_of::<Option<CanonicalKirOutputUseV1>>()
        + input.edge_arguments().len() * size_of::<Option<CanonicalKirEdgeArgumentCoordinateV1>>();
    headers
        + control
        + input.operations().len() * size_of::<Option<CanonicalKirOperationCoordinateV1>>()
        + input.blocks().len() * size_of::<usize>()
        + (input.operations().len() + input.blocks().len())
            * size_of::<Option<ProductionOptimizedSourceGapIntervalV18>>()
        + original.attachments.len() * size_of::<OptimizedAttachmentOracleV18>()
        + sites * 7 * size_of::<usize>()
}

#[test]
fn optimized_source_header_exact_and_one_short_storage_refuse_at_distinct_boundaries() {
    for short in [0usize, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let attempted = std::cell::Cell::new(false);
        let result = with_actual_optimized_transition_v18(
            prepared,
            &mut budget,
            |source, checked, budget| {
                source.with_ranked_correspondence_v18(checked.input(), budget, |original, budget| {
                let consume = |_: &ProductionOptimizedSourceCorrespondenceV18<'_>, _: &mut ArgumentBudgetV1<'_>| -> SourceOwnedResultV18<()> {
                    panic!("entry resource denial reached callback")
                };
                let header = optimized_correspondence_header_oracle_v18::<(), ProductionSourceOwnedViewErrorV18, _>(&consume);
                let padding = budget.storage_limit() - budget.storage() - (header - short);
                budget.reserve_storage(padding)?;
                let floor = budget.storage();
                let work = budget.work();
                let result = original.with_optimized_correspondence_v18(checked, budget, consume);
                assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(_)))));
                assert_eq!(budget.storage(), floor, "construction scratch refunded only after drop");
                if short == 1 {
                    assert_eq!(budget.failed_storage(), Some(budget.storage_limit() + 1));
                    assert_eq!(budget.work() - work, 2, "the control index has not started");
                } else {
                    assert_eq!(budget.failed_storage(), Some(budget.storage_limit()
                        + size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV18<'_, '_, '_>>()));
                    assert_eq!(budget.work() - work, 3, "the exact prepaid header precedes control construction");
                }
                attempted.set(true);
                budget.release_storage(padding)?;
                result
            })
            },
        );
        assert!(attempted.get());
        assert!(result.is_err());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn optimized_source_read_and_descendant_queries_have_independent_work_oracles() {
    run_optimized_source_v18(folding_source_owner_v18, |view, budget| {
        let before = budget.work();
        let input = view.input_inventory(budget)?;
        assert_eq!(budget.work() - before, 1);
        let before = budget.work();
        view.output_inventory(budget)?;
        assert_eq!(budget.work() - before, 1);
        let before = budget.work();
        view.original_source(budget)?;
        assert_eq!(budget.work() - before, 2);
        for definition in input.definitions() {
            let expected = match definition.coordinate {
                OptimizedDefinition::FunctionArgument { .. } => 7,
                OptimizedDefinition::BlockArgument { .. } => 9,
                OptimizedDefinition::Result { .. } => 11,
            };
            let before = budget.work();
            view.definition_descendants(definition.coordinate, budget)?;
            assert_eq!(budget.work() - before, expected);
        }
        Ok(())
    });
}

#[test]
fn optimized_source_read_exact_and_one_short_work_limits_are_distinct() {
    for remaining in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let attempted = std::cell::Cell::new(false);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - remaining)?;
            let before = budget.work();
            let result = view.output_inventory(budget);
            assert_eq!(result.is_ok(), remaining == 1);
            assert_eq!(budget.work() - before, remaining);
            attempted.set(true);
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "work boundary probe completed",
            ))
        });
        assert!(attempted.get());
        assert!(result.is_err());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn all_original_and_optimized_owners_coexist_on_the_continuing_ledger() {
    run_optimized_source_v18(scalar_payload_owner_v18, |view, budget| {
        let floor = budget.storage();
        let source = view.original_source(budget)?;
        let input = view.input_inventory(budget)?;
        let output = view.output_inventory(budget)?;
        let input_bytes = input.owner().canonical_bytes().len();
        let output_bytes = output.owner().canonical_bytes().len();
        assert!(floor > MODULE_FLOOR + input_bytes + output_bytes);
        assert!(input.belongs_to(&source.owner.inner.pending.graph));
        assert!(!std::ptr::eq(input.owner(), output.owner()));
        assert_eq!(
            budget.storage(),
            floor,
            "guarded borrowed reads acquire no owner credits"
        );
        Ok(())
    });
}

#[test]
fn optimized_source_callback_underpayment_is_sticky_and_denies_refund() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let residual = std::cell::Cell::new(0);
    let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
        budget.release_storage(1)?;
        residual.set(budget.storage());
        assert!(view.input_inventory(budget).is_err());
        Ok(())
    });
    assert!(residual.get() > MODULE_FLOOR);
    assert!(result.is_err());
    assert_eq!(budget.storage(), residual.get());
}
