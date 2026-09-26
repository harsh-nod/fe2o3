//! Real collected Rust uses the same unboxed source/optimizer/account route.
use super::*;
use crate::production_pipeline::ProductionPipelineError;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

const OPTIMIZED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::optimized_source_v18_tests::optimized_source_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct OptimizedObservation {
    original_digest: [u8; 32],
    output_digest: [u8; 32],
    roots: usize,
    work: usize,
    retained: usize,
    high_water: usize,
    zero_work_first_denial: usize,
    zero_storage_first_denial: usize,
    selected_error_retained: bool,
    owned_error_retained: [bool; 2],
    constant_locals: usize,
    capability_collection_keys: usize,
    capability_blocks: usize,
    typed_capability_entries: usize,
    typed_capabilities_together: bool,
    swallowed_driver_refusal_blocked: bool,
    driver_refusal_first_work: usize,
    driver_small_charge_accepted: bool,
    constant_consumer_error_retained: bool,
    partial_phase_refusals: [bool; 2],
    phase_work_prefix: [usize; 2],
    phase_custody_work: [usize; 2],
    phase_storage: [usize; 2],
    invocation_count: usize,
    repeated_callee_distinct_callers: bool,
    invocation_refusal_callbacks: [usize; 2],
    invocation_substitutions_refused: [bool; 7],
    foreign_invocation_owner_refused: bool,
    inactive_invocations: usize,
    inactive_meter_refusals: [bool; 2],
}

#[derive(Default)]
struct OptimizedCallbacks {
    result: Option<Result<OptimizedObservation, String>>,
}

impl Callbacks for OptimizedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            // Twenty unconditional transactions and two pairs of optional
            // inactive-invocation/typed-capability refusal probes.
            let mut transactions = transactions_with_original_sources_for_test_v1::<24>(tcx)?;
            let transaction_count = std::cell::Cell::new(0usize);
            let mut transaction = || {
                let ordinal = transaction_count.get() + 1;
                transaction_count.set(ordinal);
                transactions.next().ok_or_else(|| format!("source transaction {ordinal}: source-capture batch exhausted"))?
                .map_err(|error| format!("source transaction {ordinal}: {error}"))
            };
            let work_limit =
                usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                    .map_err(|_| "optimizer test work limit conversion")?;
            let storage_limit = crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
            let mut account = OwnedBudget::new(Work::new(work_limit), storage_limit);
            let (output, (original_digest, output_digest, roots, constant_locals, capability_collection_keys, capability_blocks, typed_capability_entries, typed_capabilities_together, expected_invocations), receipt) = transaction()?
                .inspect_optimized_source_v18(&mut account, |original, optimized, budget| {
                    let input = optimized.input_inventory(budget)?;
                    let output = optimized.output_inventory(budget)?;
                    assert!(std::ptr::eq(input, original.inventory(budget)?));
                    assert!(!std::ptr::eq(input, output));
                    let source = original.source(budget)?;
                    assert!(std::ptr::eq(source, optimized.original_source(budget)?));
                    let roots = source.root_count(budget)?;
                    let mut expected_invocations = 0usize;
                    for root in 0..roots { expected_invocations += source.instance_count(root, budget)?; }
                    assert_eq!(roots, output.owner().module().kernels.len());
                    let constant_locals = crate::production_ranked_projection_v1::inspect_actual_source_constants_for_test_v18(
                        original, optimized, budget,
                    ).map_err(ProductionPipelineError::RankedProjection)?;
                    assert!(constant_locals > 0);
                    let capability_collection_keys = crate::production_ranked_projection_v1::inspect_actual_source_capability_collections_for_test_v18(
                        original, optimized, budget,
                    ).map_err(ProductionPipelineError::RankedProjection)?;
                    assert!(capability_collection_keys > 0);
                    let capability_blocks = crate::production_ranked_projection_v1::inspect_actual_source_capabilities_for_test_v18(
                        original, optimized, budget,
                    ).map_err(ProductionPipelineError::RankedProjection)?;
                    assert!(capability_blocks > 0);
                    let (typed_capability_entries, typed_capabilities_together) = crate::production_ranked_projection_v1::inspect_actual_source_typed_capabilities_for_test_v18(
                        original, optimized, budget,
                    ).map_err(ProductionPipelineError::RankedProjection)?;
                    let observation = (
                        *input.owner().identity().digest(),
                        *output.owner().identity().digest(),
                        roots,
                        constant_locals,
                        capability_collection_keys,
                        capability_blocks,
                        typed_capability_entries,
                        typed_capabilities_together,
                        expected_invocations,
                    );
                    Ok((observation, std::mem::size_of_val(&observation)))
                })
                .map_err(|error| format!("actual owned source optimizer: {error:?}"))?;
            let recapture = crate::collector::capture_context_producers_v1(tcx).err()
                .expect("consumed original MIR must not be replaced with optimized MIR");
            assert!(recapture.to_string().contains("pre-optimization context producer MIR is already unavailable"));
            let retained = output.storage().retained_storage() + receipt.retained_storage();
            assert!(account.storage() >= retained);
            assert_eq!(output.owner().module().kernels.len(), roots);
            assert_eq!(output.owner().identity().digest(), &output_digest);
            assert!(!output.grants_authority() && !receipt.grants_authority());
            let (work, high_water) = (account.work(), account.peak_storage());
            assert!(work > 0 && high_water >= account.storage());
            assert_eq!(
                (account.failed_work(), account.failed_storage()),
                (None, None)
            );
            drop(output);
            account
                .with_budget(|budget| budget.release_storage(retained))
                .unwrap();
            assert_eq!((account.work(), account.peak_storage()), (work, high_water));

            let mut invocation_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
            // Observation rows are test bookkeeping, not source-owned evidence.
            let mut invocations = Vec::new();
            let mut invocation_output = None;
            let mut inactive_invocations = 0usize;
            let (owner, invocation_count, origin) = transaction()?
                .inspect_optimized_source_invocations_v18(&mut invocation_account, |view, meter| {
                    view.check_effect_rows_v18(meter)?;
                    assert_eq!(view.blocks(), view.original_function().blocks().len());
                    assert!(view.blocks() > 0);
                    let actual = std::ptr::from_ref(view.output()) as usize;
                    assert_eq!(*invocation_output.get_or_insert(actual), actual);
                    // Both endpoint decisions remain separate, including
                    // Incomplete. None of these observations is a purity proof.
                    assert_eq!(view.decisions().is_some(), view.disposition()
                        == crate::production_ranked_projection_v1::SourceInvocationDispositionV18::Active);
                    match view.arguments() {
                        crate::production_ranked_projection_v1::SourceInvocationArgumentsV18::Root =>
                            assert!(view.caller().is_none()),
                        crate::production_ranked_projection_v1::SourceInvocationArgumentsV18::Call {
                            caller, block, operands, tail,
                        } => {
                            assert_eq!(view.caller().unwrap().1, block);
                            let (actual, is_tail) = match caller.blocks()[block.index() as usize].terminator().kind() {
                                fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1::Call(call) =>
                                    (call.arguments(), false),
                                fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1::TailCall(call) =>
                                    (call.arguments(), true),
                                _ => panic!("invocation arguments are not from its exact caller"),
                            };
                            assert!(std::ptr::eq(actual, operands));
                            assert_eq!(tail, is_tail);
                        }
                    }
                    if view.disposition() == crate::production_ranked_projection_v1::SourceInvocationDispositionV18::OriginalInactive {
                        inactive_invocations += 1;
                        assert_eq!(view.fact_presence_for_test_v18(), [false; 4]);
                        assert!(view.visit_output_cfg_v18(meter, |_, _| {
                            panic!("inactive invocation was replaced by an empty output CFG")
                        }).is_err());
                    } else {
                        assert_eq!(view.fact_presence_for_test_v18(), [true; 4]);
                        if view.caller().is_none() {
                            let mut block = None;
                            let mut blocks = 0;
                            let mut terminated = 0;
                            view.visit_output_cfg_v18(meter, |event, _| {
                                use fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgEventV18 as Event;
                                match event {
                                    Event::Block { actual, segments } => {
                                        assert!(block.replace(actual.coordinate).is_none());
                                        assert!(!segments.is_empty());
                                        blocks += 1;
                                    }
                                    Event::Operation { actual, .. } =>
                                        assert_eq!(block, Some(actual.coordinate.block)),
                                    Event::Terminator { actual, edges, edge_origins, .. } => {
                                        assert_eq!(block.take(), Some(actual.coordinate));
                                        assert_eq!(edges.len(), edge_origins.len());
                                        terminated += 1;
                                    }
                                }
                                Ok(())
                            })?;
                            assert!(blocks > 0);
                            assert_eq!(blocks, terminated);
                            assert!(block.is_none());
                        }
                    }
                    let key = (view.root(), view.instance(), view.function().index(),
                        view.caller().map(|(caller, block)| (caller, block.index())));
                    assert!(!invocations.iter().any(|prior: &(usize, usize, u32, Option<(usize, u32)>)|
                        prior.0 == key.0 && prior.1 == key.1));
                    invocations.push(key);
                    Ok(0)
                }).map_err(|error| format!("exact invocation projection: {error:?}"))?;
            assert_eq!(invocation_count, invocations.len());
            assert_eq!(invocation_count, expected_invocations, "an original invocation silently disappeared");
            assert!(invocation_count > 0);
            let repeated_callee_distinct_callers = invocations.iter().enumerate().any(|(index, left)|
                invocations[index + 1..].iter().any(|right|
                    left.0 == right.0 && left.2 == right.2 && left.1 != right.1
                        && left.3.is_some() && right.3.is_some() && left.3 != right.3));
            assert!(!owner.grants_authority() && !origin.grants_authority());
            assert_eq!((invocation_account.failed_work(), invocation_account.failed_storage()), (None, None));
            drop((owner, origin));

            let mut inactive_meter_refusals = [false; 2];
            if inactive_invocations > 0 {
                for storage in [false, true] {
                    let ordinal = usize::from(storage);
                    let mut refused = OwnedBudget::new(Work::new(work_limit), storage_limit);
                    let mut injected = false;
                    let mut hits = 0;
                    let before = crate::production_ranked_projection_v1::invocation_resource_counts_for_test_v18();
                    let result = transaction()?.inspect_optimized_source_invocations_v18(&mut refused, |view, meter| {
                        assert!(!injected, "callback ran after a swallowed inactive-meter refusal");
                        view.check_effect_rows_v18(meter)?;
                        if view.disposition() == crate::production_ranked_projection_v1::SourceInvocationDispositionV18::OriginalInactive {
                            assert_eq!(view.fact_presence_for_test_v18(), [false; 4]);
                            hits += 1;
                            let error = if storage { meter.reserve_storage(usize::MAX) }
                                else { meter.charge_work(usize::MAX) };
                            assert!(error.is_err());
                            injected = true;
                        }
                        Ok(0)
                    });
                    assert_eq!(hits, 1);
                    let after = crate::production_ranked_projection_v1::invocation_resource_counts_for_test_v18();
                    assert_eq!(after.0 - before.0, after.1 - before.1);
                    inactive_meter_refusals[ordinal] = match result {
                        Err(ProductionPipelineError::SourceOwnedEntrance(SourceError::Resource(ResourceError::Storage(error))))
                            if storage => { assert_eq!(error.actual(), usize::MAX); true }
                        Err(ProductionPipelineError::SourceOwnedEntrance(SourceError::Resource(ResourceError::Work(error))))
                            if !storage => { assert_eq!(error.actual(), usize::MAX); true }
                        _ => false,
                    };
                    assert!(inactive_meter_refusals[ordinal], "inactive resource refusal allowed output or changed first error");
                    if storage { assert_eq!(refused.failed_storage(), Some(usize::MAX)); assert_eq!(refused.failed_work(), None); }
                    else { assert_eq!(refused.failed_work(), Some(usize::MAX)); assert_eq!(refused.failed_storage(), None); }
                    assert!(refused.with_budget(|budget| budget.charge_work(1)).is_ok());
                }
            }

            let mut invocation_refusal_callbacks = [0usize; 2];
            for storage in [false, true] {
                let ordinal = usize::from(storage);
                let mut refused = OwnedBudget::new(Work::new(work_limit), storage_limit);
                let before = crate::production_ranked_projection_v1::invocation_resource_counts_for_test_v18();
                let result = transaction()?.inspect_optimized_source_invocations_v18(&mut refused, |view, meter| {
                    if view.disposition() == crate::production_ranked_projection_v1::SourceInvocationDispositionV18::OriginalInactive {
                        view.check_effect_rows_v18(meter)?;
                        return Ok(0);
                    }
                    invocation_refusal_callbacks[ordinal] += 1;
                    view.check_effect_rows_v18(meter)?;
                    let error = if storage { meter.reserve_storage(usize::MAX) }
                        else { meter.charge_work(usize::MAX) };
                    assert!(error.is_err());
                    // The owned facts meter has recorded this actual refusal.
                    // Swallowing it must not allow this or a later invocation out.
                    Ok(0)
                });
                assert_eq!(invocation_refusal_callbacks[ordinal], 1);
                let after = crate::production_ranked_projection_v1::invocation_resource_counts_for_test_v18();
                assert_eq!((after.0 - before.0, after.1 - before.1), (1, 1),
                    "refused invocation did not drop its private effect rows");
                assert!(matches!(result, Err(ProductionPipelineError::SourceOwnedEntrance(
                    SourceError::Resource(_)))));
                if storage {
                    assert!(refused.failed_storage().is_some());
                    assert_eq!(refused.failed_work(), None);
                } else {
                    assert_eq!(refused.failed_work(), Some(usize::MAX));
                    assert_eq!(refused.failed_storage(), None);
                }
                assert!(refused.with_budget(|budget| budget.charge_work(1)).is_ok());
            }

            let mut invocation_substitutions_refused = [false; 7];
            for mode in 0..7u8 {
                let mut hostile = OwnedBudget::new(Work::new(work_limit), storage_limit);
                let result = transaction()?.inspect_optimized_source_v18(&mut hostile, |original, _, budget| {
                    let error = crate::production_ranked_projection_v1::refuse_invocation_substitution_for_test_v18(
                        original, budget, mode,
                    ).expect_err("substituted invocation reached the projection continuation");
                    Err::<((), usize), _>(ProductionPipelineError::RankedProjection(error))
                });
                invocation_substitutions_refused[usize::from(mode)] = result.is_err();
                assert!(invocation_substitutions_refused[usize::from(mode)]);
                // These are identity/custody rejections, not quota exhaustion.
                assert_eq!((hostile.failed_work(), hostile.failed_storage()), (None, None));
            }
            let foreign_transaction = transaction()?;
            let mut foreign_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
            let mut original_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
            let foreign_result = transaction()?.inspect_optimized_source_v18(&mut original_account,
                |original, _, original_budget| {
                    let result = foreign_transaction.inspect_optimized_source_v18(&mut foreign_account,
                        |foreign, _, foreign_budget| {
                            let error = crate::production_ranked_projection_v1::refuse_foreign_invocation_owner_for_test_v18(
                                original, original_budget, foreign, foreign_budget,
                            ).expect_err("same-byte foreign source owner reached invocation projection");
                            Err::<((), usize), _>(ProductionPipelineError::RankedProjection(error))
                        });
                    match result {
                        Err(error) => Err::<((), usize), _>(error),
                        Ok(_) => panic!("foreign invocation returned owned output"),
                    }
                });
            let foreign_invocation_owner_refused = foreign_result.is_err();
            assert!(foreign_invocation_owner_refused);
            assert_eq!((original_account.failed_work(), original_account.failed_storage()), (None, None));
            assert_eq!((foreign_account.failed_work(), foreign_account.failed_storage()), (None, None));

            let called = std::cell::Cell::new(false);
            let mut no_work = OwnedBudget::new(Work::new(0), storage_limit);
            let denied = transaction()?.inspect_optimized_source_v18(&mut no_work, |_, _, _| {
                called.set(true);
                Ok(((), 0))
            });
            let error = match denied {
                Err(ProductionPipelineError::ContextHandoff(
                    ProductionContextRootErrorV29::Resource(ResourceError::Work(error)),
                )) => error,
                _ => {
                    return Err(
                        "zero-work original context refusal changed or output escaped".into(),
                    );
                }
            };
            assert!(!called.get());
            assert_eq!(no_work.work(), 0);
            assert_eq!(error.limit(), 0);
            assert_eq!(no_work.failed_work(), Some(error.actual()));
            let zero_work_first_denial = error.actual();

            let mut no_storage = OwnedBudget::new(Work::new(work_limit), 0);
            let denied = transaction()?.inspect_optimized_source_v18(&mut no_storage, |_, _, _| {
                called.set(true);
                Ok(((), 0))
            });
            let error = match denied {
                Err(ProductionPipelineError::SourceOwnedEntrance(SourceError::Resource(
                    ResourceError::Storage(error),
                ))) => error,
                _ => {
                    return Err(
                        "zero-storage original header refusal changed or output escaped".into(),
                    );
                }
            };
            assert!(!called.get());
            assert_eq!(no_storage.work(), 0);
            assert_eq!((no_storage.storage(), no_storage.peak_storage()), (0, 0));
            assert_eq!(error.limit(), 0);
            assert_eq!(no_storage.failed_storage(), Some(error.actual()));
            let zero_storage_first_denial = error.actual();

            let mut selected_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
            let selected =
                transaction()?.inspect_optimized_source_v18(&mut selected_account, |_, _, _| {
                    called.set(true);
                    Err::<((), usize), _>(ProductionPipelineError::RustcLineageMismatch)
                });
            assert!(called.get());
            let selected_error_retained =
                matches!(selected, Err(ProductionPipelineError::RustcLineageMismatch));
            assert!(selected_error_retained);
            assert_eq!(
                (
                    selected_account.failed_work(),
                    selected_account.failed_storage()
                ),
                (None, None)
            );
            let mut owned_error_retained = [false; 2];
            for first_refusal in [false, true] {
                let mut owned_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
                let reached = std::cell::Cell::new(false);
                let backing = std::mem::size_of::<fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionErrorV1>();
                let result = transaction()?.inspect_optimized_source_v18(&mut owned_account, |original, optimized, budget| {
                    assert!(std::ptr::eq(original.inventory(budget)?, optimized.input_inventory(budget)?));
                    if first_refusal {
                        let error = budget.charge_work(usize::MAX).unwrap_err();
                        let _ = original.retain_query_resource_error_v18(error);
                    }
                    budget.reserve_storage(backing).map_err(|error|
                        ProductionPipelineError::SourceOwnedEntrance(original.retain_query_resource_error_v18(error)))?;
                    let error = Box::new(fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionErrorV1::Mismatch(
                        "selected owned error through actual backend chain"));
                    reached.set(true);
                    Err::<((), usize), _>(ProductionPipelineError::ScalarEmissionCapture(error))
                });
                assert!(reached.get());
                // The bounded adoption contract retains its conservative
                // residual, not only a newly invented error-transfer receipt.
                assert!(owned_account.storage() >= selected_account.storage() + backing);
                owned_error_retained[usize::from(first_refusal)] = match (first_refusal, result) {
                    (false, Err(ProductionPipelineError::ScalarEmissionCapture(error))) => {
                        assert!(matches!(*error, fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionErrorV1::Mismatch(
                            "selected owned error through actual backend chain")));
                        drop(error);
                        true
                    }
                    (true, Err(ProductionPipelineError::SourceOwnedEntrance(SourceError::Resource(
                        ResourceError::Work(error))))) => { assert_eq!(error.actual(), usize::MAX); true }
                    _ => false,
                };
                assert!(owned_error_retained[usize::from(first_refusal)]);
                assert_eq!(owned_account.failed_work(), first_refusal.then_some(usize::MAX));
                assert_eq!(owned_account.failed_storage(), None);
            }
            let first_driver_refusal = std::cell::Cell::new(None);
            let small_charge_accepted = std::cell::Cell::new(false);
            let mut swallowed_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
            let swallowed = transaction()?.inspect_optimized_source_v18(&mut swallowed_account, |original, optimized, budget| {
                let result = crate::production_ranked_projection_v1::refuse_actual_source_capabilities_for_test_v18(
                    original, optimized, budget);
                let error = result.err().expect("the actual live driver lost its injected first resource refusal");
                assert_eq!(crate::production_ranked_projection_v1::capability_work_refusal_for_test_v18(&error), Some(usize::MAX));
                first_driver_refusal.set(budget.failed_work());
                // Raw Budget is not sticky. The source-owner boundary, not a
                // naturally exhausted quota, must prevent a successful escape.
                small_charge_accepted.set(budget.charge_work(1).is_ok());
                Ok(((), 0))
            });
            let swallowed_driver_refusal_blocked = swallowed.is_err();
            assert!(swallowed_driver_refusal_blocked, "an actual source owner published after a swallowed driver resource refusal");
            let driver_refusal_first_work = first_driver_refusal.get().expect("actual driver callback did not refuse");
            let driver_small_charge_accepted = small_charge_accepted.get();
            assert!(driver_small_charge_accepted);
            assert_eq!(swallowed_account.failed_work(), Some(driver_refusal_first_work));
            let mut chronology_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
            let chronology = transaction()?.inspect_optimized_source_v18(&mut chronology_account, |original, optimized, budget| {
                crate::production_ranked_projection_v1::fail_actual_source_constants_after_custody_for_test_v18(
                    original, optimized, budget, work_limit).map(|_| ((), 0)).map_err(ProductionPipelineError::RankedProjection)
            });
            let constant_consumer_error_retained = matches!(chronology, Err(ProductionPipelineError::RankedProjection(
                crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Unsupported(
                    "selected constant consumer before custody postflight"))));
            assert!(constant_consumer_error_retained, "constant custody or exhausted postflight replaced the earlier consumer error");
            assert_eq!(chronology_account.work(), work_limit);
            let mut partial_phase_refusals = [false; 2];
            let mut phase_work_prefix = [0; 2];
            let mut phase_custody_work = [0; 2];
            let mut phase_storage = [0; 2];
            if typed_capabilities_together {
                for (index, normalize) in [true, false].into_iter().enumerate() {
                    let observed = std::cell::Cell::new(None);
                    let raw_small_charge = std::cell::Cell::new(false);
                    let mut phase_account = OwnedBudget::new(Work::new(work_limit), storage_limit);
                    let refused = transaction()?.inspect_optimized_source_v18(&mut phase_account, |original, optimized, budget| {
                        let (error, phase) = crate::production_ranked_projection_v1::refuse_actual_source_capability_phase_for_test_v18(
                            original, optimized, budget, normalize);
                        assert_eq!(crate::production_ranked_projection_v1::capability_work_refusal_for_test_v18(&error), Some(usize::MAX));
                        assert!(phase.injected && phase.created > 0 && phase.created == phase.dropped);
                        assert!(phase.requested_work > 0);
                        assert_eq!(phase.first_failed_work, Some(usize::MAX));
                        assert_eq!(phase.first_failed_storage, None);
                        assert_eq!(phase.storage_before, phase.storage_after);
                        // Genuine facts custody checks precede the oversized raw
                        // debit. Their accepted work remains on the same ledger.
                        assert!(phase.work_after > phase.work_before);
                        observed.set(Some(phase));
                        raw_small_charge.set(budget.charge_work(1).is_ok());
                        Ok(((), 0))
                    });
                    assert!(raw_small_charge.get(), "raw Budget became sticky after a partial phase refusal");
                    assert!(matches!(refused, Err(ProductionPipelineError::SourceOwnedEntrance(SourceError::Resource(
                        ResourceError::Work(error)))) if error.actual() == usize::MAX),
                        "the source owner published or replaced its first partial-phase resource failure");
                    let phase = observed.get().expect("phase fixture callback did not run");
                    assert_eq!(phase_account.failed_work(), Some(usize::MAX));
                    assert_eq!(phase_account.failed_storage(), None);
                    assert!(phase_account.work() >= phase.work_after + 1);
                    partial_phase_refusals[index] = true;
                    phase_work_prefix[index] = phase.work_before;
                    phase_custody_work[index] = phase.work_after - phase.work_before;
                    phase_storage[index] = phase.storage_before;
                }
            }
            // Two positive entrances, active refusals, substitutions, two
            // foreign-owner entrances, two zero-quota probes, selected error,
            // owned errors, swallowed refusal, and error chronology.
            let unconditional_transactions = 2 + invocation_refusal_callbacks.len()
                + invocation_substitutions_refused.len() + 2 + 2 + 1
                + owned_error_retained.len() + 1 + 1;
            assert_eq!(unconditional_transactions, 20);
            assert_eq!(transaction_count.get(), unconditional_transactions
                + inactive_meter_refusals.len() * usize::from(inactive_invocations > 0)
                + partial_phase_refusals.len() * usize::from(typed_capabilities_together));
            Ok(OptimizedObservation {
                original_digest,
                output_digest,
                roots,
                work,
                retained,
                high_water,
                zero_work_first_denial,
                zero_storage_first_denial,
                selected_error_retained,
                owned_error_retained,
                constant_locals,
                capability_collection_keys,
                capability_blocks,
                typed_capability_entries,
                typed_capabilities_together,
                swallowed_driver_refusal_blocked,
                driver_refusal_first_work,
                driver_small_charge_accepted,
                constant_consumer_error_retained,
                partial_phase_refusals,
                phase_work_prefix,
                phase_custody_work,
                phase_storage,
                invocation_count,
                repeated_callee_distinct_callers,
                invocation_refusal_callbacks,
                invocation_substitutions_refused,
                foreign_invocation_owner_refused,
                inactive_invocations,
                inactive_meter_refusals,
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires the actual-source request from its parent"]
fn optimized_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = OptimizedCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual optimized-source callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("optimized-source report path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual optimized source: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_rust_source_optimizer_preserves_retained_output_and_exact_refusals() {
    run_actual_sources::<OptimizedObservation>(
        &[
            ("plain", "let value = seed;"),
            ("workgroup", "let value = ctx.with_workgroup(|_wg| 7_u32);"),
            ("matrix_context", "let _matrix = fe2o3_device::tensor::DeviceMatrix::current(); let value = seed;"),
            ("wave_lane", "let _lane = fe2o3_device::wave::WaveLane::<fe2o3_device::wave::Wave64>::current(); let value = seed;"),
            ("typed_pair", "let _matrix = fe2o3_device::tensor::DeviceMatrix::current(); let _lane = fe2o3_device::wave::WaveLane::<fe2o3_device::wave::Wave64>::current(); let value = seed;"),
            ("repeated_callers", "#[inline(never)] fn shared(value: u32) -> u32 { value.wrapping_add(1) } #[inline(never)] fn left(value: u32) -> u32 { shared(value) } #[inline(never)] fn right(value: u32) -> u32 { shared(value) } let value = left(seed).wrapping_add(right(seed));"),
            ("inactive_after_no_return", "#[inline(never)] fn after(value: u32) -> u32 { value.wrapping_add(1) } let first = stop(seed); let value = after(first);"),
            ("plain", "let value = seed;"),
        ],
        &[(0, 0), (3, 2)],
        OPTIMIZED_CHILD,
        "SOURCE_OPTIMIZED_V18_CONTINUING_ACCOUNT",
        |body| {
            // Keep the genuinely divergent helper outside the kernel's direct
            // control-flow contract; no finite iteration bound describes it.
            format!("{}\n#[inline(never)]\nfn stop(value: u32) -> u32 {{ let _ = value; loop {{}} }}\n", source(body))
        },
        |_, _, label, observation, previous| {
            assert_eq!(observation.roots, 1);
            assert!(observation.work > 0 && observation.retained > 0);
            assert!(observation.high_water >= observation.retained);
            assert!(observation.zero_work_first_denial > 0);
            assert!(observation.zero_storage_first_denial > 0);
            assert!(observation.selected_error_retained);
            assert_eq!(observation.owned_error_retained, [true; 2]);
            assert!(observation.constant_locals > 0);
            assert!(observation.capability_collection_keys > 0);
            assert!(observation.capability_blocks > 0);
            if matches!(label, "matrix_context" | "wave_lane" | "typed_pair") {
                assert!(observation.typed_capability_entries > 0, "typed source producer never entered the live capability state");
            } else {
                assert_eq!(observation.typed_capability_entries, 0);
            }
            assert!(observation.swallowed_driver_refusal_blocked);
            assert_eq!(observation.driver_refusal_first_work, usize::MAX);
            assert!(observation.driver_small_charge_accepted);
            assert!(observation.constant_consumer_error_retained);
            assert!(observation.invocation_count > 0);
            assert_eq!(observation.invocation_refusal_callbacks, [1, 1]);
            assert_eq!(observation.invocation_substitutions_refused, [true; 7]);
            assert!(observation.foreign_invocation_owner_refused);
            if label == "inactive_after_no_return" {
                assert!(observation.inactive_invocations > 0,
                    "actual no-normal-return helper did not preserve an inactive original invocation");
                assert_eq!(observation.inactive_meter_refusals, [true; 2]);
            }
            if label == "repeated_callers" {
                assert!(observation.repeated_callee_distinct_callers,
                    "fixture did not retain the same callee under distinct caller instances");
            }
            if label == "typed_pair" {
                assert!(observation.typed_capabilities_together, "source fixture lacks a reachable two-capability state");
                assert_eq!(observation.partial_phase_refusals, [true; 2]);
                assert!(observation.phase_work_prefix.into_iter().all(|work| work > 0));
                assert!(observation.phase_custody_work.into_iter().all(|work| work > 0));
                assert!(observation.phase_storage.into_iter().all(|bytes| bytes > 0));
            } else { assert_eq!(observation.partial_phase_refusals, [false; 2]); }
            if let Some(old) = previous.get(label) {
                assert_eq!(old, &observation);
            } else {
                previous.insert(label.to_owned(), observation);
            }
        },
    );
}
