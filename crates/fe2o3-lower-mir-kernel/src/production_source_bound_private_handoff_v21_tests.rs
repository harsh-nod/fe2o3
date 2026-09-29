use super::*;

fn launches_v21(count: usize) -> Vec<fe2o3_kernel_ir::CanonicalFormalLaunchInputV19> {
    vec![
        fe2o3_kernel_ir::CanonicalFormalLaunchInputV19::PhysicalEnvelope(
            fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [192, 1, 1]
            }
        );
        count
    ]
}

fn memory_census_v21(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18) -> [usize; 3] {
    let mut result = [0; 3];
    for operation in owner
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        match operation.kind {
            OperationKind::Alloca { .. } => result[0] += 1,
            OperationKind::Load { .. }
            | OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue { .. }) => {
                result[1] += 1
            }
            OperationKind::Store { .. }
            | OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::WriteValue { .. }) => {
                result[2] += 1
            }
            _ => {}
        }
    }
    result
}

#[test]
fn bound_private_pointer_reason_role_is_exact_not_a_general_escape_discharge() {
    use fe2o3_kernel_ir::{AddressSpace, MemoryAccess, StorageOperationV1, StorageProjectionV1};
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let mut volatile = access;
    volatile.volatile = true;
    let read = |address, access| {
        OperationKind::Storage(StorageOperationV1::ReadValue {
            address: ValueId(address),
            access,
        })
    };
    let write = |address, value, access| {
        OperationKind::Storage(StorageOperationV1::WriteValue {
            address: ValueId(address),
            value: ValueId(value),
            access,
        })
    };
    assert_eq!(
        ProductionPrivateMemoryCheckedNativePoliciesV18::test_private_formal_storage_pointer_v21(
            &read(3, access),
            &read(7, access),
            ValueId(3)
        ),
        Some(ValueId(7))
    );
    assert_eq!(
        ProductionPrivateMemoryCheckedNativePoliciesV18::test_private_formal_storage_pointer_v21(
            &write(3, 4, access),
            &write(7, 8, access),
            ValueId(3)
        ),
        Some(ValueId(7))
    );
    for (before, after, pointer) in [
        (read(3, access), read(7, access), ValueId(4)),
        (write(3, 4, access), write(7, 8, access), ValueId(4)),
        (read(3, access), write(7, 8, access), ValueId(3)),
        (write(3, 4, access), read(7, access), ValueId(3)),
        (
            read(3, access),
            read(7, MemoryAccess::new(AddressSpace::Private, 8)),
            ValueId(3),
        ),
        (
            read(3, MemoryAccess::new(AddressSpace::Global, 4)),
            read(7, MemoryAccess::new(AddressSpace::Global, 4)),
            ValueId(3),
        ),
        (
            OperationKind::Storage(StorageOperationV1::Project {
                base: ValueId(3),
                step: StorageProjectionV1::Field(0),
            }),
            read(7, access),
            ValueId(3),
        ),
        (
            OperationKind::Load {
                pointer: ValueId(3),
                access,
            },
            read(7, access),
            ValueId(3),
        ),
        (read(3, volatile), read(7, volatile), ValueId(3)),
    ] {
        assert_eq!(
            ProductionPrivateMemoryCheckedNativePoliciesV18::test_private_formal_storage_pointer_v21(&before, &after, pointer),
            None
        );
    }
}

#[test]
fn bound_private_handoff_keeps_actual_nonempty_load_store_owners_and_final_boundary() {
    for factory in [
        private_entry_forward_owner_v20 as fn() -> _,
        private_entry_constant_owner_v20,
        private_entry_captured_owner_v20,
        private_entry_root_owner_v20,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let launches = launches_v21(source.root_count(budget)?);
                let input = source.canonical(budget)?;
                assert!(memory_census_v21(input).into_iter().all(|count| count > 0));
                let handoff = source.checked_bound_private_output_v21(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                )?;
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                let output = handoff.output(budget)?;
                assert!(
                    memory_census_v21(output.owner())
                        .into_iter()
                        .all(|count| count > 0)
                );
                assert_eq!(output.input_audit_bytes(), input.canonical_bytes());
                assert_eq!(output.owner().identity(), output.map().output_identity());
                assert_eq!(output.owner().module().kernels.len(), roots.len());
                assert!(!output.grants_authority());
                assert!(!handoff.ranked_verification_is_complete());
                assert!(!handoff.grants_artifact_or_launch_authority());
                assert_eq!(
                    handoff.formal_context_v21(budget)?,
                    (
                        launches.as_slice(),
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64
                    )
                );
                assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok::<_, ProductionBoundPrivateHandoffErrorV21>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn bound_private_handoff_retains_actual_changed_and_noop_fixed_integer_outputs() {
    for (factory, changed) in [
        (private_entry_neutral_owner_v20 as fn() -> _, true),
        (private_entry_non_neutral_owner_v20, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                assert!(
                    private_entry_typed_memory_census_v20(source.canonical(budget)?)
                        .into_iter()
                        .all(|count| count > 0)
                );
                let launches = launches_v21(source.root_count(budget)?);
                let handoff = source.checked_bound_private_output_v21(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                )?;
                let output = handoff.output(budget)?;
                assert!(
                    private_entry_typed_memory_census_v20(output.owner())
                        .into_iter()
                        .all(|count| count > 0)
                );
                assert_eq!(output.report().passes()[0].changed(), changed);
                assert_eq!(
                    output.owner().canonical_bytes() != output.input_audit_bytes(),
                    changed
                );
                handoff.discard(budget)?;
                completed.set(true);
                Ok::<_, ProductionBoundPrivateHandoffErrorV21>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn binding_cause_v21(error: &(dyn std::error::Error + 'static), expected: &'static str) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if matches!(error.downcast_ref::<ProductionSourceOwnedViewErrorV18>(), Some(ProductionSourceOwnedViewErrorV18::Binding(message)) if *message == expected)
        {
            return true;
        }
        current = error.source();
    }
    false
}

#[test]
fn bound_private_handoff_refuses_missing_root_abi_and_unknown_launch_without_deleting_reasons() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) =
            integer_handoff_prepared_v18(private_entry_forward_owner_v20, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let mut launches = launches_v21(source.root_count(budget)?);
            if mode == 0 { launches.pop(); }
            if mode == 2 { launches[0] = fe2o3_kernel_ir::CanonicalFormalLaunchInputV19::Exact(fe2o3_kernel_ir::ExplicitLaunchExtent::Unknown); }
            let abi = if mode == 1 { &roots[..1] } else { &roots };
            let error = source.checked_bound_private_output_v21(ProductionKernelArgumentAbiInputV18 { roots: abi }, &launches, fe2o3_kernel_ir::FormalIndexWidth::Bits64, budget).err().expect("incomplete binding must refuse");
            if mode < 2 {
                assert!(binding_cause_v21(&error, if mode == 0 { "paired formal report complete launch roster" } else { "kernel argument ABI profile differs from its original descriptor/source contract" }), "{error:?}");
            } else {
                let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&error);
                let mut exact = false;
                while let Some(cause) = current {
                    exact |= matches!(cause.downcast_ref::<ProductionBoundPrivateResidualV21>(), Some(ProductionBoundPrivateResidualV21::Reason(0)));
                    current = cause.source();
                }
                assert!(exact, "{error:?}");
            }
            completed.set(true);
            Err::<(), _>(error)
        });
        assert!(result.is_err());
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn bound_private_handoff_rejects_equal_identity_foreign_source_and_foreign_ledger() {
    for foreign_ledger in [false, true] {
        let foreign = private_entry_forward_owner_v20();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) =
            integer_handoff_prepared_v18(private_entry_forward_owner_v20, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let launches = launches_v21(source.root_count(budget)?);
            let handoff = source.checked_bound_private_output_v21(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            let held = budget.storage();
            let error = if foreign_ledger {
                let mut foreign_work =
                    CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                let foreign_budget = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                handoff.output(&foreign_budget).err().unwrap()
            } else {
                assert_eq!(foreign.identity(), source.source_ssa(budget)?.identity());
                handoff.check_original_source(&foreign, budget).unwrap_err()
            };
            assert_eq!(budget.storage(), held);
            if foreign_ledger {
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
            } else {
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
                ));
            }
            let _ = handoff.discard(budget);
            completed.set(true);
            Err::<(), ProductionBoundPrivateHandoffErrorV21>(error.into())
        });
        assert!(result.is_err());
        assert!(completed.get());
        if !foreign_ledger {
            assert_eq!(budget.storage(), MODULE_FLOOR);
        } else {
            assert!(budget.storage() > MODULE_FLOOR);
        }
    }
}

fn bound_private_resource_cause_v21(
    error: &(dyn std::error::Error + 'static),
) -> Option<ArgumentResourceV1> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(ProductionSourceOwnedViewErrorV18::Resource(resource)) =
            error.downcast_ref::<ProductionSourceOwnedViewErrorV18>()
        {
            return Some(*resource);
        }
        if let Some(resource) = error.downcast_ref::<ArgumentResourceV1>() {
            return Some(*resource);
        }
        if let Some(resource) =
            error.downcast_ref::<fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1>()
        {
            return Some(optimized_source_formal_resource_v18(*resource));
        }
        current = error.source();
    }
    None
}

#[test]
fn bound_private_handoff_header_failure_is_first_and_retry_spends_nothing() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_forward_owner_v20, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let launches = launches_v21(source.root_count(budget)?);
        let padding = MODULE_LIMIT - budget.storage();
        budget.reserve_storage(padding)?;
        let error = source
            .checked_bound_private_output_v21(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )
            .err()
            .unwrap();
        let first = bound_private_resource_cause_v21(&error).unwrap();
        assert!(
            matches!(first, ArgumentResourceV1::Storage(refusal) if refusal.limit() == MODULE_LIMIT)
        );
        budget.release_storage(padding)?;
        let after = (budget.work(), budget.storage());
        let retry = source
            .checked_bound_private_output_v21(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )
            .err()
            .unwrap();
        assert_eq!(bound_private_resource_cause_v21(&retry), Some(first));
        assert_eq!((budget.work(), budget.storage()), after);
        completed.set(true);
        Err::<(), _>(error)
    });
    assert!(result.is_err());
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn bound_private_live_reason_cursor_rejects_duplicate_skipped_missing_and_wrong_site_coverage() {
    for mode in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let headers = private_source_completion_headers_v20().unwrap()
            + size_of::<PrivateFormalReasonCursorV21<'_, '_, '_>>()
            + size_of::<
                Result<
                    (),
                    ProductionOptimizedSourceReportsErrorV19<ProductionBoundPrivateResidualV21>,
                >,
            >()
            + size_of::<Result<(), ProductionPrivateSourceCheckErrorV20>>()
            + size_of::<ProductionBoundPrivateResidualV21>()
            + size_of::<[usize; 16]>();
        budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
        let prepared =
            private_memory_prepared_v18(typed_root_entry_rhs_owner_v18, &mut budget).unwrap();
        let completed = std::cell::Cell::new(None);
        let settled = std::cell::Cell::new(false);
        let result = with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                let launches = launches_v21(original.source.root_count(budget)?);
                let mut consume =
                    |native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
                     budget: &mut ArgumentBudgetV1<'_>| {
                        let functions = native.function_count(budget)?;
                        assert!(functions > 0);
                        assert!(native.report(functions, budget)?.is_none());
                        assert!(native.history(functions, budget)?.is_none());
                        let result = original.with_optimized_formal_reports_v19(optimized, &launches, fe2o3_kernel_ir::FormalIndexWidth::Bits64, fe2o3_kernel_ir::ControlFlowLimits::DEFAULT, budget, |before, after, budget| {
                    let count = before.analysis().incomplete_reasons().len();
                    assert_eq!(count, 3, "two typed accesses plus their conservative private-pointer reason: {:?}", before.analysis().incomplete_reasons());
                    assert!(matches!(before.analysis().incomplete_reasons()[0], fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }));
                    assert!(matches!(before.analysis().incomplete_reasons()[1], fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }));
                    assert!(matches!(before.analysis().incomplete_reasons()[2], fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedPointerDerivation { .. }));
                    let mut cursor = PrivateFormalReasonCursorV21::new(native, before, budget)?;
                    if mode == 4 {
                        for ordinal in 0..count { cursor.check_next(native, ordinal, budget)?; }
                        cursor.finish(native, budget)?;
                        // The output owner has an independent invariant lifetime.
                        let mut output_cursor = PrivateFormalReasonCursorV21::new(native, after, budget)?;
                        for ordinal in 0..after.analysis().incomplete_reasons().len() { output_cursor.check_next(native, ordinal, budget)?; }
                        output_cursor.finish(native, budget)?;
                        completed.set(Some(count));
                        return Ok(());
                    }
                    let error = match mode {
                        0 => { cursor.check_next(native, 0, budget)?; let error = cursor.check_next(native, 0, budget).unwrap_err(); assert!(matches!(cursor.check_next(native, 1, budget), Err(ProductionBoundPrivateResidualV21::Roster))); assert!(matches!(cursor.finish(native, budget), Err(ProductionBoundPrivateResidualV21::Roster))); error },
                        1 => { let error = cursor.check_next(native, 1, budget).unwrap_err(); assert!(matches!(cursor.check_next(native, 0, budget), Err(ProductionBoundPrivateResidualV21::Roster))); error },
                        2 => cursor.finish(native, budget).unwrap_err(),
                        _ => {
                            let error = native.check_formal_reason_v21(before, count, budget).unwrap_err();
                            assert!(matches!(error, ProductionSourceNativeLifecycleErrorV18::Source(ProductionSourceOwnedViewErrorV18::Binding("private formal reason lacks exact memory occurrence"))));
                            ProductionBoundPrivateResidualV21::Coverage { ordinal: count, error }
                        },
                    };
                    if mode < 3 { assert!(matches!(error, ProductionBoundPrivateResidualV21::Roster)); }
                    completed.set(Some(count));
                    Err::<(), _>(error)
                });
                        assert_eq!(result.is_ok(), mode == 4, "{result:?}");
                        settled.set(true);
                        Ok(())
                    };
                let private =
                    with_private_source_completion_v21(original, optimized, budget, &mut consume);
                if mode == 4 {
                    assert!(private.is_ok(), "{private:?}");
                }
                Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                    "selected private reason control complete",
                ))
            },
        );
        assert!(result.is_err());
        assert!(completed.get().is_some_and(|count| count > 1));
        assert!(settled.get());
        assert_eq!(budget.storage(), MODULE_FLOOR + headers);
    }
}

fn private_handoff_cut_v21(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_root_owner_v20, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(None);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let launches = launches_v21(source.root_count(budget)?);
        let initial = budget.storage();
        let mut padding = 0;
        if let Some((work, remaining)) = cut {
            if work {
                budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - remaining)?;
            } else {
                padding = MODULE_LIMIT - budget.storage() - remaining;
                budget.reserve_storage(padding)?;
            }
        }
        let before = (budget.work(), budget.storage());
        let attempt = source.checked_bound_private_output_v21(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &launches,
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            budget,
        );
        match attempt {
            Ok(handoff) => {
                let used = budget.work() - before.0;
                let peak = budget.peak_storage() - before.1;
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), initial + padding);
                budget.release_storage(padding)?;
                completed.set(Some((used, peak, true)));
                Err::<(), ProductionBoundPrivateHandoffErrorV21>(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "selected private constructor boundary",
                    )
                    .into(),
                )
            }
            Err(error) => {
                let (is_work, _) = cut.expect("uncut genuine private constructor must pass");
                let resource =
                    bound_private_resource_cause_v21(&error).expect("typed resource chain");
                match (is_work, resource) {
                    (true, ArgumentResourceV1::Work(refusal)) => {
                        assert_eq!(refusal.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                        assert!(refusal.actual() > refusal.limit())
                    }
                    (false, ArgumentResourceV1::Storage(refusal)) => {
                        assert_eq!(refusal.limit(), MODULE_LIMIT);
                        assert!(refusal.actual() > refusal.limit())
                    }
                    _ => panic!("wrong boundary error: {error:?}"),
                }
                budget.release_storage(padding)?;
                completed.set(Some((0, 0, false)));
                Err(error)
            }
        }
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
    completed
        .get()
        .expect("all inner boundary assertions must complete")
}

#[test]
fn bound_private_handoff_complete_constructor_has_exact_and_one_short_work_and_storage() {
    let (work, storage, accepted) = private_handoff_cut_v21(None);
    assert!(accepted && work > 0 && storage > 0);
    for (is_work, remaining) in [(true, work), (false, storage)] {
        assert!(private_handoff_cut_v21(Some((is_work, remaining))).2);
        assert!(!private_handoff_cut_v21(Some((is_work, remaining - 1))).2);
    }
}

#[test]
fn bound_private_reason_attempt_header_one_short_latches_before_inner_work_and_retry() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let headers = private_source_completion_headers_v20().unwrap();
    budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
    let prepared =
        private_memory_prepared_v18(typed_root_entry_rhs_owner_v18, &mut budget).unwrap();
    let completed = std::cell::Cell::new(false);
    let settled = std::cell::Cell::new(false);
    let result = with_production_optimizer_result_v18(
        prepared,
        &mut budget,
        |original, optimized, budget| {
            let launches = launches_v21(original.source.root_count(budget)?);
            let mut consume = |native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
                               budget: &mut ArgumentBudgetV1<'_>| {
                let result = original.with_optimized_formal_reports_v19(optimized, &launches, fe2o3_kernel_ir::FormalIndexWidth::Bits64, fe2o3_kernel_ir::ControlFlowLimits::DEFAULT, budget, |before, _, budget| {
                assert!(!before.analysis().incomplete_reasons().is_empty());
                native.check_formal_reason_v21(before, 0, budget).unwrap_or_else(|error| panic!("positive reason query failed before header cut: {error:?}; reasons={:?}", before.analysis().incomplete_reasons()));
                let floor = budget.storage();
                let header = scoped_source_attempt_header_oracle_v29::<(), ProductionSourceNativeLifecycleErrorV18, [usize; 5]>();
                let padding = MODULE_LIMIT - floor - header + 1;
                budget.reserve_storage(padding).unwrap();
                let error = native.check_formal_reason_v21(before, 0, budget).unwrap_err();
                let first = bound_private_resource_cause_v21(&error).unwrap();
                assert!(matches!(first, ArgumentResourceV1::Storage(limit) if limit.actual() == MODULE_LIMIT + 1 && limit.limit() == MODULE_LIMIT));
                assert_eq!(budget.storage(), floor + padding);
                budget.release_storage(padding).unwrap();
                let state = (budget.work(), budget.storage());
                let retry = native.check_formal_reason_v21(before, 0, budget).unwrap_err();
                assert_eq!(bound_private_resource_cause_v21(&retry), Some(first));
                assert_eq!((budget.work(), budget.storage()), state);
                completed.set(true);
                Err::<(), _>(error)
            });
                // The containing effects scope independently observes the same
                // denied ledger before report consumer settlement completes.
                assert!(
                    matches!(
                        &result,
                        Err(ProductionOptimizedSourceReportsErrorV19::Formal {
                            error: fe2o3_kernel_ir::CanonicalFormalReportErrorV19::Effects(
                                fe2o3_kernel_ir::CanonicalEffectErrorV19::Resource(ArgumentResourceV1::Storage(refusal))),
                            source_refusal: ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(first)),
                        }) if refusal.actual() == MODULE_LIMIT + 1 && refusal.limit() == MODULE_LIMIT
                            && first == refusal
                    ),
                    "{result:?}"
                );
                settled.set(true);
                Ok(())
            };
            let _ = with_private_source_completion_v21(original, optimized, budget, &mut consume);
            Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                "private reason header control completed",
            ))
        },
    );
    assert!(result.is_err());
    assert!(completed.get() && settled.get());
    assert_eq!(budget.storage(), MODULE_FLOOR + headers);
}

#[test]
fn bound_private_handoff_rejects_equal_byte_and_edited_canonical_owner_substitutions() {
    for output in [false, true] {
        for edited in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let (prepared, fixture) =
                integer_handoff_prepared_v18(private_entry_forward_owner_v20, &mut budget);
            let roots = fixture.roots();
            let completed = std::cell::Cell::new(false);
            let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                let launches = launches_v21(source.root_count(budget)?);
                let handoff = source.checked_bound_private_output_v21(ProductionKernelArgumentAbiInputV18 { roots: &roots }, &launches, fe2o3_kernel_ir::FormalIndexWidth::Bits64, budget)?;
                let owner = if output { handoff.output(budget)?.owner() } else { source.canonical(budget)? };
                let mut module = owner.module().clone();
                if edited { module.id = "changed-private-report-subject".into(); }
                let (other, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(&module, source.limits(budget)?.storage_layout_limits(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage())?;
                assert_eq!(other.canonical_bytes() == owner.canonical_bytes(), !edited);
                assert!(!std::ptr::eq(&other, owner));
                let error = handoff.check_reported_owner_v21(&other, budget).unwrap_err();
                assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding("bound private formal owner differs")));
                drop(other);
                budget.release_storage(receipt.retained_storage())?;
                assert!(matches!(handoff.output(budget), Err(ProductionSourceOwnedViewErrorV18::Binding("bound private formal owner differs"))));
                assert!(matches!(handoff.discard(budget), Err(ProductionSourceOwnedViewErrorV18::Binding("bound private formal owner differs"))));
                completed.set(true);
                Err::<(), ProductionBoundPrivateHandoffErrorV21>(error.into())
            });
            assert!(result.is_err());
            assert!(completed.get());
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn bound_private_reason_rejects_foreign_live_report_and_failed_foreign_account() {
    for foreign_account in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let headers = private_source_completion_headers_v20().unwrap();
        budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
        let prepared =
            private_memory_prepared_v18(typed_root_entry_rhs_owner_v18, &mut budget).unwrap();
        let completed = std::cell::Cell::new(false);
        let settled = std::cell::Cell::new(false);
        let result = with_production_optimizer_result_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                let launch = launches_v21(1)[0];
                let owner = original.source.canonical(budget)?;
                let mut consume =
                    |native: &ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_>,
                     budget: &mut ArgumentBudgetV1<'_>| {
                        let layouts = original.source.limits(budget)?.storage_layout_limits();
                        let (other, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(owner.module(), layouts, budget).unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        assert_eq!(other.canonical_bytes(), owner.canonical_bytes());
                        let subject = if foreign_account { owner } else { &other };
                        fe2o3_kernel_ir::with_canonical_effects_v19(
                            subject,
                            budget,
                            |effects, budget| {
                                let report =
                                    fe2o3_kernel_ir::with_canonical_owner_formal_report_v19(
                                        subject,
                                        0,
                                        effects,
                                        launch,
                                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                                        fe2o3_kernel_ir::ControlFlowLimits::DEFAULT,
                                        budget,
                                        |report, budget| {
                                            assert!(
                                                !report.analysis().incomplete_reasons().is_empty()
                                            );
                                            let state = (budget.work(), budget.storage());
                                            if foreign_account {
                                                let mut failed =
                                                    CanonicalKernelIrWorkBudgetV1::new(0);
                                                let mut foreign = ArgumentBudgetV1::new(
                                                    &mut failed,
                                                    MODULE_LIMIT,
                                                );
                                                assert!(foreign.charge_work(1).is_err());
                                                let before = (foreign.work(), foreign.storage());
                                                let error = native
                                                    .check_formal_reason_v21(
                                                        report,
                                                        0,
                                                        &mut foreign,
                                                    )
                                                    .unwrap_err();
                                                assert!(matches!(
                                                    error,
                                                    ProductionSourceNativeLifecycleErrorV18::Source(
                                                        ProductionSourceOwnedViewErrorV18::Resource(
                                                            ArgumentResourceV1::Accounting
                                                        )
                                                    )
                                                ));
                                                assert_eq!(
                                                    (foreign.work(), foreign.storage()),
                                                    before
                                                );
                                                assert_eq!(
                                                    (budget.work(), budget.storage()),
                                                    state
                                                );
                                            } else {
                                                assert!(!std::ptr::eq(
                                                    report.original_owner(),
                                                    owner
                                                ));
                                                let error = native
                                                    .check_formal_reason_v21(report, 0, budget)
                                                    .unwrap_err();
                                                assert!(matches!(
                                                    error,
                                                    ProductionSourceNativeLifecycleErrorV18::Source(
                                                        ProductionSourceOwnedViewErrorV18::Binding(
                                                            "private formal report changed owner"
                                                        )
                                                    )
                                                ));
                                                assert_eq!(budget.storage(), state.1);
                                            }
                                            completed.set(true);
                                            Ok(())
                                        },
                                    );
                                assert!(report.is_ok(), "{report:?}");
                                Ok(())
                            },
                        )
                        .unwrap();
                        drop(other);
                        budget.release_storage(receipt.retained_storage()).unwrap();
                        settled.set(true);
                        Ok(())
                    };
                let _ =
                    with_private_source_completion_v21(original, optimized, budget, &mut consume);
                Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                    "private foreign report control completed",
                ))
            },
        );
        assert!(result.is_err());
        assert!(completed.get() && settled.get());
        if foreign_account {
            assert!(budget.storage() > MODULE_FLOOR + headers);
        } else {
            assert_eq!(budget.storage(), MODULE_FLOOR + headers);
        }
    }
}

fn external_report_module_v21() -> fe2o3_kernel_ir::Module {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, Constant, Function,
        IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
        ScalarType, Signature, StorageLayoutKindV1, StorageLayoutV1, Terminator, Type, ValueDef,
        ValueId,
    };
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let value = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        value(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        value(4, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        value(
            5,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(3),
                rhs: ValueId(4),
            },
        ),
        value(
            6,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        value(7, pointer, OperationKind::SliceData { slice: ValueId(1) }),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(5),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut stores = BasicBlock::new(BlockId(1));
    for pointer in [6, 7] {
        stores.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(pointer),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    stores.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("private-class-external-refusals");
    let slice = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![slice.clone(), slice, scalar], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, stores, exit],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    module
}

#[test]
fn bound_private_external_rows_remain_required_even_when_every_conflict_path_is_excluded() {
    use fe2o3_kernel_analysis::{
        FormalPaidPathDecisionV20, with_formal_path_observations_v21, with_presburger_queries_v4,
    };
    use fe2o3_kernel_ir::{with_canonical_effects_v19, with_canonical_owner_formal_report_v19};
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let module = external_report_module_v21();
    let layouts = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
        rows: 64,
        edges: 256,
        containment_depth: 32,
        object_bytes: 4096,
    };
    let (owner, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(&module, layouts, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let completed = std::cell::Cell::new(false);
    with_canonical_effects_v19(&owner, &mut budget, |effects, budget| {
        let report = with_canonical_owner_formal_report_v19(
            &owner,
            0,
            effects,
            launches_v21(1)[0],
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            fe2o3_kernel_ir::ControlFlowLimits::DEFAULT,
            budget,
            |report, budget| {
                let rows = bound_private_external_rows_v21(report.analysis());
                assert!(rows.iter().all(|(required, _)| *required));
                assert_eq!(
                    rows[0],
                    (true, ProductionBoundPrivateObligationV21::Conflict)
                );
                let original = report.analysis().obligations().inter_invocation_conflicts();
                // Distinct formal allocations have a runtime alias obligation;
                // each store separately has an inter-invocation self-conflict.
                let accesses = report.analysis().obligations().accesses();
                assert_eq!(accesses.len(), 2);
                assert_eq!(original.len(), 2);
                for (conflict, access) in original.iter().zip(accesses) {
                    assert_eq!(conflict.left(), access.location());
                    assert_eq!(conflict.right(), access.location());
                    assert_eq!(conflict.allocation(), access.allocation());
                }
                let aliases = report.analysis().obligations().runtime_alias_requirements();
                assert_eq!(aliases.len(), 1);
                assert_ne!(aliases[0].left(), aliases[0].right());
                let session =
                    with_presburger_queries_v4(Default::default(), budget, |queries, budget| {
                        let source = report.source_scope_v20(budget).unwrap();
                        let result = with_formal_path_observations_v21(
                            source,
                            queries,
                            budget,
                            |path, _| {
                                assert!(std::ptr::eq(path.analysis(), report.analysis()));
                                assert_eq!(path.observations().len(), original.len());
                                assert!(path.observations().iter().all(|row| row.decision()
                                    == FormalPaidPathDecisionV20::ExcludedForAllU64Coordinates));
                                assert_eq!(bound_private_external_rows_v21(path.analysis()), rows);
                                completed.set(true);
                                Ok(())
                            },
                        );
                        assert!(result.is_ok(), "{result:?}");
                        Ok(())
                    });
                assert!(session.is_ok(), "{session:?}");
                Ok(())
            },
        );
        assert!(report.is_ok(), "{report:?}");
        Ok(())
    })
    .unwrap();
    assert!(completed.get());
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn bound_private_report_reason_order_uses_original_ids_with_nonmonotone_loop_blocks() {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, FormalMemoryIncompleteReason, Function,
        FunctionOperationLocation, Kernel, LaunchDomain, LaunchExtent, Module, Operation,
        ScalarType, Signature, StorageLayoutKindV1, StorageLayoutV1, Terminator, Type, ValueDef,
        ValueId,
    };
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(
        scalar.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut blocks: Vec<_> = [41, 3, 29]
        .into_iter()
        .enumerate()
        .map(|(ordinal, id)| {
            let mut block = BasicBlock::new(BlockId(id));
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(ordinal as u32 + 1), pointer.clone()),
                OperationKind::Alloca {
                    element: scalar.clone(),
                    count: None,
                    address_space: AddressSpace::Workgroup,
                    alignment: 4,
                },
            ));
            block
        })
        .collect();
    blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(3),
        then_arguments: vec![],
        else_target: BlockId(29),
        else_arguments: vec![],
    });
    blocks[2].terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("private-reason-block-id-order");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        blocks,
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let layouts = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
        rows: 64,
        edges: 256,
        containment_depth: 32,
        object_bytes: 4096,
    };
    let (owner, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(&module, layouts, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let completed = std::cell::Cell::new(false);
    fe2o3_kernel_ir::with_canonical_effects_v19(&owner, &mut budget, |effects, budget| {
        let result = fe2o3_kernel_ir::with_canonical_owner_formal_report_v19(
            &owner,
            0,
            effects,
            launches_v21(1)[0],
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            fe2o3_kernel_ir::ControlFlowLimits::DEFAULT,
            budget,
            |report, budget| {
                let expected =
                    [3, 29, 41].map(|id| FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                        location: FunctionOperationLocation::new(BlockId(id), 0),
                    });
                assert_eq!(report.analysis().incomplete_reasons(), expected);
                let source = report.source_scope_v20(budget)?;
                for (id, ordinal) in [(41, 0), (3, 1), (29, 2)] {
                    assert_eq!(source.block_ordinal(BlockId(id), budget)?, Some(ordinal));
                    assert!(source.reachable(BlockId(id), budget)?);
                }
                completed.set(true);
                Ok(())
            },
        );
        assert!(result.is_ok(), "{result:?}");
        Ok(())
    })
    .unwrap();
    assert!(completed.get());
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
