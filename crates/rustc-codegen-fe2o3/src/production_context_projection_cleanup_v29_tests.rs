//! Private callback lifecycle controls over the actual projected source fixture.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

// Exact capacity-successor predecessor, only for nominal transcript parity.
fn legacy_project<R>(
    source: &RetainedExecutionSourceV29<'_>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    use_source: impl for<'a> FnOnce(
        ProductionExecutionSourceInputV29<'a>,
        &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<R, ProductionContextRootErrorV29>,
) -> Result<R, ProductionContextRootErrorV29> {
    let ledger = budget.work_ledger_identity_v1();
    let bytes = [
        projection_bytes::<ProductionContextRootInputV29<'_>>(source.roots().len())?,
        projection_bytes::<ProductionScopeCallableCandidateV29>(source.classes().len())?,
        projection_bytes::<ProductionScopeEventCandidateV29>(source.events().len())?,
    ]
    .into_iter()
    .try_fold(0_usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })?;
    let work = [
        (source.roots().len(), 32),
        (source.classes().len(), 12),
        (source.events().len(), 8),
    ]
    .into_iter()
    // Prepay the fixed capacity reconciliation for each of the three vectors.
    .try_fold(18_usize, |sum, (count, width)| {
        count
            .checked_mul(width)
            .and_then(|amount| sum.checked_add(amount))
            .ok_or(Resource::Arithmetic)
    })?;
    budget.charge_work(work)?;
    budget.reserve_storage(bytes)?;
    let mut charged = bytes;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut roots = projection_rows(source.roots().len(), budget, &mut charged)?;
        let mut classes = projection_rows(source.classes().len(), budget, &mut charged)?;
        let mut events = projection_rows(source.events().len(), budget, &mut charged)?;
        for entry in source.roots() {
            let (root, root_identity) = entry.root();
            let (helper, helper_identity) = entry.helper();
            let (issuer, issuer_identity) = entry.issuer();
            let (context_type, context_identity) = entry.context();
            roots.push(ProductionContextRootInputV29 {
                semantic_sha256: source.semantic_sha256(),
                root,
                root_identity,
                helper,
                helper_identity,
                issuer,
                issuer_identity,
                context_type,
                context_identity,
                issuance: project_boundary(entry.issuance()),
                helper_call: project_boundary(entry.helper_call()),
                helper_context_local: entry.helper_argument(),
                helper_arguments: entry.helper_operands(),
            });
        }
        classes.extend(source.classes().iter().copied().map(project_class));
        events.extend(
            source
                .events()
                .iter()
                .map(|event| ProductionScopeEventCandidateV29 {
                    function: event.function,
                    block: event.block,
                    statement_count: event.statement_count,
                    kind: project_event(event.kind),
                }),
        );
        use_source(
            ProductionExecutionSourceInputV29 {
                semantic_sha256: source.semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            budget,
        )
    }));
    // The temporary row backing has dropped. Consumer-owned storage is not ours
    // to refund, and a replaced ledger must never receive our release.
    let cleanup = if budget.work_ledger_identity_v1() == ledger {
        budget.release_storage(charged)
    } else {
        Err(Resource::Accounting)
    };
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(payload) => resume_unwind(payload),
    }
}

#[test]
fn projection_cleanup_preserves_exact_nominal_predecessor_transcripts() {
    with_source(|source| {
        let (projection_work, capacities) = measured_projection(source);
        let backing: usize = capacities.iter().sum();
        for exit in 0..3 {
            for work_limit in [
                7,
                7 + projection_work - 1,
                7 + projection_work + 2,
                7 + projection_work + 3,
            ] {
                for storage_limit in [17, 17 + backing, 27 + backing, 28 + backing] {
                    let run = |legacy| {
                        let _guard = AllocationGuard::install([Allocation::Extra(2); 3]);
                        let mut work = Work::new(work_limit);
                        let transcript = {
                            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                                &mut work,
                                storage_limit,
                            );
                            budget.reserve_storage(17).unwrap();
                            budget.charge_work(7).unwrap();
                            let ledger = budget.work_ledger_identity_v1();
                            let mut consumer = 0;
                            let body = |input: ProductionExecutionSourceInputV29<'_>,
                                        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>| {
                                assert_eq!(input.roots.len(), source.roots().len());
                                budget.reserve_storage(11)?;
                                consumer = 11;
                                budget.charge_work(3)?;
                                let output = Box::new([42_u8; 11]);
                                match exit {
                                    0 => Ok(output),
                                    1 => Err(ProductionContextRootErrorV29::Arguments),
                                    _ => std::panic::panic_any(String::from("nominal projection panic")),
                                }
                            };
                            let result = catch_unwind(AssertUnwindSafe(|| {
                                if legacy {
                                    legacy_project(source, &mut budget, body)
                                } else {
                                    with_projected_execution_source_v29(source, &mut budget, body)
                                }
                            }));
                            let result = match result {
                                Ok(Ok(output)) => {
                                    assert_eq!(*output, [42; 11]);
                                    drop(output);
                                    String::from("success")
                                }
                                Ok(Err(error)) => format!("error: {error:?}"),
                                Err(payload) => *payload.downcast::<String>().unwrap(),
                            };
                            assert_eq!(budget.storage(), 17 + consumer);
                            assert!(budget.work_ledger_identity_v1() == ledger);
                            let state = (
                                result,
                                budget.storage(),
                                budget.work(),
                                budget.peak_storage(),
                                budget.failed_storage(),
                            );
                            budget.release_storage(consumer).unwrap();
                            assert_eq!(budget.storage(), 17);
                            state
                        };
                        (transcript, work.failed_work())
                    };
                    assert_eq!(run(false), run(true));
                }
            }
        }
    });
}

#[test]
fn projection_cleanup_refuses_same_ledger_entry_floor_undercut_without_refund() {
    with_source(|source| {
        for error in [false, true] {
            let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
            let mut work = Work::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
            budget.reserve_storage(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = with_projected_execution_source_v29(source, &mut budget, |_, budget| {
                budget.release_storage(1)?;
                if error {
                    Err(ProductionContextRootErrorV29::Arguments)
                } else {
                    Ok(42)
                }
            });
            assert_eq!(
                result,
                Err(ProductionContextRootErrorV29::Resource(
                    Resource::Accounting
                ))
            );
            let backing: usize = guard.state().capacities.iter().sum();
            assert_eq!(budget.storage(), 16 + backing);
            assert_eq!(budget.peak_storage(), 17 + backing);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(budget.failed_storage().is_none());
        }
    });
}

#[test]
fn projection_cleanup_preserves_extra_owned_output_without_requiring_exact_floor() {
    with_source(|source| {
        let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
        let mut work = Work::new(100_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
        budget.reserve_storage(17).unwrap();
        let output = with_projected_execution_source_v29(source, &mut budget, |_, budget| {
            budget.reserve_storage(11)?;
            Ok(Box::new([5_u8; 11]))
        })
        .unwrap();
        assert_eq!(*output, [5; 11]);
        assert_eq!(budget.storage(), 28);
        assert_eq!(
            budget.peak_storage(),
            28 + guard.state().capacities.iter().sum::<usize>()
        );
        drop(output);
        budget.release_storage(11).unwrap();
        assert_eq!(budget.storage(), 17);
    });
}

struct Rejected(Arc<AtomicUsize>);
impl Drop for Rejected {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("rejected projection result Drop");
    }
}

#[test]
fn projection_cleanup_contains_rejected_result_drop_after_same_ledger_refusal() {
    with_source(|source| {
        let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut work = Work::new(100_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
        budget.reserve_storage(17).unwrap();
        let result = with_projected_execution_source_v29(source, &mut budget, |_, budget| {
            budget.release_storage(1)?;
            Ok(Rejected(dropped.clone()))
        });
        assert!(matches!(
            result,
            Err(ProductionContextRootErrorV29::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(
            budget.storage(),
            16 + guard.state().capacities.iter().sum::<usize>()
        );
    });
}

#[test]
fn projection_cleanup_foreign_stack_ledger_keeps_both_histories_and_accounting_error() {
    with_source(|source| {
        let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut original_work = Work::new(100_000);
        let mut foreign_work = Work::new(100_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut original_work, 1 << 20);
        let mut other =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut foreign_work, 1 << 20);
        budget.reserve_storage(17).unwrap();
        other.reserve_storage(101).unwrap();
        other.charge_work(5).unwrap();
        let original = budget.work_ledger_identity_v1();
        let foreign = other.work_ledger_identity_v1();
        let result = with_projected_execution_source_v29(source, &mut budget, |_, budget| {
            std::mem::swap(budget, &mut other);
            Ok(Rejected(dropped.clone()))
        });
        assert!(matches!(
            result,
            Err(ProductionContextRootErrorV29::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert!(budget.work_ledger_identity_v1() == foreign);
        assert_eq!(
            (budget.storage(), budget.work(), budget.peak_storage()),
            (101, 5, 101)
        );
        assert!(budget.failed_storage().is_none());
        assert!(other.work_ledger_identity_v1() == original);
        let backing: usize = guard.state().capacities.iter().sum();
        assert_eq!(other.storage(), 17 + backing);
        assert!(other.work() > 0);
        // The test owns both meters and knows the rejected result/backing dropped.
        other.release_storage(backing).unwrap();
        assert_eq!(other.storage(), 17);
    });
}

struct OriginalPayload(Arc<AtomicUsize>);
impl Drop for OriginalPayload {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("caller-owned original payload Drop");
    }
}

#[test]
fn projection_cleanup_resumes_original_payload_after_valid_or_refused_cleanup() {
    with_source(|source| {
        for undercut in [false, true] {
            let guard = AllocationGuard::install([Allocation::Extra(2); 3]);
            let dropped = Arc::new(AtomicUsize::new(0));
            let mut work = Work::new(100_000);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1 << 20);
            budget.reserve_storage(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_projected_execution_source_v29(
                    source,
                    &mut budget,
                    |_, budget| -> Result<(), ProductionContextRootErrorV29> {
                        if undercut {
                            budget.release_storage(1)?;
                        }
                        std::panic::panic_any(OriginalPayload(dropped.clone()))
                    },
                )
            }));
            let payload = result.expect_err("original callback panic must resume");
            let original = payload.downcast_ref::<OriginalPayload>().unwrap();
            assert!(Arc::ptr_eq(&original.0, &dropped));
            assert_eq!(dropped.load(Ordering::SeqCst), 0);
            let expected = if undercut {
                16 + guard.state().capacities.iter().sum::<usize>()
            } else {
                17
            };
            assert_eq!(budget.storage(), expected);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(catch_unwind(AssertUnwindSafe(|| drop(payload))).is_err());
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
            assert_eq!(budget.storage(), expected);
        }
    });
}
