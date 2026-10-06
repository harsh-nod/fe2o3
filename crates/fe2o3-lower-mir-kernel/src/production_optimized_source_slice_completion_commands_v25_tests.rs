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
            assert_eq!(
                completion.arguments.len(),
                completion
                    .source
                    .profile
                    .argument_count_v25(budget)
                    .map_err(source_argument_error_v18)?
            );
            for (ordinal, row) in completion.arguments.iter().enumerate() {
                let argument =
                    u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                assert_eq!(
                    completion
                        .source
                        .profile
                        .slice_parameter_type_v26(argument, budget)
                        .map_err(source_argument_error_v18)?,
                    row.as_ref().map(|row| row.ty),
                );
            }
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
        parameter: SliceDefinition::FunctionArgument {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
            argument: 0,
        },
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

pub(super) fn test_slice_parameter_census_refusal_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
    reached: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.with_global_source_expressions_v23(optimized, budget, &mut |_, source, budget| {
        source.with_shared_entry_regions_v18(budget, |entries, budget| {
            entries.with_slice_entry_index_v26(budget, &mut |index, budget| {
                let floor = budget.storage();
                budget.reserve_storage(slice_parameters_headers_v26()?)?;
                let mut rows = source_reference_emission_vec_v29(index.rows.len(), budget)
                    .map_err(source_argument_error_v18)?;
                budget.charge_work(index.rows.len())?;
                rows.extend_from_slice(index.rows);
                let selected = rows
                    .iter_mut()
                    .find(|row| row.is_some_and(|row| row.argument == 1))
                    .expect("genuine unused second slice declaration");
                match fault {
                    0 => *selected = None,
                    1 => selected.as_mut().unwrap().argument = 0,
                    2 => selected.as_mut().unwrap().slot = usize::MAX,
                    3 => selected.as_mut().unwrap().value = ValueId(u32::MAX),
                    4 => selected.as_mut().unwrap().ty = SemanticTypeIdV1::from_index(u32::MAX),
                    _ => unreachable!(),
                }
                let count = entries
                    .profile
                    .argument_count_v25(budget)
                    .map_err(source_argument_error_v18)?;
                let mut completed = source_reference_emission_vec_v29(count, budget)
                    .map_err(source_argument_error_v18)?;
                budget.charge_work(count)?;
                completed.resize(count, None);
                let changed = SourceSliceEntryIndexV26 {
                    source: index.source,
                    rows: &rows,
                };
                let result = changed.complete_parameters_v26(&mut completed, budget);
                assert!(
                    result.is_err(),
                    "counterfeit declaration census was admitted"
                );
                reached.set(true);
                drop(changed);
                drop(completed);
                drop(rows);
                budget.release_storage(
                    budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(ArgumentResourceV1::Accounting)?,
                )?;
                result
            })
        })
    })
}

#[test]
fn slice_parameter_census_headers_match_independent_exact_and_one_short_oracle() {
    // Independent inventory of the new declaration walk. The preexisting ABI
    // query quote is a separately owned component, not the census quote itself.
    type CensusOracle<'a> = (
        &'a SourceSliceEntryIndexV26<'a, 'a>,
        &'a PendingSharedEntryRegionsV18<'a, 'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        [&'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>; 2],
        [&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>; 2],
        &'a mut [Option<SourceSliceArgumentCompletionV25>],
        &'a SourceSliceIndexedArgumentV26,
        &'a mut Option<SourceSliceArgumentCompletionV25>,
        SourceSliceArgumentCompletionV25,
        [SliceDefinition; 2],
        Option<SliceDefinition>,
        Option<SemanticTypeIdV1>,
        [usize; 4],
        [u32; 2],
        std::iter::Enumerate<std::slice::Iter<'a, Option<SourceSliceIndexedArgumentV26>>>,
        std::iter::Enumerate<std::slice::Iter<'a, Option<SourceSliceArgumentCompletionV25>>>,
        std::slice::Iter<'a, fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    let expected = size_of::<CensusOracle<'_>>()
        .checked_add(
            size_of::<SourceOwnedResultV18<CensusOracle<'_>>>()
                .checked_mul(2)
                .unwrap(),
        )
        .unwrap()
        .checked_add(kernel_argument_abi_v18::slice_entry_abi_headers_v25().unwrap())
        .unwrap();
    assert_eq!(slice_parameters_headers_v26().unwrap(), expected);
    let floor = 17;
    for short in [false, true] {
        let limit = floor + expected - usize::from(short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let result = budget.reserve_storage(slice_parameters_headers_v26().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == floor + expected && error.limit() == limit));
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), floor + expected);
            assert_eq!(budget.peak_storage(), floor + expected);
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}
