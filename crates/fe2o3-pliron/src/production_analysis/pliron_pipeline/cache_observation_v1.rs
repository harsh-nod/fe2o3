use super::invocation_receipt_v1::InvocationObserverV1;
use crate::production_analysis::{
    pliron_analysis_manager::{
        PlironMemoryOrderAnalysisFailureV1 as MemoryCacheFailure,
        PlironSimtProtocolAnalysisFailureV1 as SimtCacheFailure,
    },
    pliron_function_inventory::BoundedPlironFunctionInventoryFailureV1 as InventoryFailure,
    pliron_invocation_trace::PlironTraceFailureV1 as TraceFailure,
    pliron_memory_order::PlironMemoryOrderFailureV1 as MemoryFailure,
    pliron_provenance_alias::PlironProvenanceFailureV1 as ProvenanceFailure,
    pliron_resource_envelope::{
        ProductionAnalysisResourceLimitV1 as Limit, ProductionAnalysisResourcePhaseV1 as Phase,
    },
    pliron_simt_protocol::{
        PlironSimtProtocolAnalysisV1 as SimtAnalysis, PlironSimtProtocolIssueV1 as SimtIssue,
    },
    pliron_sparse_index::SparseIndexFailureV1 as SparseFailure,
};

fn deny(observer: &InvocationObserverV1<'_, '_>, phase: Phase, resource: &'static str) {
    observer.deny(Limit { phase, resource });
}

pub(super) fn observe_inventory_failure_v1(
    observer: &InvocationObserverV1<'_, '_>,
    failure: &InventoryFailure,
) {
    deny(observer, Phase::FunctionInventory, failure.resource());
}

pub(super) fn observe_sparse_failure_v1(
    observer: &InvocationObserverV1<'_, '_>,
    failure: &SparseFailure,
) {
    if let SparseFailure::ResourceLimit { resource, .. } = failure {
        deny(observer, Phase::SparseIndex, resource);
    }
}

pub(super) fn observe_trace_failure_v1(
    observer: &InvocationObserverV1<'_, '_>,
    failure: &TraceFailure,
) {
    match failure {
        TraceFailure::ResourceLimit => deny(
            observer,
            Phase::InvocationTrace,
            "invocation trace resource limit",
        ),
        TraceFailure::LaunchTooLarge { .. } => deny(
            observer,
            Phase::InvocationTrace,
            "invocation trace launch limit",
        ),
        TraceFailure::Sparse(failure) => observe_sparse_failure_v1(observer, failure),
        _ => {}
    }
}

pub(super) fn observe_layout_failure_v1(
    observer: &InvocationObserverV1<'_, '_>,
    failure: &TraceFailure,
) {
    if matches!(failure, TraceFailure::ResourceLimit) {
        deny(observer, Phase::LaunchContract, "execution-layout analysis");
    }
}

pub(super) fn observe_provenance_failure_v1(
    observer: &InvocationObserverV1<'_, '_>,
    failure: &ProvenanceFailure,
) {
    if matches!(failure, ProvenanceFailure::ResourceLimit { .. }) {
        deny(
            observer,
            Phase::ProvenanceAlias,
            "provenance analysis resource limit",
        );
    }
}

pub(super) fn observe_simt_cache_v1(
    observer: &InvocationObserverV1<'_, '_>,
    result: &Result<&SimtAnalysis, SimtCacheFailure>,
) {
    match result {
        // The cap producer replaces all diagnostics with this sole sentinel.
        Ok(analysis) if matches!(analysis.issues(), [SimtIssue::ResourceLimitExceeded]) => {
            deny(observer, Phase::SimtProtocol, "SIMT protocol issue limit")
        }
        Err(SimtCacheFailure::Trace(failure)) => observe_trace_failure_v1(observer, failure),
        _ => {}
    }
}

pub(super) fn observe_memory_order_failure_v1(
    observer: &InvocationObserverV1<'_, '_>,
    failure: &MemoryCacheFailure,
) {
    match failure {
        MemoryCacheFailure::Trace(failure) => observe_trace_failure_v1(observer, failure),
        MemoryCacheFailure::MemoryOrder(MemoryFailure::VersionLimitExceeded) => {
            deny(observer, Phase::MemoryOrder, "memory-order version limit")
        }
        MemoryCacheFailure::MemoryOrder(MemoryFailure::PublicationEdgeLimitExceeded) => deny(
            observer,
            Phase::MemoryOrder,
            "memory-order publication edge limit",
        ),
        MemoryCacheFailure::MemoryOrder(MemoryFailure::IssueLimitExceeded) => {
            deny(observer, Phase::MemoryOrder, "memory-order issue limit")
        }
        // The original provenance cache must be observed before stringification.
        MemoryCacheFailure::Provenance(_) | MemoryCacheFailure::MemoryOrder(_) => {}
    }
}

#[cfg(test)]
mod full_ordinary_observation_tests {
    use super::super::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use super::super::{
        PipelineErrorV1, PipelineFamilyV1, PipelineOutcomeV1, ProductionPlironPreloweringErrorV2,
        run_shared_production_checks_v1,
    };
    use super::{Limit, Phase};
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1 as Provider;
    use crate::production_analysis::pliron_pass_contract::{
        PlironPassPreservationErrorV1 as PreservationError, PlironStructuralIdentityProviderV1,
    };
    use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitsV1 as Limits;
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
    };

    fn exercise(work_short: bool) {
        let hard = Limits::production_hard_ceiling();
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "ordinary_observed_return".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        dialect_kernel::ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        let Ok(capture) = Provider::new(&context, &function).capture_with_resource_limits_v1(hard)
        else {
            panic!("actual external identity capture failed");
        };
        let PipelineOutcomeV1::Ordinary(expected) = run_shared_production_checks_v1(
            &context,
            &function,
            None,
            None,
            hard,
            (PipelineFamilyV1::Ordinary, None),
            None,
        )
        .unwrap() else {
            panic!("ordinary dispatcher returned conditional outcome")
        };
        assert!(expected.report.is_clean());
        assert_eq!(expected.report.preservation().certificates().len(), 9);
        assert_eq!(expected.report.report_validation().stages().len(), 9);
        let h = expected.resource_upper_bound;
        assert!(capture.resource_upper_bound.retained_storage_upper_bound() > 0);
        // Both owners remain alive throughout the measured invocation.
        let q = capture
            .resource_upper_bound
            .checked_then_retain(h, Phase::StructuralIdentity)
            .unwrap();
        let total = q.checked_then_retain(h, Phase::StructuralIdentity).unwrap();
        let global = Limits::new(
            total.work_upper_bound() - usize::from(work_short),
            total.peak_storage_upper_bound(),
        );
        let mut receipt = Receipt::new(q, global).unwrap();
        let result = run_shared_production_checks_v1(
            &context,
            &function,
            None,
            None,
            hard,
            (PipelineFamilyV1::Ordinary, Some(&mut receipt)),
            None,
        );
        let state = receipt.snapshot();
        assert!(!state.caught_panic);
        assert_eq!(state.current, state.committed);
        if work_short {
            let denial = Limit {
                phase: Phase::PassPreservation,
                resource: "work upper bound",
            };
            assert!(matches!(
                result,
                Err(PipelineErrorV1::Ordinary(
                    ProductionPlironPreloweringErrorV2::Preservation(
                        PreservationError::ResourceLimit {
                            resource: "work upper bound"
                        }
                    )
                ))
            ));
            assert_eq!(state.first_denial, Some(denial));
            assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
            let finish = expected.report.preservation().resource_upper_bound_v1();
            assert_eq!(
                state.current.work_upper_bound(),
                h.work_upper_bound()
                    .checked_sub(finish.work_upper_bound())
                    .unwrap()
            );
            assert!(state.current.retained_storage_upper_bound() > 0);
            global
                .require(
                    Phase::PassPreservation,
                    q.checked_then_retain(state.current, Phase::PassPreservation)
                        .unwrap(),
                )
                .unwrap();
        } else {
            let PipelineOutcomeV1::Ordinary(actual) = result.unwrap() else {
                panic!("wrong family")
            };
            assert_eq!(actual.report, expected.report);
            assert_eq!(actual.resource_upper_bound, h);
            assert_eq!(state.first_denial, None);
            assert_eq!(state.committed, h);
            assert_eq!(receipt.complete(), Ok(h));
        }
        drop((receipt, expected, capture));
    }
    #[test]
    fn full_ordinary_observer_exact_global_work_and_storage() {
        exercise(false);
    }
    #[test]
    fn full_ordinary_observer_global_work_one_short_keeps_accepted_prefix() {
        exercise(true);
    }
}

#[cfg(test)]
mod actual_cache_classifier_tests {
    use super::super::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use super::*;
    use crate::production_analysis::{
        pliron_analysis_manager::PlironAnalysisManagerV1 as Manager,
        pliron_invocation_trace::{
            preflight_execution_layout_resource_upper_bound_v1 as layout_bound,
            preflight_invocation_trace_resource_upper_bound_v1 as trace_bound,
        },
        pliron_ir_identity::LivePlironStructuralIdentityProviderV1 as Provider,
        pliron_memory_order::preflight_memory_order_attempt_resource_upper_bound_v1 as memory_bound,
        pliron_pass_contract::begin_production_pliron_pass_contract_session_with_resource_limits_v1 as begin,
        pliron_presburger_adapter::preflight_presburger_resource_upper_bound_v1 as presburger_bound,
        pliron_provenance_alias::preflight_provenance_alias_resource_upper_bound_v1 as provenance_bound,
        pliron_race::MAX_PLIRON_RACE_INVOCATIONS_V1,
        pliron_resource_envelope::{
            ProductionAnalysisResourceLimitsV1 as Limits,
            ProductionAnalysisResourceUpperBoundV1 as Bound,
        },
        pliron_simt_protocol::preflight_simt_protocol_resource_upper_bound_v1 as simt_bound,
        pliron_sparse_index::preflight_sparse_index_resource_upper_bound_v1 as sparse_bound,
    };
    use dialect_kernel::{InvocationIndexOp, ReturnOp};
    use fe2o3_pliron_owner_core::ensure_context_identity;
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
    };

    fn hard() -> Limits {
        Limits::production_hard_ceiling()
    }

    fn remaining(manager: &Manager, phase: Phase) -> Limits {
        manager.remaining_resource_limits(phase).unwrap()
    }

    fn inspect(
        floor: Bound,
        owner: Phase,
        expected: Option<Limit>,
        classify: impl FnOnce(&InvocationObserverV1<'_, '_>),
    ) {
        let mut receipt = Receipt::new(floor, hard()).unwrap();
        let phase = receipt.phase(owner, 0).unwrap();
        classify(&phase.observer(&Ok));
        drop(phase);
        let state = receipt.snapshot();
        assert_eq!(state.first_denial, expected);
        assert!(!state.caught_panic);
        // Only classification is observed. Already-admitted owners are in floor.
        assert_eq!(state.current, Bound::default());
        assert_eq!(state.committed, Bound::default());
        assert_eq!(
            receipt.complete(),
            match expected {
                Some(error) => Err(ReceiptFailure::Denied(error)),
                None => Ok(Bound::default()),
            }
        );
    }

    fn actual_trace_chain(extent: u64, quota: bool) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        ensure_context_identity(&mut context).unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "cached_trace_classifier".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        InvocationIndexOp::new(&mut context, 0, extent)
            .get_operation()
            .insert_at_back(entry, &context);
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        let preservation = begin(Provider::new(&context, &function), hard()).unwrap();
        let census = preservation.input_census_v1();
        let mut manager = Manager::new_with_resource_contract(
            &function,
            census,
            preservation.initial_identity_resource_upper_bound_v1(),
            preservation
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound(),
            hard(),
        )
        .unwrap();
        macro_rules! reserve {
            ($phase:expr, $bound:expr) => {{
                let bound = $bound;
                manager
                    .admit_retained_resource_upper_bound($phase, bound)
                    .unwrap();
            }};
        }
        reserve!(
            Phase::SparseIndex,
            sparse_bound(
                &context,
                &function,
                census,
                remaining(&manager, Phase::SparseIndex),
            )
            .unwrap()
        );
        manager.prepare_sparse_indices(&context, &function);
        assert!(manager.function_inventory().is_ok());
        assert!(manager.sparse_indices().is_ok());
        reserve!(
            Phase::Presburger,
            presburger_bound(
                manager.sparse_indices().ok(),
                remaining(&manager, Phase::Presburger),
            )
            .unwrap()
        );
        manager.prepare_presburger(&context, &function);
        assert!(manager.presburger().is_ok());
        reserve!(
            Phase::LaunchContract,
            layout_bound(census, remaining(&manager, Phase::LaunchContract),).unwrap()
        );
        manager.prepare_execution_layout(&context, &function);
        let layout = manager.execution_layout().unwrap();
        assert!(layout.is_none());
        let preflight = trace_bound(
            &context,
            Some(manager.function_inventory().unwrap()),
            census,
            Some(manager.sparse_indices().unwrap()),
            layout,
            remaining(&manager, Phase::InvocationTrace),
        )
        .unwrap();
        reserve!(Phase::InvocationTrace, preflight.attempt_upper_bound());
        manager.prepare_exact_trace(&context, &function);
        let traces = manager.exact_trace();
        let failure = traces.as_ref().unwrap_err().clone();
        let expected_failure = if quota {
            TraceFailure::LaunchTooLarge {
                invocations: extent,
            }
        } else {
            TraceFailure::DynamicLaunch { dimension: 0 }
        };
        assert_eq!(failure, expected_failure);
        let expected_denial = quota.then_some(Limit {
            phase: Phase::InvocationTrace,
            resource: "invocation trace launch limit",
        });
        let floor = manager.resource_upper_bound();
        assert!(floor.work_upper_bound() > 0);
        assert!(floor.retained_storage_upper_bound() > 0);
        inspect(floor, Phase::InvocationTrace, expected_denial, |observer| {
            observe_trace_failure_v1(observer, &failure);
        });
        // Both actual failures use the existing no-exact-trace fallback.
        let admission = preflight.exact_admission(traces).unwrap();
        assert_eq!(admission, None);
        reserve!(
            Phase::ProvenanceAlias,
            provenance_bound(census, remaining(&manager, Phase::ProvenanceAlias),).unwrap()
        );
        manager.prepare_provenance_alias(&context, &function);
        assert!(manager.provenance_alias().is_ok());
        reserve!(
            Phase::SimtProtocol,
            simt_bound(admission, remaining(&manager, Phase::SimtProtocol),).unwrap()
        );
        manager.prepare_simt_protocol(&context, &function);
        let simt = manager.simt_protocol();
        assert_eq!(
            simt.as_ref().unwrap_err(),
            &SimtCacheFailure::Trace(failure.clone())
        );
        inspect(
            manager.resource_upper_bound(),
            Phase::SimtProtocol,
            expected_denial,
            |observer| observe_simt_cache_v1(observer, &simt),
        );
        let memory =
            memory_bound(census, admission, remaining(&manager, Phase::MemoryOrder)).unwrap();
        reserve!(Phase::MemoryOrder, memory.upper_bound());
        manager.prepare_memory_order(&context, &function);
        let memory_failure = manager.memory_order().unwrap_err();
        assert_eq!(memory_failure, MemoryCacheFailure::Trace(failure));
        inspect(
            manager.resource_upper_bound(),
            Phase::MemoryOrder,
            expected_denial,
            |observer| observe_memory_order_failure_v1(observer, &memory_failure),
        );
        // Owners remain live until every classifier inspection has completed.
        drop(manager);
        drop(preservation);
    }

    #[test]
    fn actual_cached_launch_65537_is_quota_through_both_wrappers() {
        assert_eq!(MAX_PLIRON_RACE_INVOCATIONS_V1, 65_536);
        actual_trace_chain(65_537, true);
    }

    #[test]
    fn actual_cached_dynamic_launch_is_nonquota_through_both_wrappers() {
        actual_trace_chain(0, false);
    }

    #[test]
    fn classifier_only_memory_order_cap_matrix() {
        for (failure, resource) in [
            (
                MemoryFailure::VersionLimitExceeded,
                "memory-order version limit",
            ),
            (
                MemoryFailure::PublicationEdgeLimitExceeded,
                "memory-order publication edge limit",
            ),
            (
                MemoryFailure::IssueLimitExceeded,
                "memory-order issue limit",
            ),
        ] {
            let failure = MemoryCacheFailure::MemoryOrder(failure);
            inspect(
                Bound::default(),
                Phase::MemoryOrder,
                Some(Limit {
                    phase: Phase::MemoryOrder,
                    resource,
                }),
                |observer| observe_memory_order_failure_v1(observer, &failure),
            );
        }
    }

    mod row_preparation_tests {
        use super::*;
        use crate::production_analysis::pliron_hierarchical_ownership::conditional_execution_v1 as rows;
        use std::{
            cell::{Cell, RefCell},
            panic::{AssertUnwindSafe, catch_unwind},
        };

        const OWNER: Phase = Phase::HierarchicalOwnership;
        const ROWS: usize = 1;

        fn exercise(borrow_panic: bool) {
            let mut context = Context::new();
            dialect_kernel::register_dialect(
                &mut context,
                &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
            )
            .unwrap();
            ensure_context_identity(&mut context).unwrap();
            let signature = FunctionType::get(&context, vec![], vec![]);
            let function = FuncOp::new(
                &mut context,
                "observed_row_preparation".try_into().unwrap(),
                signature,
            );
            let entry = function.get_entry_block(&context);
            // Real SSA names make N exceed capture's historical scratch peak.
            for index in 0..4 {
                let op = InvocationIndexOp::new(&mut context, 0, 64).get_operation();
                op.insert_at_back(entry, &context);
                let result = op.deref(&context).get_result(0);
                result.set_name(
                    &context,
                    Some(
                        pliron::identifier::Identifier::try_new(format!(
                            "named{index}{}",
                            "x".repeat(32_768)
                        ))
                        .unwrap(),
                    ),
                );
            }
            ReturnOp::new(&mut context)
                .get_operation()
                .insert_at_back(entry, &context);

            let preservation = begin(Provider::new(&context, &function), hard()).unwrap();
            let census = preservation.input_census_v1();
            let expected_epoch = preservation.initial_subject_v1().unwrap().1;
            assert_eq!(census.results, 4);
            let fresh = || {
                let mut manager = Manager::new_with_resource_contract(
                    &function,
                    census,
                    preservation.initial_identity_resource_upper_bound_v1(),
                    preservation
                        .lineage_identity_resource_upper_bound_v1()
                        .retained_storage_upper_bound(),
                    hard(),
                )
                .unwrap();
                // Constructor already admits the real inventory reservation.
                manager.prepare_function_inventory(&context, &function);
                assert!(manager.function_inventory().is_ok());
                manager
            };

            // Observe actual O and returned O.then(N), never subtract AM totals.
            // Reference owners die before the independent boundary trials.
            let (floor, overhead, full, named) = {
                let mut manager = fresh();
                let floor = manager.resource_upper_bound();
                assert!(floor.work_upper_bound() > 0);
                assert!(floor.retained_storage_upper_bound() > 0);
                let mut receipt = Receipt::new(floor, hard()).unwrap();
                let phase = receipt.phase(OWNER, 0).unwrap();
                let first = Cell::new(None);
                let last = Cell::new(None);
                let calls = Cell::new(0);
                let project = |bound: Bound| {
                    if first.get().is_none() {
                        first.set(Some(bound));
                    }
                    last.set(Some(bound));
                    calls.set(calls.get() + 1);
                    Ok(bound)
                };
                let (prepared, metadata) = rows::prepare_rows_with_observation_v1(
                    &context,
                    &function,
                    &mut manager,
                    census,
                    expected_epoch,
                    ROWS,
                    Some(&phase.observer(&project)),
                )
                .unwrap();
                let overhead = first.get().unwrap();
                let full = metadata.unwrap();
                assert_eq!(calls.get(), 2);
                assert_eq!(last.get(), Some(full));
                assert!(prepared.rows.is_empty());
                assert_eq!(prepared.rows.capacity(), ROWS);
                assert!(prepared.named_census.identifier_bytes > census.identifier_bytes);
                assert!(full.work_upper_bound() > overhead.work_upper_bound());
                assert!(
                    full.retained_storage_upper_bound() > overhead.retained_storage_upper_bound()
                );
                for bound in [overhead, full] {
                    assert_eq!(
                        bound.retained_storage_upper_bound(),
                        bound.peak_storage_upper_bound()
                    );
                }
                assert_eq!(
                    manager.resource_upper_bound(),
                    floor.checked_then_retain(full, OWNER).unwrap()
                );
                phase.commit(full).unwrap();
                assert_eq!(receipt.snapshot().first_denial, None);
                assert_eq!(receipt.complete(), Ok(full));
                (floor, overhead, full, prepared.named_census)
            };
            let through_o = floor.checked_then_retain(overhead, OWNER).unwrap();
            let through_n = floor.checked_then_retain(full, OWNER).unwrap();

            if borrow_panic {
                let mut manager = fresh();
                assert_eq!(manager.resource_upper_bound(), floor);
                let raw = manager.function_inventory().unwrap().operations()[0].pointer();
                assert_eq!(raw.deref(&context).get_num_results(), 1);
                let held = RefCell::new(None);
                let calls = Cell::new(0);
                let mut receipt = Receipt::new(floor, hard()).unwrap();
                let phase = receipt.phase(OWNER, 0).unwrap();
                let project = |bound: Bound| {
                    assert_eq!(calls.get(), 0);
                    assert_eq!(bound, overhead);
                    calls.set(1);
                    // Entry checks passed; no IR bytes are changed.
                    *held.borrow_mut() = Some(raw.deref_mut(&context));
                    Ok(bound)
                };
                let observer = phase.observer(&project);
                let panicked = catch_unwind(AssertUnwindSafe(|| {
                    rows::prepare_rows_with_observation_v1(
                        &context,
                        &function,
                        &mut manager,
                        census,
                        expected_epoch,
                        ROWS,
                        Some(&observer),
                    )
                }))
                .is_err();
                drop(held.borrow_mut().take());
                // Outside catch: phase Drop cannot manufacture caught_panic.
                drop(phase);
                assert!(panicked);
                assert_eq!(calls.get(), 1);
                assert_eq!(manager.resource_upper_bound(), through_o);
                let state = receipt.snapshot();
                assert_eq!(state.current, overhead);
                assert_eq!(state.committed, overhead);
                assert_eq!(state.first_denial, None);
                assert!(state.caught_panic);
                assert_eq!(receipt.complete(), Err(ReceiptFailure::CaughtPanic));
                return;
            }

            // Prove both one-short limits accept O and reject only O.then(N).
            assert!(through_n.work_upper_bound() > through_o.work_upper_bound());
            assert!(
                through_n.peak_storage_upper_bound() > through_o.peak_storage_upper_bound(),
                "O={through_o:?}, N={through_n:?}"
            );
            for short in 0..3 {
                let limits = Limits::new(
                    through_n.work_upper_bound() - usize::from(short == 1),
                    through_n.peak_storage_upper_bound() - usize::from(short == 2),
                );
                limits.require(OWNER, through_o).unwrap();
                // Original wrapper, explicit None adapter, observed adapter.
                for mode in 0..3 {
                    let observed = mode == 2;
                    let denial = (observed && short != 0).then_some(Limit {
                        phase: OWNER,
                        resource: if short == 1 {
                            "work upper bound"
                        } else {
                            "peak storage upper bound"
                        },
                    });
                    let mut manager = fresh();
                    assert_eq!(manager.resource_upper_bound(), floor);
                    let mut receipt = Receipt::new(floor, limits).unwrap();
                    let phase = receipt.phase(OWNER, 0).unwrap();
                    let observer = phase.observer(&Ok);
                    let result = if mode == 0 {
                        rows::prepare_rows_v1(
                            &context,
                            &function,
                            &mut manager,
                            census,
                            expected_epoch,
                            ROWS,
                        )
                        .map(|prepared| (prepared, None))
                    } else {
                        rows::prepare_rows_with_observation_v1(
                            &context,
                            &function,
                            &mut manager,
                            census,
                            expected_epoch,
                            ROWS,
                            observed.then_some(&observer),
                        )
                    };
                    let kept = match result {
                        Ok((prepared, metadata)) => {
                            assert!(denial.is_none());
                            assert_eq!(metadata, observed.then_some(full));
                            assert_eq!(prepared.named_census, named);
                            assert!(prepared.rows.is_empty());
                            assert_eq!(prepared.rows.capacity(), ROWS);
                            if observed {
                                phase.commit(full).unwrap();
                            } else {
                                drop(phase);
                            }
                            Some(prepared)
                        }
                        Err(failure) => {
                            assert_eq!(failure, rows::FailureV1::Resource(denial.unwrap()));
                            drop(phase);
                            None
                        }
                    };
                    let prefix = if !observed {
                        Bound::default()
                    } else if denial.is_some() {
                        overhead
                    } else {
                        full
                    };
                    assert_eq!(
                        manager.resource_upper_bound(),
                        if denial.is_some() {
                            through_o
                        } else {
                            through_n
                        }
                    );
                    let state = receipt.snapshot();
                    assert_eq!(state.first_denial, denial);
                    assert_eq!(state.current, prefix);
                    assert_eq!(state.committed, prefix);
                    assert!(!state.caught_panic);
                    assert_eq!(
                        receipt.complete(),
                        denial.map_or(Ok(prefix), |error| Err(ReceiptFailure::Denied(error)))
                    );
                    // Keep successful row storage live through receipt assertions.
                    drop(kept);
                }
            }
        }

        #[test]
        fn actual_row_preparation_exact_and_global_one_short() {
            exercise(false);
        }

        #[test]
        fn actual_row_preparation_census_borrow_panic_retains_overhead() {
            exercise(true);
        }
    }

    mod tensor_producer_tests {
        use super::*;
        use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObservationV1 as Observation;
        use crate::production_analysis::pliron_tensor_layout::{
            MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1, PlironTensorLayoutCheckErrorV1 as TensorError,
            PlironTensorLayoutFindingV1 as Finding, PlironTensorLayoutReportV1 as TensorReport,
            preflight_tensor_layout_resource_upper_bound_v1 as tensor_bound,
            require_pliron_tensor_layout_with_observation_v1 as require_tensor,
        };
        use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
        use dialect_kernel::{TensorConvergenceAttr, TensorLayoutOp};
        use fe2o3_kernel_ir::TensorLayoutContractV1;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        enum Case {
            TraceCap,
            Semantic,
            Truncation,
            BorrowPanic,
        }

        struct Outcome {
            report: Option<Result<TensorReport, TensorError>>,
            state: Observation,
            bound: Bound,
            completion: Result<Bound, ReceiptFailure>,
        }

        fn produce(case: Case, observed: bool) -> Outcome {
            let mut context = Context::new();
            dialect_kernel::register_dialect(
                &mut context,
                &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
            )
            .unwrap();
            dialect_gpu::register_dialect(&mut context).unwrap();
            ensure_context_identity(&mut context).unwrap();
            let signature = FunctionType::get(&context, vec![], vec![]);
            let function = FuncOp::new(
                &mut context,
                "observed_tensor_producer".try_into().unwrap(),
                signature,
            );
            let entry = function.get_entry_block(&context);
            let extent = match case {
                Case::TraceCap => 65_600,
                Case::BorrowPanic => 64,
                Case::Semantic | Case::Truncation => 1,
            };
            if matches!(case, Case::TraceCap | Case::BorrowPanic) {
                ExecutionLayoutOp::new_with_domain(
                    &mut context,
                    7,
                    [extent, 1, 1],
                    [64, 1, 1],
                    64,
                    ExecutionDomainAttr::FullPhysicalWorkgroups,
                )
                .get_operation()
                .insert_at_back(entry, &context);
            }
            InvocationIndexOp::new(&mut context, 0, extent)
                .get_operation()
                .insert_at_back(entry, &context);
            let count = if case == Case::Truncation {
                MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1
            } else {
                1
            };
            let convergence = if matches!(case, Case::Semantic | Case::Truncation) {
                TensorConvergenceAttr::Opaque
            } else {
                TensorConvergenceAttr::UniformSubgroup
            };
            let contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
            let mut first_tensor = None;
            for _ in 0..count {
                let raw =
                    TensorLayoutOp::new(&mut context, &contract, convergence, 64).get_operation();
                if first_tensor.is_none() {
                    first_tensor = Some(raw);
                }
                raw.insert_at_back(entry, &context);
            }
            ReturnOp::new(&mut context)
                .get_operation()
                .insert_at_back(entry, &context);
            let preservation = begin(Provider::new(&context, &function), hard()).unwrap();
            let census = preservation.input_census_v1();
            let mut manager = Manager::new_with_resource_contract(
                &function,
                census,
                preservation.initial_identity_resource_upper_bound_v1(),
                preservation
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound(),
                hard(),
            )
            .unwrap();
            macro_rules! reserve {
                ($phase:expr, $bound:expr) => {{
                    let bound = $bound;
                    manager
                        .admit_retained_resource_upper_bound($phase, bound)
                        .unwrap();
                }};
            }
            reserve!(
                Phase::SparseIndex,
                sparse_bound(
                    &context,
                    &function,
                    census,
                    remaining(&manager, Phase::SparseIndex),
                )
                .unwrap()
            );
            manager.prepare_sparse_indices(&context, &function);
            assert!(manager.sparse_indices().is_ok());
            reserve!(
                Phase::Presburger,
                presburger_bound(
                    manager.sparse_indices().ok(),
                    remaining(&manager, Phase::Presburger),
                )
                .unwrap()
            );
            manager.prepare_presburger(&context, &function);
            assert!(manager.presburger().is_ok());
            reserve!(
                Phase::LaunchContract,
                layout_bound(census, remaining(&manager, Phase::LaunchContract)).unwrap()
            );
            manager.prepare_execution_layout(&context, &function);
            let layout = manager.execution_layout().unwrap();
            let trace = trace_bound(
                &context,
                Some(manager.function_inventory().unwrap()),
                census,
                Some(manager.sparse_indices().unwrap()),
                layout,
                remaining(&manager, Phase::InvocationTrace),
            )
            .unwrap();
            reserve!(Phase::InvocationTrace, trace.attempt_upper_bound());
            manager.prepare_exact_trace(&context, &function);
            if case == Case::TraceCap {
                assert!(matches!(
                    manager.exact_trace(),
                    Err(TraceFailure::LaunchTooLarge {
                        invocations: 65_600
                    })
                ));
            } else {
                assert!(manager.exact_trace().is_ok());
            }
            let admission = trace.exact_admission(manager.exact_trace()).unwrap();
            // Real dependency owners remain live in this inherited floor.
            let floor = manager.resource_upper_bound();
            assert!(floor.work_upper_bound() > 0);
            assert!(floor.retained_storage_upper_bound() > 0);
            let local = remaining(&manager, Phase::TensorLayout);
            let bound = tensor_bound(census, admission, local).unwrap();
            let mut receipt = Receipt::new(floor, hard()).unwrap();
            let phase = receipt.phase(Phase::TensorLayout, 0).unwrap();
            let observer = phase.observer(&Ok);
            // Admission precedes both the borrow conflict and the producer call.
            observer
                .require(local, Phase::TensorLayout, Ok(bound))
                .unwrap();
            manager
                .admit_retained_resource_upper_bound(Phase::TensorLayout, bound)
                .unwrap();
            let held =
                (case == Case::BorrowPanic).then(|| first_tensor.unwrap().deref_mut(&context));
            let result = catch_unwind(AssertUnwindSafe(|| {
                require_tensor(
                    &context,
                    &function,
                    &mut manager,
                    observed.then_some(&observer),
                )
            }));
            drop(held);
            let report = match result {
                Ok(report) => {
                    phase.commit(bound).unwrap();
                    Some(report)
                }
                Err(_) => {
                    // Drop outside catch: only the producer guard observes this unwind.
                    drop(phase);
                    None
                }
            };
            assert_eq!(
                manager.resource_upper_bound(),
                floor
                    .checked_then_retain(bound, Phase::TensorLayout)
                    .unwrap()
            );
            Outcome {
                report,
                state: receipt.snapshot(),
                bound,
                completion: receipt.complete(),
            }
        }

        #[test]
        fn actual_tensor_trace_cap_survives_clean_symbolic_fallback() {
            let ordinary = produce(Case::TraceCap, false);
            let observed = produce(Case::TraceCap, true);
            assert_eq!(observed.report, ordinary.report);
            assert!(
                observed
                    .report
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .is_clean()
            );
            let denial = Limit {
                phase: Phase::InvocationTrace,
                resource: "invocation trace launch limit",
            };
            assert_eq!(ordinary.state.first_denial, None);
            assert_eq!(ordinary.completion, Ok(ordinary.bound));
            assert_eq!(observed.state.first_denial, Some(denial));
            assert_eq!(observed.completion, Err(ReceiptFailure::Denied(denial)));
            for outcome in [&ordinary, &observed] {
                assert_eq!(outcome.state.committed, outcome.bound);
                assert_eq!(outcome.state.current, outcome.bound);
                assert!(!outcome.state.caught_panic);
            }
        }

        #[test]
        fn actual_tensor_semantic_and_diagnostic_truncation_are_nonquota() {
            for case in [Case::Semantic, Case::Truncation] {
                let ordinary = produce(case, false);
                let observed = produce(case, true);
                assert_eq!(observed.report, ordinary.report);
                let report = observed
                    .report
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap_err()
                    .report();
                assert!(
                    report
                        .findings()
                        .iter()
                        .any(|finding| matches!(finding, Finding::ConvergenceMismatch { .. }))
                );
                if case == Case::Truncation {
                    assert_eq!(
                        report.findings().len(),
                        MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 + 1
                    );
                    assert!(matches!(
                        report.findings().last(),
                        Some(Finding::ResourceLimitExceeded)
                    ));
                }
                for outcome in [&ordinary, &observed] {
                    assert_eq!(outcome.state.first_denial, None);
                    assert!(!outcome.state.caught_panic);
                    assert_eq!(outcome.state.committed, outcome.bound);
                    assert_eq!(outcome.state.current, outcome.bound);
                    assert_eq!(outcome.completion, Ok(outcome.bound));
                }
            }
        }

        #[test]
        fn actual_tensor_borrow_panic_keeps_admitted_prefix() {
            for observed in [false, true] {
                let outcome = produce(Case::BorrowPanic, observed);
                assert!(outcome.report.is_none());
                let held = Bound::checked_phase(
                    Phase::TensorLayout,
                    outcome.bound.work_upper_bound(),
                    outcome.bound.peak_storage_upper_bound(),
                    0,
                )
                .unwrap();
                assert_eq!(outcome.state.committed, held);
                assert_eq!(outcome.state.current, held);
                assert_eq!(outcome.state.first_denial, None);
                assert_eq!(outcome.state.caught_panic, observed);
                assert_eq!(
                    outcome.completion,
                    if observed {
                        Err(ReceiptFailure::CaughtPanic)
                    } else {
                        Ok(held)
                    }
                );
            }
        }
    }
}
