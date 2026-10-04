use super::*;
use crate::{
    CanonicalFormalLaunchInputV19 as Launch, CanonicalFormalReportErrorV19 as Error,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    with_canonical_owner_formal_report_v19 as report,
};

#[path = "canonical_report_source_v20_tests.rs"]
mod retained_source_tests;

fn public_run(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    input: Launch,
    width: FormalIndexWidth,
    repeats: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), Error>,
    Vec<FormalMemoryObligationAnalysis>,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut observations = Vec::new();
    let mut entered = false;
    let mut completed = false;
    let mut observed = None;
    let effects = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
        entered = true;
        let floor = budget.storage();
        for _ in 0..repeats {
            let result = report(
                owner,
                0,
                effects,
                input,
                width,
                ControlFlowLimits::DEFAULT,
                budget,
                |view, _| {
                    assert!(std::ptr::eq(view.original_owner(), owner));
                    assert!(std::ptr::eq(
                        view.original_function(),
                        &owner.module().functions[0]
                    ));
                    assert_eq!(view.root_index(), 0);
                    assert_eq!(view.launch_input(), input);
                    assert_eq!(view.index_width(), width);
                    observations.push(view.analysis().clone());
                    Ok(())
                },
            );
            assert_eq!(budget.storage(), floor);
            let denied = result.is_err();
            observed = Some(result);
            if denied {
                break;
            }
        }
        completed = true;
        Ok(())
    });
    assert!(
        !entered || completed,
        "post-denial assertions must complete"
    );
    let result = match (observed, effects) {
        (_, Err(error)) => Err(Error::Effects(error)),
        (Some(result), Ok(())) => result,
        (None, Ok(())) => panic!("missing report attempt"),
    };
    assert_eq!(budget.storage(), FLOOR);
    (result, observations, budget.work(), budget.peak_storage())
}

#[test]
fn public_report_uses_actual_owner_and_preserves_full_exact_physical_reports() {
    for stores in [0, 1, 2, 7] {
        let mut module = store_fixture(stores, false);
        module.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(2),
        };
        with_owner(&module, |owner| {
            for (input, interpretation) in [
                (Launch::Exact(launch(2)), Interpretation::Exact),
                (Launch::Exact(launch(8)), Interpretation::Exact),
                (
                    Launch::PhysicalEnvelope(launch(8)),
                    Interpretation::Envelope,
                ),
                (
                    Launch::Exact(ExplicitLaunchExtent::Unknown),
                    Interpretation::Exact,
                ),
            ] {
                let extent = match input {
                    Launch::Exact(e) | Launch::PhysicalEnvelope(e) => e,
                };
                for width in [
                    FormalIndexWidth::Unknown,
                    FormalIndexWidth::Bits32,
                    FormalIndexWidth::Bits64,
                ] {
                    let expected = legacy(owner, extent, width, interpretation);
                    let (result, reports, _, _) = public_run(owner, input, width, 2, LIMIT, LIMIT);
                    assert_eq!(result, Ok(()));
                    assert_eq!(reports, [expected.clone(), expected]);
                }
            }
        });
    }
}

#[test]
fn public_report_full_chain_has_exact_and_one_short_cumulative_bounds() {
    with_owner(&store_fixture(2, false), |owner| {
        let input = Launch::Exact(launch(8));
        let full = public_run(owner, input, FormalIndexWidth::Bits64, 2, LIMIT, LIMIT);
        assert_eq!(full.0, Ok(()));
        let exact = public_run(owner, input, FormalIndexWidth::Bits64, 2, full.2, full.3);
        assert_eq!(exact.0, Ok(()));
        assert_eq!(exact.1, full.1);
        for (work_limit, storage_limit, work_denial) in
            [(full.2 - 1, full.3, true), (full.2, full.3 - 1, false)]
        {
            let failed = public_run(
                owner,
                input,
                FormalIndexWidth::Bits64,
                2,
                work_limit,
                storage_limit,
            );
            match failed.0 {
                Err(Error::Effects(CanonicalEffectErrorV19::Resource(Resource::Work(error))))
                    if work_denial =>
                {
                    assert!(error.actual() > error.limit());
                    assert_eq!(error.limit(), work_limit);
                }
                Err(Error::Effects(CanonicalEffectErrorV19::Resource(Resource::Storage(
                    error,
                )))) if !work_denial => {
                    assert!(error.actual() > error.limit());
                    assert_eq!(error.limit(), storage_limit);
                }
                error => panic!("wrong exact first denial: {error:?}"),
            }
        }
    });
}

#[test]
fn public_report_rejects_equal_bytes_foreign_effects_and_missing_root() {
    let module = store_fixture(1, false);
    with_owner(&module, |owner| {
        with_owner(&module, |foreign| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut completed = false;
            let outer = with_canonical_effects_v19(foreign, &mut budget, |effects, budget| {
                let floor = budget.storage();
                let result = report(
                    owner,
                    0,
                    effects,
                    Launch::Exact(launch(8)),
                    FormalIndexWidth::Bits64,
                    ControlFlowLimits::DEFAULT,
                    budget,
                    |_, _| panic!("foreign scope must not consume"),
                );
                assert_eq!(
                    result,
                    Err(Error::Effects(CanonicalEffectErrorV19::ForeignOwner))
                );
                assert_eq!(budget.storage(), floor);
                completed = true;
                Ok(())
            });
            assert!(completed);
            assert_eq!(outer, Err(CanonicalEffectErrorV19::ForeignOwner));
            assert_eq!(budget.storage(), FLOOR);
            let mut completed = false;
            let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
                let floor = budget.storage();
                let result = report(
                    owner,
                    usize::MAX,
                    effects,
                    Launch::Exact(launch(8)),
                    FormalIndexWidth::Bits64,
                    ControlFlowLimits::DEFAULT,
                    budget,
                    |_, _| panic!("bad root must not consume"),
                );
                assert_eq!(result, Err(Error::Resource(ResourceError::Accounting)));
                assert_eq!(budget.storage(), floor);
                completed = true;
                Ok(())
            });
            assert!(completed);
            assert_eq!(outer, Ok(()));
            assert_eq!(budget.storage(), FLOOR);
        })
    });
}

#[test]
fn public_report_preserves_consumer_payload_and_caught_panic_cleanup() {
    with_owner(&store_fixture(1, false), |owner| {
        for panic in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut completed = false;
            let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
                let floor = budget.storage();
                let result = report(
                    owner,
                    0,
                    effects,
                    Launch::Exact(launch(8)),
                    FormalIndexWidth::Bits64,
                    ControlFlowLimits::DEFAULT,
                    budget,
                    |_, _| {
                        if panic {
                            panic!("trusted report callback");
                        }
                        Err(Error::Invocation(RegionValidationError::InvalidAlignment {
                            alignment: 3,
                        }))
                    },
                );
                assert_eq!(
                    result,
                    Err(if panic {
                        Error::Panicked
                    } else {
                        Error::Invocation(RegionValidationError::InvalidAlignment { alignment: 3 })
                    })
                );
                assert_eq!(budget.storage(), floor);
                completed = true;
                Ok(())
            });
            assert!(completed);
            assert_eq!(outer, Ok(()));
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn public_report_keeps_swallowed_resource_denial_ahead_of_undercut() {
    with_owner(&store_fixture(2, false), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut completed = false;
        let mut undercut_completed = false;
        let mut first = None;
        let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
            let result = report(
                owner,
                0,
                effects,
                Launch::Exact(launch(8)),
                FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT,
                budget,
                |_, budget| {
                    let error = budget.charge_work(LIMIT).unwrap_err();
                    first = Some(error);
                    let before = budget.storage();
                    budget.release_storage(1).unwrap();
                    assert_eq!(budget.storage(), before - 1);
                    undercut_completed = true;
                    Ok(())
                },
            );
            assert_eq!(result, Err(Error::Resource(first.unwrap().into())));
            completed = true;
            Ok(())
        });
        assert!(completed);
        assert!(undercut_completed);
        assert_eq!(
            outer,
            Err(CanonicalEffectErrorV19::Resource(first.unwrap()))
        );
    });
}

#[test]
fn public_report_foreign_budget_does_not_poison_original_effect_scope() {
    with_owner(&store_fixture(1, false), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut completed = false;
        let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
            let before = (budget.work(), budget.storage());
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut other = Budget::new(&mut other_work, LIMIT);
            other.reserve_storage(FLOOR).unwrap();
            let refused = report(
                owner,
                0,
                effects,
                Launch::Exact(launch(8)),
                FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT,
                &mut other,
                |_, _| panic!("foreign-budget consumer"),
            );
            assert_eq!(
                refused,
                Err(Error::Effects(CanonicalEffectErrorV19::Resource(
                    Resource::Accounting
                )))
            );
            assert_eq!(other.storage(), FLOOR);
            assert_eq!((budget.work(), budget.storage()), before);
            let accepted = report(
                owner,
                0,
                effects,
                Launch::Exact(launch(8)),
                FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT,
                budget,
                |_, _| Ok(()),
            );
            assert_eq!(accepted, Ok(()));
            assert_eq!(budget.storage(), before.1);
            completed = true;
            Ok(())
        });
        assert!(completed);
        assert_eq!(outer, Ok(()));
    });
}

#[test]
fn public_report_does_not_refund_callback_owned_surplus_as_analysis_backing() {
    with_owner(&store_fixture(1, false), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut completed = false;
        let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
            let floor = budget.storage();
            let refused = report(
                owner,
                0,
                effects,
                Launch::Exact(launch(8)),
                FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT,
                budget,
                |_, budget| {
                    budget.reserve_storage(37).unwrap();
                    Ok(())
                },
            );
            assert_eq!(refused, Err(Error::Resource(ResourceError::Accounting)));
            assert_eq!(budget.storage(), floor + 37);
            budget.release_storage(37).unwrap();
            completed = true;
            Ok(())
        });
        assert!(completed);
        assert_eq!(outer, Ok(()));
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn public_report_prior_denial_survives_panicking_rejected_capture() {
    struct Dropping<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Dropping<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("trusted rejected capture");
        }
    }
    with_owner(&store_fixture(1, false), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let dropped = std::cell::Cell::new(0);
        let mut first = None;
        let mut completed = false;
        let outer = with_canonical_effects_v19(owner, &mut budget, |effects, budget| {
            first = Some(budget.charge_work(LIMIT).unwrap_err());
            let retained = budget.storage();
            let capture = Dropping(&dropped);
            let refused = report(
                owner,
                0,
                effects,
                Launch::Exact(launch(8)),
                FormalIndexWidth::Bits64,
                ControlFlowLimits::DEFAULT,
                budget,
                move |_, _| {
                    drop(capture);
                    panic!("prior-denied consumer cannot run");
                },
            );
            assert_eq!(refused, Err(Error::Resource(first.unwrap().into())));
            assert_eq!(budget.storage(), retained);
            assert_eq!(dropped.get(), 1);
            completed = true;
            Ok(())
        });
        assert!(completed);
        assert_eq!(
            outer,
            Err(CanonicalEffectErrorV19::Resource(first.unwrap()))
        );
        assert_eq!(dropped.get(), 1);
    });
}

#[test]
fn public_report_error_sources_retain_exact_effect_and_resource_causes() {
    use std::error::Error as _;

    let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
    let mut budget = Budget::new(&mut work, 11);
    let work = budget.charge_work(8).unwrap_err();
    let storage = budget.reserve_storage(12).unwrap_err();
    for resource in [
        work,
        storage,
        Resource::Allocation,
        Resource::Accounting,
        Resource::Arithmetic,
    ] {
        let error = Error::Effects(CanonicalEffectErrorV19::Resource(resource));
        let effects = error.source().unwrap();
        assert_eq!(
            effects.downcast_ref::<CanonicalEffectErrorV19>(),
            Some(&CanonicalEffectErrorV19::Resource(resource))
        );
        let cause = effects.source().unwrap();
        assert_eq!(cause.downcast_ref::<Resource>(), Some(&resource));
        match resource {
            Resource::Work(expected) => {
                assert_eq!(
                    cause
                        .source()
                        .unwrap()
                        .downcast_ref::<crate::CanonicalKernelIrWorkLimitV1>(),
                    Some(&expected)
                );
            }
            Resource::Storage(expected) => {
                assert_eq!(
                    cause
                        .source()
                        .unwrap()
                        .downcast_ref::<crate::CanonicalKernelIrVerificationStorageLimitV1>(),
                    Some(&expected)
                );
            }
            Resource::Allocation | Resource::Accounting | Resource::Arithmetic => {
                assert!(cause.source().is_none());
            }
        }
        let direct = Error::Resource(resource.into());
        assert_eq!(
            direct
                .source()
                .unwrap()
                .downcast_ref::<FormalGuardedMemoryResourceErrorV1>(),
            Some(&resource.into())
        );
    }
    let flow = crate::ControlFlowError::EmptyFunction;
    assert_eq!(
        Error::ControlFlow(flow.clone())
            .source()
            .unwrap()
            .downcast_ref::<crate::ControlFlowError>(),
        Some(&flow)
    );
    let invocation = RegionValidationError::UnboundedExpression;
    assert_eq!(
        Error::Invocation(invocation)
            .source()
            .unwrap()
            .downcast_ref::<RegionValidationError>(),
        Some(&invocation)
    );
    for effect in [
        CanonicalEffectErrorV19::ForeignOwner,
        CanonicalEffectErrorV19::Consumer,
        CanonicalEffectErrorV19::Panicked,
    ] {
        assert!(effect.source().is_none());
        assert_eq!(
            Error::Effects(effect)
                .source()
                .unwrap()
                .downcast_ref::<CanonicalEffectErrorV19>(),
            Some(&effect)
        );
    }
    assert!(Error::ConsumerRejected.source().is_none());
    assert!(Error::Panicked.source().is_none());
}
