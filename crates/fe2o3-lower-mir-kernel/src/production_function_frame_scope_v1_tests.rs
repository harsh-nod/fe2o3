use std::mem::size_of;

#[allow(dead_code)]
struct InvocationLayoutFields {
    source_entry: BlockId,
    preheader: Option<BlockId>,
    trap: Option<BlockId>,
    first_block: u32,
    next_block: u32,
    entry_predecessors: usize,
}

#[allow(dead_code)]
struct InvocationPlanFields {
    source: &'static SemanticFunctionDeclV1,
    ssa: &'static ProductionSemanticSsaFunctionPlanV1,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    layout: InvocationLayoutFields,
}

#[allow(dead_code)]
struct InvocationOwnerFields {
    plan: InvocationPlanFields,
    source: Option<&'static SourceReferencePlanV29<'static, 'static>>,
    growth: Option<source_storage_v29::SourceStorageRootGrowthV29<'static, 'static, 'static>>,
    entry: usize,
    required: usize,
    owned: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

fn independent_invocation_owner_headers() -> usize {
    type Error = ProductionSemanticKirErrorV1;
    assert_eq!(
        size_of::<InvocationLayoutFields>(),
        size_of::<InvocationBlockLayoutV1>()
    );
    assert_eq!(
        size_of::<InvocationPlanFields>(),
        size_of::<InvocationEntryPlanV1<'static>>()
    );
    assert_eq!(
        size_of::<InvocationOwnerFields>(),
        size_of::<OwnedInvocationEntryPlanV1<'static>>()
    );
    let bytes = size_of::<InvocationOwnerFields>()
        + size_of::<Result<InvocationOwnerFields, Error>>()
        + size_of::<Result<(), Error>>();
    assert_eq!(
        bytes,
        invocation_entry_plan_owner_v1::owned_invocation_entry_plan_headers_v1().unwrap()
    );
    bytes
}

fn independent_invocation_work_schedule(
    source: &SemanticFunctionDeclV1,
    ssa: &ProductionSemanticSsaFunctionPlanV1,
) -> Vec<usize> {
    // Eight owner custody/settlement units precede the existing four-unit
    // source check. Count each original reachable edge, not emitted blocks.
    let mut schedule = vec![8, 4];
    for (index, block) in source.blocks().iter().enumerate() {
        schedule.push(1);
        if ssa.plan().is_reachable(SsaBlockIdV1::new(index as u32)) {
            block
                .terminator()
                .kind()
                .try_for_each_edge(|_| {
                    schedule.push(2);
                    Ok::<_, ()>(())
                })
                .unwrap();
        }
    }
    let definitions = ssa.plan().entry_definitions().len();
    schedule.push(definitions);
    let lookup = definitions.checked_ilog2().unwrap_or(0) as usize + 2;
    schedule.extend(std::iter::repeat_n(
        lookup + 2,
        ssa.plan().entry_arguments().len(),
    ));
    schedule
}

#[test]
fn owning_invocation_scope_has_independent_exact_and_one_short_resources() {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        resource_tests::helper_closure_semantic_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let source = &owner.source_semantic().functions()[0];
    let ssa = owner
        .plan_for_function(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let headers = independent_invocation_owner_headers();
    let schedule = independent_invocation_work_schedule(source, ssa);
    let work: usize = schedule.iter().sum();
    let floor = 73;
    let prior = 11;
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let limit = prior + work - usize::from(short_work);
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget =
            ArgumentBudgetV1::new(&mut meter, floor + headers - usize::from(short_storage));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prior).unwrap();
        let result = OwnedInvocationEntryPlanV1::new(
            source,
            ssa,
            SemanticEmissionPlacementV1 {
                first_block: 17,
                first_value: 23,
            },
            false,
            None,
            &mut budget,
        );
        assert_eq!(result.is_ok(), !short_work && !short_storage);
        let expected_work = if short_storage {
            prior + 8
        } else {
            let mut spent = prior;
            for amount in &schedule {
                if spent + amount > limit {
                    break;
                }
                spent += amount;
            }
            spent
        };
        if let Ok(plan) = result {
            assert!(std::ptr::eq(plan.plan().source, source));
            assert!(std::ptr::eq(plan.plan().ssa, ssa));
            assert_eq!(budget.storage(), floor + headers);
            plan.settle(&mut budget).unwrap();
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), expected_work);
        assert_eq!(
            budget.peak_storage(),
            floor + headers
                - if short_storage {
                    size_of::<InvocationPlanFields>()
                } else {
                    0
                }
        );
        assert_eq!(
            budget.failed_storage(),
            short_storage.then_some(floor + headers)
        );
        drop(budget);
        assert_eq!(meter.failed_work(), short_work.then_some(prior + work));
    }
}

#[test]
fn owning_invocation_scope_settles_known_headers_and_preserves_failure_panic() {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        resource_tests::helper_closure_semantic_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let source = &owner.source_semantic().functions()[0];
    let ssa = owner
        .plan_for_function(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let other = owner
        .plan_for_function(SemanticFunctionIdV1::from_index(1))
        .unwrap();
    assert_ne!(source.identity(), other.function_identity());
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut meter, LIMIT);
    let floor = 73;
    budget.reserve_storage(floor).unwrap();
    let placement = SemanticEmissionPlacementV1 {
        first_block: 17,
        first_value: 23,
    };
    let plan =
        OwnedInvocationEntryPlanV1::new(source, ssa, placement, false, None, &mut budget).unwrap();
    budget.reserve_storage(13).unwrap();
    plan.settle(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor + 13);
    budget.release_storage(13).unwrap();
    assert!(matches!(
        OwnedInvocationEntryPlanV1::new(source, other, placement, false, None, &mut budget),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    assert_eq!(budget.storage(), floor);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scoped_slot_attempt_v29(&mut budget, |budget| {
            let _plan =
                OwnedInvocationEntryPlanV1::new(source, ssa, placement, false, None, budget)?;
            std::panic::panic_any(1174usize);
            #[allow(unreachable_code)]
            Ok::<_, ProductionSemanticKirErrorV1>(())
        })
    }));
    let payload = outcome.unwrap_err();
    assert_eq!(payload.downcast_ref::<usize>(), Some(&1174));
    assert_eq!(budget.storage(), floor);
}

#[allow(dead_code)]
struct LeaseFields<'a> {
    references: Option<&'a SourceReferenceEmissionV29<'a, 'a>>,
    growth: Option<source_storage_v29::SourceStorageRootGrowthV29<'a, 'a, 'a>>,
    entry: usize,
    required: usize,
    owned: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    denied: std::cell::Cell<bool>,
}

#[allow(dead_code)]
struct OwnedCursorFields<'a> {
    cursor: ExecutionAvailabilityV29<'a>,
    lease: LeaseFields<'a>,
}

fn independent_lease_headers() -> usize {
    type Cursor = ExecutionAvailabilityV29<'static>;
    type Failure = ProductionSemanticKirErrorV1;
    type Payload = Box<dyn std::any::Any + Send>;
    assert_eq!(
        size_of::<LeaseFields<'static>>(),
        size_of::<ExecutionAvailabilityLeaseV1<'static>>()
    );
    assert_eq!(
        size_of::<OwnedCursorFields<'static>>(),
        size_of::<OwnedExecutionAvailabilityV1<'static>>()
    );
    // Cursor value plus the two existing constructor return envelopes, then
    // receipt, settlement, unwind and owning-return envelopes. No run is used
    // to discover this amount.
    let bytes = size_of::<Cursor>()
        + 2 * size_of::<Result<Cursor, Failure>>()
        + size_of::<LeaseFields<'static>>()
        + size_of::<Result<(), Failure>>()
        + size_of::<Result<Result<Cursor, Failure>, Payload>>()
        + size_of::<Option<Payload>>()
        + size_of::<Result<(), Payload>>()
        + size_of::<Result<OwnedCursorFields<'static>, Failure>>();
    assert_eq!(bytes, execution_availability_lease_headers_v1().unwrap());
    bytes
}

#[test]
fn cursor_consumption_headers_have_independent_exact_and_one_short_limits() {
    type Failure = ProductionSemanticKirErrorV1;
    type Payload = Box<dyn std::any::Any + Send>;
    type Output = (u32, u32);
    type Outcome = Result<Result<Output, Failure>, Payload>;
    let headers = size_of::<Outcome>()
        + size_of::<(LeaseFields<'static>, Outcome)>()
        + size_of::<Result<(), Failure>>();
    assert_eq!(
        headers,
        execution_availability_consumption_headers_v1::<Output>().unwrap()
    );
    assert_eq!(
        size_of::<LeaseFields<'static>>(),
        size_of::<CompletedExecutionAvailabilityV1<'static>>()
    );
    let lease_headers = independent_lease_headers();
    let floor = 73;
    let prior = 11;
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let mut meter =
            CanonicalKernelIrWorkBudgetV1::new(prior + 12 + 8 - usize::from(short_work));
        let mut budget = ArgumentBudgetV1::new(
            &mut meter,
            floor + lease_headers + headers - usize::from(short_storage),
        );
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prior).unwrap();
        let mut lease = ExecutionAvailabilityLeaseV1::test_begin(None, &mut budget).unwrap();
        let result = lease.test_reserve_consumption::<Output>(&mut budget);
        assert_eq!(result.is_ok(), !short_work && !short_storage);
        assert_eq!(
            budget.peak_storage(),
            floor + lease_headers + if short_storage { 0 } else { headers }
        );
        assert_eq!(
            budget.failed_storage(),
            short_storage.then_some(floor + lease_headers + headers)
        );
        assert_eq!(
            budget.work(),
            prior + 12 + if short_work || short_storage { 0 } else { 8 }
        );
        lease.test_release(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(budget);
        assert_eq!(meter.failed_work(), short_work.then_some(prior + 12 + 8));
    }
}

#[test]
fn paired_cursor_stays_paid_until_discard_and_plain_output_can_escape() {
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        resource_tests::helper_closure_semantic_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut meter, LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let reached = std::cell::Cell::new(false);
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let child = instances.calls(instances.root()).unwrap()[0]
            .child()
            .unwrap();
        let floor = budget.storage();
        let held = OwnedExecutionAvailabilityV1::new(instances, child, None, None, budget)?;
        let retained = budget.storage();
        assert!(retained > floor);
        let held = Some(held);
        assert_eq!(budget.storage(), retained);
        held.unwrap().discard(budget)?;
        assert_eq!(budget.storage(), floor);
        let output = with_source_reference_availability_and_identity_v1(
            instances,
            child,
            None,
            None,
            budget,
            |cursor, _| {
                let result = (cursor.function_id.index(), cursor.source.root.index());
                drop(cursor);
                Ok(result)
            },
        )?;
        assert_eq!(
            output,
            (
                instances.instance(child).unwrap().function().index(),
                ROOT.index()
            )
        );
        assert_eq!(budget.storage(), floor);
        reached.set(true);
        Ok::<_, ProductionSemanticKirErrorV1>(())
    })
    .unwrap();
    assert!(
        reached.get(),
        "a caught refusal cannot satisfy the owning boundary test"
    );
}

#[test]
fn observation_seed_copy_preserves_original_buffer_and_exact_resource_charges() {
    let seeds = [None, None];
    let storage = 2 * size_of::<Option<SemanticExecutionBindingV29>>();
    let floor = 73;
    let prior = 11;
    // emission_vec pays three work, and copying these two fixed-size bindings
    // pays two. The surrounding observation/cursor owners retain their headers.
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(prior + 3 + 2 - usize::from(short_work));
        let mut budget =
            ArgumentBudgetV1::new(&mut meter, floor + storage - usize::from(short_storage));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prior).unwrap();
        let result = copy_execution_observation_seeds_v1(&seeds, &mut budget);
        assert_eq!(result.is_ok(), !short_work && !short_storage);
        if let Ok(copy) = &result {
            assert_eq!(copy.as_slice(), &seeds);
            assert_ne!(copy.as_ptr(), seeds.as_ptr());
        }
        assert_eq!(seeds, [None, None]);
        assert_eq!(
            budget.storage(),
            floor + if short_storage { 0 } else { storage }
        );
        assert_eq!(
            budget.peak_storage(),
            floor + if short_storage { 0 } else { storage }
        );
        assert_eq!(
            budget.failed_storage(),
            short_storage.then_some(floor + storage)
        );
        assert_eq!(
            budget.work(),
            prior + 3 + if short_work || short_storage { 0 } else { 2 }
        );
        drop(result);
        if !short_storage {
            budget.release_storage(storage).unwrap();
        }
        assert_eq!(budget.storage(), floor);
        drop(budget);
        assert_eq!(meter.failed_work(), short_work.then_some(prior + 3 + 2));
    }
}

#[test]
fn paid_cursor_lease_has_independent_inclusive_headers_and_prepaid_settlement_work() {
    let headers = independent_lease_headers();
    let floor = 73;
    let prior = 11;
    // No C2 root: initial, post-construction and final comparisons each prepay4.
    let work = 4 + 4 + 4;
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let mut meter = CanonicalKernelIrWorkBudgetV1::new(prior + work - usize::from(short_work));
        let mut budget =
            ArgumentBudgetV1::new(&mut meter, floor + headers - usize::from(short_storage));
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prior).unwrap();
        let lease = ExecutionAvailabilityLeaseV1::test_begin(None, &mut budget);
        assert_eq!(lease.is_ok(), !short_work && !short_storage);
        if let Ok(lease) = lease {
            assert_eq!(lease.test_fields(), (floor, floor + headers, headers));
            lease.test_release(&mut budget).unwrap();
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            budget.peak_storage(),
            floor + if short_storage { 0 } else { headers }
        );
        assert_eq!(
            budget.failed_storage(),
            short_storage.then_some(floor + headers)
        );
        assert_eq!(
            budget.work(),
            prior + if short_work || short_storage { 0 } else { work }
        );
        drop(budget);
        assert_eq!(meter.failed_work(), short_work.then_some(prior + work));
    }
}

#[test]
fn cursor_lease_keeps_prior_denials_and_extra_live_output_credit() {
    let headers = independent_lease_headers();
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut meter, 73 + headers + 13);
    budget.reserve_storage(73).unwrap();
    assert!(budget.charge_work(101).is_err());
    assert!(budget.reserve_storage(headers + 14).is_err());
    let denial = budget.failed_storage();
    let mut lease = ExecutionAvailabilityLeaseV1::test_begin(None, &mut budget).unwrap();
    lease.test_capture(&budget).unwrap();
    budget.reserve_storage(13).unwrap();
    lease.test_release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 73 + 13);
    assert_eq!(budget.failed_storage(), denial);
    assert_eq!(budget.work(), 12);
    drop(budget);
    assert_eq!(meter.failed_work(), Some(101));
}

#[test]
fn cursor_lease_observed_loss_is_sticky_after_counter_restore() {
    let mut meter = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut meter, LIMIT);
    budget.reserve_storage(73).unwrap();
    let mut lease = ExecutionAvailabilityLeaseV1::test_begin(None, &mut budget).unwrap();
    let before = budget.storage();
    budget.release_storage(1).unwrap();
    assert!(matches!(
        lease.test_capture(&budget),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    budget.reserve_storage(1).unwrap();
    assert!(!lease.test_permits_release(&budget));
    assert!(lease.test_release(&mut budget).is_err());
    assert_eq!(budget.storage(), before);
}

#[test]
fn cursor_lease_rejects_foreign_ledger_without_debit_or_refund() {
    let mut original_work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut original = ArgumentBudgetV1::new(&mut original_work, LIMIT);
    let lease = ExecutionAvailabilityLeaseV1::test_begin(None, &mut original).unwrap();
    let owned = original.storage();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(owned + 73).unwrap();
    let before = foreign.storage();
    assert!(lease.test_release(&mut foreign).is_err());
    assert_eq!(foreign.storage(), before);
    assert_eq!(foreign.work(), 0);
    assert_eq!(original.storage(), owned);
}

#[test]
fn original_cursor_callback_boundary_preserves_selected_failure_panic_and_paid_outputs() {
    for exit in 0..3 {
        let mut owner = ProductionSemanticSsaOwnerV1::try_new(
            resource_tests::helper_closure_semantic_owner(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        let capture = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let reached = std::cell::Cell::new(false);
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let floor = budget.storage();
            let child = instances.calls(instances.root()).unwrap()[0]
                .child()
                .unwrap();
            let result = with_source_reference_availability_and_identity_v1(
                instances,
                child,
                None,
                None,
                budget,
                |cursor, budget| {
                    assert_eq!(cursor.instance, child);
                    assert!(std::ptr::eq(
                        cursor.function,
                        instances.instance(child).unwrap().declaration()
                    ));
                    budget.reserve_storage(13)?;
                    drop(cursor);
                    reached.set(true);
                    match exit {
                        0 => Ok(()),
                        1 => Err(source_reference_error_v29("selected cursor callback error")),
                        2 => std::panic::panic_any(1164usize),
                        _ => unreachable!(),
                    }
                },
            );
            assert!(reached.get());
            match (exit, result) {
                (0, Ok(())) => (),
                (
                    1,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "selected cursor callback error",
                        ..
                    }),
                ) => (),
                (
                    2,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source reference availability construction or callback panicked",
                        ..
                    }),
                ) => (),
                (_, other) => panic!("selected boundary outcome changed: {other:?}"),
            }
            assert_eq!(budget.storage(), floor + 13);
            budget.release_storage(13)?;
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        })
        .unwrap();
        assert!(
            reached.get(),
            "a caught setup refusal is not callback coverage"
        );
        assert_eq!(budget.storage(), capture.retained_storage());
    }
}

#[test]
fn actual_cursor_owner_retains_c2_growth_and_denies_cushioned_loss_before_refund() {
    for growth in [false, true] {
        for lose in [false, true] {
            for ignore in [false, true] {
                let mut owner = ProductionSemanticSsaOwnerV1::try_new(
                    resource_tests::helper_closure_semantic_owner(),
                    ProductionSemanticSsaLimitsV1::default(),
                )
                .unwrap();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR).unwrap();
                let capture = owner
                    .try_capture_occurrences_with_budget_v1(&mut budget)
                    .unwrap();
                budget.reserve_storage(capture.retained_storage()).unwrap();
                let unit = owner.source_semantic().functions()[1].locals()[0].ty();
                let mut layouts =
                    source_storage_v29::SourceStorageLayoutsV29::new(&owner, &[unit], &mut budget)
                        .unwrap();
                let reached = std::cell::Cell::new(false);
                with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
                    let table_floor = budget.storage();
                    let result = source_storage_v29::with_source_storage_root_v29(&mut layouts, instances, budget, |references, root, budget| {
                        let frame_floor = budget.storage();
                        let emission = source_reference_optional_emission_v29(Some(references), budget)?.unwrap();
                        let emission_storage = budget.storage() - frame_floor;
                        let child = instances.calls(instances.root()).unwrap()[0].child().unwrap();
                        let owner_floor = budget.storage();
                        let owned = OwnedExecutionAvailabilityV1::new(instances, child, Some(&emission), None, budget)?;
                        type GrowthResult<'a> = Result<usize, source_storage_v29::SourceStorageRootCallbackErrorV29<'a>>;
                        let required = budget.storage() + execution_availability_consumption_headers_v1::<GrowthResult<'_>>()?;
                        let credit = required - owner_floor;
                        let (completed, outcome) = owned.consume(budget, |cursor, budget| {
                            let growth = (|| -> GrowthResult<'_> {
                            assert_eq!(cursor.instance, child);
                            assert!(std::ptr::eq(cursor.function, instances.instance(child).unwrap().declaration()));
                            assert!(std::ptr::eq(cursor.references.unwrap(), &emission));
                            let growth_start = budget.storage();
                            if growth {
                                let state = root.new_state(child, SemanticLocalIdV1::from_index(0), budget)?;
                                let path = root.root_path(unit, budget)?;
                                root.mutate(state, path, source_storage_v29::SourceStorageRootMutationV29::Initialize, budget)?;
                                let copied = root.copy_state(state, budget)?;
                                assert!(root.is_initialized(copied, path, budget)?);
                            }
                            let grown = budget.storage() - growth_start;
                            assert_eq!(grown > 0, growth);
                            if lose {
                                budget.release_storage(1)?;
                                if growth { assert!(budget.storage() >= required, "old absolute receipt floor has unrelated cushion"); }
                            }
                            drop(cursor);
                            Ok(grown)
                            })();
                            Ok(growth)
                        });
                        let grown = outcome.expect("real cursor consumer must not panic")??;
                        let before = budget.storage();
                        let release = completed.release(budget);
                        assert_eq!(release.is_err(), lose);
                        if lose {
                            assert_eq!(budget.storage(), before);
                            assert!(!root.permits_cleanup_refund(instances, &references.failure, 0, budget));
                            assert!(references.failure.first_error().is_none());
                            budget.reserve_storage(1)?;
                            assert!(!root.permits_cleanup_refund(instances, &references.failure, 0, budget));
                        } else {
                            assert_eq!(budget.storage(), before - credit);
                            drop(emission);
                            assert!(root.permits_cleanup_refund(instances, &references.failure, emission_storage, budget));
                            budget.release_storage(emission_storage)?;
                            assert_eq!(budget.storage(), frame_floor + grown);
                        }
                        reached.set(true);
                        if lose && !ignore {
                            Err(source_reference_error_v29("cursor owner selected source failure").into())
                        } else { Ok(()) }
                    });
                    assert!(reached.get(), "caught construction refusal cannot satisfy cleanup coverage");
                    match result {
                        Ok(()) => assert!(!lose),
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)) => assert!(lose && ignore),
                        Err(ProductionSemanticKirErrorV1::Unsupported { detail: "cursor owner selected source failure", .. }) => assert!(lose && !ignore),
                        other => panic!("unexpected root settlement: {other:?}"),
                    }
                    if !lose { assert_eq!(budget.storage(), table_floor); }
                    Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
                }).unwrap();
                assert!(reached.get());
                assert_eq!(layouts.release(&mut budget).is_err(), lose);
                drop(owner);
                budget.release_storage(budget.storage() - FLOOR).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
}
