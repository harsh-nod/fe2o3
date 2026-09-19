//! Test-only observation of a consuming source constructor; no executable continuation.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionPendingScopedSourceOwnerV29 as Pending, ProductionSemanticKirLimitsV1,
};
use std::convert::Infallible;

fn observe_pending_v29(
    entries: &RetainedContextEntriesV29,
    ssa: ProductionSemanticSsaOwnerV1,
    launch: ProductionSourceLaunchRosterV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    observe: impl FnOnce(
        &Pending,
        &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), ProductionPipelineError>,
) -> Result<Infallible, ProductionPipelineError> {
    let source = execution_source_v29(entries, &ssa, budget)?.ok_or(
        ProductionPipelineError::ContextHandoff(ProductionContextRootErrorV29::RootCensus),
    )?;
    let owner = with_projected_execution_source_v29(&source, budget, |input, budget| {
        Ok(Pending::try_materialize_with_budget(
            ssa,
            launch,
            input,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        ))
    })
    .map_err(ProductionPipelineError::ContextHandoff)?
    .map_err(ProductionPipelineError::PendingScopedSource)?;
    // Projection vectors are gone before replay or inspection; adopted storage stays live.
    let ledger = budget.work_ledger_identity_v1();
    let adopted = owner.adopted_storage();
    let ready_storage = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| {
        owner
            .replay_with_budget(budget)
            .map_err(ProductionPipelineError::PendingScopedSource)?;
        observe(&owner, budget)
    }));
    drop(owner);
    let cleanup = if budget.work_ledger_identity_v1() == ledger && budget.storage() >= ready_storage
    {
        budget.release_storage(adopted)
    } else {
        Err(Resource::Accounting)
    };
    match result {
        Ok(result) => {
            cleanup.map_err(|error| ProductionPipelineError::ContextHandoff(error.into()))?;
            result?;
            Err(ProductionPipelineError::PendingScopedObservationIncomplete)
        }
        Err(payload) => resume_unwind(payload),
    }
}

impl<'tcx> super::super::ProductionCompilation<'tcx, super::super::CollectedRustStage<'tcx>> {
    pub(crate) fn observe_pending_scoped_source_v29(
        self,
        observe: impl FnOnce(
            &Pending,
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<(), ProductionPipelineError>,
    ) -> Result<(), Box<ProductionPipelineError>> {
        let never = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .with_prepared_materialization_budget_v29(|prepared, budget| {
                super::super::consume_prepared_with_budget_v29(
                    prepared,
                    budget,
                    |_, _| Ok(()),
                    |ssa, launch, entries, budget| {
                        observe_pending_v29(entries, ssa, launch, budget, observe)
                    },
                )
            })?;
        match never.materialized {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    type Budget<'a> = CanonicalKernelIrVerificationResourceBudgetV1<'a>;

    #[test]
    fn pending_observer_restores_only_owner_storage_on_success_error_and_panic() {
        for mode in 0..3 {
            let (ssa, launch, entries) = RetainedContextEntriesV29::projection_test_fixture_v29();
            let mut work = Work::new(1_000_000_000);
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(7).unwrap();
            let mut visits = 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                observe_pending_v29(&entries, ssa, launch, &mut budget, |owner, budget| {
                    visits += 1;
                    assert_eq!(owner.pending_module().kernels.len(), 4);
                    assert_eq!(budget.storage(), 7 + owner.adopted_storage());
                    budget.reserve_storage(11).unwrap();
                    match mode {
                        0 => Ok(()),
                        1 => Err(ProductionPipelineError::ContextHandoff(
                            ProductionContextRootErrorV29::Arguments,
                        )),
                        _ => std::panic::panic_any(String::from("pending observer panic")),
                    }
                })
            }));
            match mode {
                0 => {
                    let result = result.unwrap();
                    assert!(
                        matches!(
                            result,
                            Err(ProductionPipelineError::PendingScopedObservationIncomplete)
                        ),
                        "{result:?}"
                    );
                }
                1 => {
                    let result = result.unwrap();
                    assert!(
                        matches!(
                            result,
                            Err(ProductionPipelineError::ContextHandoff(
                                ProductionContextRootErrorV29::Arguments
                            ))
                        ),
                        "{result:?}"
                    );
                }
                _ => assert_eq!(
                    *result.unwrap_err().downcast::<String>().unwrap(),
                    "pending observer panic"
                ),
            }
            assert_eq!(visits, 1);
            assert_eq!(budget.storage(), 18);
            assert!(budget.work() > 0);
        }
    }

    #[test]
    fn pending_observer_rejects_floor_theft_without_refunding_unowned_storage() {
        let (ssa, launch, entries) = RetainedContextEntriesV29::projection_test_fixture_v29();
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(7).unwrap();
        let mut remaining = 0;
        let error = observe_pending_v29(&entries, ssa, launch, &mut budget, |_, budget| {
            budget.release_storage(1).unwrap();
            remaining = budget.storage();
            Ok(())
        })
        .unwrap_err();
        assert!(
            matches!(
                error,
                ProductionPipelineError::ContextHandoff(ProductionContextRootErrorV29::Resource(
                    Resource::Accounting
                ))
            ),
            "{error:?}"
        );
        assert_eq!(budget.storage(), remaining);
    }

    #[test]
    fn pending_observer_preserves_foreign_ledger_and_original_panic() {
        for panic in [false, true] {
            let (ssa, launch, entries) = RetainedContextEntriesV29::projection_test_fixture_v29();
            let mut work = Work::new(1_000_000_000);
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(7).unwrap();
            let mut foreign = Budget::new(Box::leak(Box::new(Work::new(1_000_000_000))), 101);
            foreign.reserve_storage(101).unwrap();
            foreign.charge_work(5).unwrap();
            let mut foreign = Some(foreign);
            let result = catch_unwind(AssertUnwindSafe(|| {
                observe_pending_v29(&entries, ssa, launch, &mut budget, |_, budget| {
                    let _original = std::mem::replace(budget, foreign.take().unwrap());
                    if panic {
                        std::panic::panic_any(String::from("pending foreign panic"));
                    }
                    Ok(())
                })
            }));
            if panic {
                assert_eq!(
                    *result.unwrap_err().downcast::<String>().unwrap(),
                    "pending foreign panic"
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ProductionPipelineError::ContextHandoff(
                        ProductionContextRootErrorV29::Resource(Resource::Accounting)
                    ))
                ));
            }
            assert_eq!(budget.work(), 5);
            assert_eq!(budget.storage(), 101);
            assert_eq!(budget.peak_storage(), 101);
        }
    }

    #[test]
    fn pending_observer_exact_and_one_short_budgets_do_not_accept_early_refusal() {
        let (ssa, launch, entries) = RetainedContextEntriesV29::projection_test_fixture_v29();
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(7).unwrap();
        let first = observe_pending_v29(&entries, ssa, launch, &mut budget, |_, _| Ok(()));
        assert!(
            matches!(
                first,
                Err(ProductionPipelineError::PendingScopedObservationIncomplete)
            ),
            "{first:?}"
        );
        let exact_work = budget.work();
        let exact_storage = budget.peak_storage() - 7;
        for (work_limit, storage_limit, success) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
            (0, exact_storage, false),
            (exact_work, 0, false),
        ] {
            let (ssa, launch, entries) = RetainedContextEntriesV29::projection_test_fixture_v29();
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit + 7);
            budget.reserve_storage(7).unwrap();
            let mut visits = 0;
            let result = observe_pending_v29(&entries, ssa, launch, &mut budget, |_, _| {
                visits += 1;
                Ok(())
            });
            assert_eq!(
                matches!(
                    result,
                    Err(ProductionPipelineError::PendingScopedObservationIncomplete)
                ),
                success
            );
            assert_eq!(visits, usize::from(success));
            assert_eq!(budget.storage(), 7);
        }
    }
}
