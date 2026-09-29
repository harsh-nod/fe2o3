pub(super) fn test_slice_entry_regions_v25(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    exclusive: bool,
    check_shapes: bool,
    counts: &std::cell::Cell<[usize; 2]>,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    test_slice_entry_regions_impl_v26(
        original,
        optimized,
        budget,
        exclusive,
        check_shapes,
        false,
        counts,
        completed,
    )
}

pub(super) fn test_slice_entry_regions_indexed_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    exclusive: bool,
    counts: &std::cell::Cell<[usize; 2]>,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    test_slice_entry_regions_impl_v26(
        original, optimized, budget, exclusive, false, true, counts, completed,
    )
}

fn test_slice_entry_regions_impl_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    exclusive: bool,
    check_shapes: bool,
    indexed: bool,
    counts: &std::cell::Cell<[usize; 2]>,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    original.with_global_source_expressions_v23(
        optimized,
        budget,
        &mut |root, source, budget| {
            source.with_shared_entry_regions_v18(budget, |entries, budget| {
                let mut run = |index: Option<&SourceSliceEntryIndexV26<'_, '_>>, budget: &mut ArgumentBudgetV1<'_>| {
                with_native_global_test_view_v18(optimized, budget, |native, budget| {
                    for row in source.roles.rows {
                        if !matches!(
                            row.role,
                            Some(DescriptorSourceRoleV18::Read | DescriptorSourceRoleV18::Write)
                        ) {
                            continue;
                        }
                        source
                            .with_native_access_v18(native, row.output, budget, |access, budget| {
                                let access = access.expect("actual native global occurrence");
                                let mut consume = |entry: &SourceSliceEntryRegionV25<'_, '_>, budget: &mut ArgumentBudgetV1<'_>| {
                                            assert_eq!(entry.root, root);
                                            assert_eq!(
                                                entry.requires_exclusive_runtime_binding(),
                                                exclusive
                                            );
                                            assert_eq!(
                                                entry.requires_initialized_extent(),
                                                !access.pair.output.writing
                                            );
                                            assert_eq!(
                                                entry.original_parameter(),
                                                access.pair.input.logical.root
                                            );
                                            assert_eq!(
                                                entry.optimized_parameter(),
                                                access.pair.output.logical.root
                                            );
                                            assert_eq!(
                                                entry.original_access(),
                                                access.pair.input.logical.access.operation
                                            );
                                            assert_eq!(entry.optimized_access(), row.output);
                                            assert!(!entry.grants_memory_or_launch_authority());
                                            assert_eq!(
                                                entry.abi.function(),
                                                entries.original_function
                                            );
                                            assert_eq!(
                                                entry.abi.argument(),
                                                entry.original_argument()
                                            );
                                            assert_eq!(
                                                entry.abi.ty(),
                                                entry.source.semantic_type()
                                            );
                                            assert_eq!(
                                                entry.abi.scalar(),
                                                access.pair.input.scalar
                                            );
                                            assert_eq!(
                                                entry.abi.scalar(),
                                                access.pair.output.scalar
                                            );
                                            assert_eq!(
                                                entry.abi.source(),
                                                original
                                                    .source
                                                    .owner
                                                    .inner
                                                    .source
                                                    .owner
                                                    .source_semantic_sha256()
                                            );
                                            if check_shapes && !access.pair.output.writing {
                                                let definition =
                                                    optimized_source_definition_row_v18(
                                                        original.inventory,
                                                        entry.original_parameter(),
                                                        budget,
                                                    )?;
                                                entries
                                                    .profile
                                                    .check_slice_entry_shapes_v25(
                                                        entry.original_argument(),
                                                        entry.source.semantic_type(),
                                                        definition.ty,
                                                        exclusive,
                                                        budget,
                                                    )
                                                    .map_err(source_argument_error_v18)?;
                                            }
                                            let mut observed = counts.get();
                                            observed[usize::from(access.pair.output.writing)] += 1;
                                            counts.set(observed);
                                            Ok(())
                                        };
                                match index {
                                    Some(index) => index.with_entry_v26(access, budget, &mut consume),
                                    None => entries.with_slice_entry_region_v25(access, budget, &mut consume),
                                }.map_err(PendingGlobalNativeErrorV18::from)
                            })
                            .unwrap();
                    }
                })
                };
                if indexed {
                    entries.with_slice_entry_index_v26(budget, &mut |index, budget| run(Some(index), budget))
                } else {
                    run(None, budget)
                }
            })
        },
    )?;
    assert_eq!(budget.storage(), floor);
    completed.set(true);
    Ok(())
}

pub(super) fn test_slice_entry_index_refusal_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
    reached: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    if fault == 3 {
        return with_source_slice_completion_v25(
            original,
            optimized,
            &[fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: 3,
                extents: [64, 1, 1],
            }],
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            Default::default(),
            budget,
            &mut |completion, budget| {
                completion
                    .source
                    .with_slice_entry_index_v26(budget, &mut |index, budget| {
                        let row = completion
                            .source
                            .source
                            .roles
                            .rows
                            .iter()
                            .find(|row| matches!(row.role, Some(DescriptorSourceRoleV18::Read)))
                            .expect("actual shared read");
                        completion
                            .source
                            .source
                            .with_native_access_v18(
                                completion.native,
                                row.output,
                                budget,
                                |access, budget| {
                                    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
                                    let mut foreign = ArgumentBudgetV1::new(&mut work, 100_000_000);
                                    foreign.reserve_storage(budget.storage())?;
                                    let denied = index.with_entry_v26(
                                        access.expect("actual native read"),
                                        &mut foreign,
                                        &mut |_, _| {
                                            panic!("foreign ledger entered indexed consumer")
                                        },
                                    );
                                    assert!(matches!(
                                        denied,
                                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                                            ArgumentResourceV1::Accounting
                                        ))
                                    ));
                                    reached.set(true);
                                    Ok(())
                                },
                            )
                            .map_err(slice_completion_native_error_v25)
                    })
            },
        );
    }
    original.with_global_source_expressions_v23(optimized, budget, &mut |_, source, budget| {
        source.with_shared_entry_regions_v18(budget, |entries, budget| {
            entries.with_slice_entry_index_v26(budget, &mut |index, budget| {
                if fault == 4 {
                    budget.release_storage(1)?;
                    reached.set(true);
                    return Ok(());
                }
                let row_floor = budget.storage();
                let mut rows = source_reference_emission_vec_v29(index.rows.len(), budget)
                    .map_err(source_argument_error_v18)?;
                budget.charge_work(index.rows.len())?;
                rows.extend_from_slice(index.rows);
                let selected = rows
                    .iter_mut()
                    .find(|row| row.is_some())
                    .expect("actual indexed slice argument");
                match fault {
                    0 => *selected = None,
                    1 => selected.as_mut().unwrap().slot = usize::MAX,
                    2 => selected.as_mut().unwrap().value = ValueId(u32::MAX),
                    _ => unreachable!(),
                }
                let changed = SourceSliceEntryIndexV26 {
                    source: index.source,
                    rows: &rows,
                };
                let mut result = None;
                let observed =
                    with_native_global_test_view_v18(optimized, budget, |native, budget| {
                        let row = source
                            .roles
                            .rows
                            .iter()
                            .find(|row| matches!(row.role, Some(DescriptorSourceRoleV18::Read)))
                            .expect("actual shared read");
                        result = Some(
                            source
                                .with_native_access_v18(
                                    native,
                                    row.output,
                                    budget,
                                    |access, budget| {
                                        let result = changed.with_entry_v26(
                                            access.expect("actual native access"),
                                            budget,
                                            &mut |_, _| {
                                                panic!("counterfeit index entered consumer")
                                            },
                                        );
                                        assert!(matches!(
                                            result,
                                            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
                                        ));
                                        reached.set(true);
                                        result.map_err(PendingGlobalNativeErrorV18::from)
                                    },
                                )
                                .map_err(slice_completion_native_error_v25),
                        );
                    });
                drop(changed);
                drop(rows);
                budget.release_storage(
                    budget
                        .storage()
                        .checked_sub(row_floor)
                        .ok_or(ArgumentResourceV1::Accounting)?,
                )?;
                observed?;
                result.expect("actual native callback")
            })
        })
    })
}
