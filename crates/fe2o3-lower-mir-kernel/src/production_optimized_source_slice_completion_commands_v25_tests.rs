pub(super) fn test_source_slice_completion_v25(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    launch: fe2o3_kernel_ir::ExplicitLaunchExtent,
    width: fe2o3_kernel_ir::FormalIndexWidth,
    observed: &std::cell::Cell<[usize; 3]>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    with_source_slice_completion_v25(
        original,
        optimized,
        &[launch],
        width,
        fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default(),
        budget,
        &mut |completion, budget| {
            assert_eq!(completion.launch, launch);
            assert_eq!(completion.width, width);
            assert_eq!(completion.source.root(), 0);
            completion
                .native
                .check_owner(optimized.output_inventory(budget)?.owner(), budget)
                .map_err(slice_entry_native_error_v25)?;
            let mut counts = observed.get();
            counts[0] += 1;
            for row in completion.arguments.iter().flatten() {
                counts[1] += row.reads;
                counts[2] += row.writes;
                if row.writes != 0 {
                    assert!(row.exclusive);
                    assert!(!row.different_projection);
                    assert!(row.axis.is_some());
                }
            }
            observed.set(counts);
            Ok(())
        },
    )?;
    assert_eq!(budget.storage(), floor);
    Ok(())
}

fn projection_summary_v25(exclusive: bool) -> SourceSliceArgumentCompletionV25 {
    SourceSliceArgumentCompletionV25 {
        ty: SemanticTypeIdV1::from_index(0),
        identity: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1::from_sha256([0; 32]),
        scalar: ScalarType::U32,
        exclusive,
        reads: 0,
        writes: 0,
        axis: None,
        different_projection: false,
    }
}

#[test]
fn slice_pairwise_summary_accepts_identical_global_projections_in_every_order() {
    use fe2o3_kernel_ir::Axis;
    for sequence in [
        [true, false, true],
        [false, true, true],
        [true, true, false],
    ] {
        let mut row = projection_summary_v25(true);
        for writing in sequence {
            row.record_projection_v25(writing, Some(Axis::X)).unwrap();
        }
        assert_eq!(
            (row.reads, row.writes, row.axis, row.different_projection),
            (1, 2, Some(Axis::X), false)
        );
    }
}

#[test]
fn slice_pairwise_summary_rejects_different_or_unknown_projections_before_and_after_stores() {
    use fe2o3_kernel_ir::Axis;
    for changed in [None, Some(Axis::Y), Some(Axis::Z)] {
        for read_first in [false, true] {
            let mut row = projection_summary_v25(true);
            if read_first {
                row.record_projection_v25(false, changed).unwrap();
                assert!(row.record_projection_v25(true, Some(Axis::X)).is_err());
            } else {
                row.record_projection_v25(true, Some(Axis::X)).unwrap();
                assert!(row.record_projection_v25(false, changed).is_err());
            }
        }
        let mut row = projection_summary_v25(true);
        row.record_projection_v25(true, Some(Axis::X)).unwrap();
        assert!(row.record_projection_v25(true, changed).is_err());
    }
}

#[test]
fn slice_pairwise_summary_refuses_read_only_writes_and_retains_unprojected_reads() {
    use fe2o3_kernel_ir::Axis;
    let mut row = projection_summary_v25(false);
    row.record_projection_v25(false, None).unwrap();
    row.record_projection_v25(false, Some(Axis::Y)).unwrap();
    assert_eq!((row.reads, row.writes), (2, 0));
    assert!(row.different_projection);
    assert!(row.record_projection_v25(true, Some(Axis::X)).is_err());
    let mut row = projection_summary_v25(false);
    assert!(row.record_projection_v25(true, Some(Axis::X)).is_err());
}

#[test]
fn slice_pairwise_summary_refuses_counter_overflow() {
    use fe2o3_kernel_ir::Axis;
    for writing in [false, true] {
        let mut row = projection_summary_v25(true);
        row.axis = Some(Axis::X);
        if writing {
            row.writes = usize::MAX;
        } else {
            row.reads = usize::MAX;
        }
        assert!(matches!(
            row.record_projection_v25(writing, Some(Axis::X)),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Arithmetic
            ))
        ));
    }
}
