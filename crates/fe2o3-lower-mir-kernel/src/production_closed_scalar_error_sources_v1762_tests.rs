use std::error::Error as _;

fn closed_resource_cause_v1762<'a>(
    mut error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a ArgumentResourceV1> {
    for _ in 0..32 {
        if let Some(resource) = error.downcast_ref::<ArgumentResourceV1>() {
            return Some(resource);
        }
        error = error.source()?;
    }
    panic!("error cause cycle")
}

#[test]
fn closed_scalar_error_chains_preserve_typed_resources_without_meter_effects() {
    use ProductionClosedScalarCheckErrorV18 as Check;
    use ProductionClosedScalarHandoffErrorV18 as Handoff;
    use ProductionSourceOptimizationErrorV18 as Optimization;
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryErrorV1 as Inventory, CanonicalKirTransitionErrorV1 as Transition,
        CanonicalRankedViewErrorV1 as Ranked,
    };
    use fe2o3_kernel_ir::{
        CanonicalKernelIrReplayAdmissionErrorV12 as Replay12,
        CanonicalKernelIrReplayAdmissionErrorV18 as Replay18,
    };
    use fe2o3_pliron::{
        KirBridgeErrorV12 as Bridge12, KirBridgeErrorV18 as Bridge18,
        KirCheckedNeutralOptimizationErrorV1 as Adoption,
        KirNeutralOptimizationErrorV18 as Observation, KirOptimizationMapErrorV12 as Map,
        PlironOptimizationErrorV12 as Execution,
    };

    let mut work = CanonicalKernelIrWorkBudgetV1::new(5);
    let mut budget = ArgumentBudgetV1::new(&mut work, 11);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(7).unwrap();
    let work_error = budget.charge_work(1).unwrap_err();
    let storage_error = budget.reserve_storage(5).unwrap_err();
    assert!(
        matches!(work_error, ArgumentResourceV1::Work(error) if (error.actual(), error.limit()) == (6, 5))
    );
    assert!(
        matches!(storage_error, ArgumentResourceV1::Storage(error) if (error.actual(), error.limit()) == (12, 11))
    );
    let history = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_work(),
        budget.failed_storage(),
        budget.work_ledger_identity_v1(),
    );
    for resource in [
        work_error,
        storage_error,
        ArgumentResourceV1::Allocation,
        ArgumentResourceV1::Accounting,
        ArgumentResourceV1::Arithmetic,
    ] {
        let source = || ProductionSourceOwnedViewErrorV18::Resource(resource);
        let errors = [
            Handoff::Check(Check::Source(source())),
            Handoff::Check(Check::Ranked(Ranked::Resource(resource))),
            Handoff::Check(Check::Native(
                ProductionSourceNativeLifecycleErrorV18::Source(source()),
            )),
            Handoff::Optimization(Optimization::Source(source())),
            Handoff::Optimization(Optimization::Observation(Observation::Resource(resource))),
            Handoff::Optimization(Optimization::Observation(Observation::Bridge(
                Bridge18::Resource(resource),
            ))),
            Handoff::Optimization(Optimization::Observation(Observation::Bridge(
                Bridge18::Canonical(Replay18::Resource(resource)),
            ))),
            Handoff::Optimization(Optimization::Observation(Observation::Execution(
                Execution::Resources(resource),
            ))),
            Handoff::Optimization(Optimization::Observation(Observation::Execution(
                Execution::Bridge(Bridge12::Resource(resource)),
            ))),
            Handoff::Optimization(Optimization::Observation(Observation::Execution(
                Execution::Bridge(Bridge12::Canonical(Replay12::Resource(resource))),
            ))),
            Handoff::Optimization(Optimization::Observation(Observation::Execution(
                Execution::Mapping(Map::Resources(resource)),
            ))),
            Handoff::Optimization(Optimization::Observation(Observation::Mapping(
                Map::Resources(resource),
            ))),
            Handoff::Optimization(Optimization::Adoption(Adoption::Resource(resource))),
            Handoff::Optimization(Optimization::Adoption(Adoption::Inventory(
                Inventory::Resource(resource),
            ))),
            Handoff::Optimization(Optimization::Adoption(Adoption::Transition(
                Transition::Resource(resource),
            ))),
            Handoff::Optimization(Optimization::Adoption(Adoption::Origin(Check::Source(
                source(),
            )))),
        ];
        for error in &errors {
            let cause = closed_resource_cause_v1762(error).unwrap();
            assert_eq!(*cause, resource);
            assert!(std::ptr::eq(
                cause,
                closed_resource_cause_v1762(error).unwrap()
            ));
            assert_eq!(error.to_string(), error.source().unwrap().to_string());
            match cause {
                ArgumentResourceV1::Work(expected) => {
                    let actual = cause
                        .source()
                        .unwrap()
                        .downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>()
                        .unwrap();
                    assert!(std::ptr::eq(actual, expected));
                }
                ArgumentResourceV1::Storage(expected) => {
                    let actual = cause.source().unwrap().downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrVerificationStorageLimitV1>().unwrap();
                    assert!(std::ptr::eq(actual, expected));
                }
                _ => assert!(cause.source().is_none()),
            }
            assert!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                    budget.work_ledger_identity_v1()
                ) == history,
                "error inspection must preserve budget history and ledger identity"
            );
        }
    }
}

#[test]
fn closed_scalar_generic_origin_error_is_borrowed_without_reclassification() {
    #[derive(Debug)]
    struct Selected(&'static str);
    impl std::fmt::Display for Selected {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(self.0)
        }
    }
    impl std::error::Error for Selected {}
    let error = ProductionSourceOptimizationErrorV18::Adoption(
        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(Selected(
            "selected original failure",
        )),
    );
    let ProductionSourceOptimizationErrorV18::Adoption(adoption) = &error else {
        unreachable!()
    };
    let fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(expected) = adoption else {
        unreachable!()
    };
    assert!(std::ptr::eq(
        error
            .source()
            .unwrap()
            .downcast_ref::<fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1<Selected>>()
            .unwrap(),
        adoption
    ));
    assert!(std::ptr::eq(
        adoption
            .source()
            .unwrap()
            .downcast_ref::<Selected>()
            .unwrap(),
        expected
    ));
    assert_eq!(
        error.to_string(),
        "checked neutral origin transport failed: selected original failure"
    );
    assert!(closed_resource_cause_v1762(&error).is_none());
}

#[test]
fn closed_scalar_nonresource_diagnostics_do_not_fabricate_resource_causes() {
    use ProductionClosedScalarCheckErrorV18 as Check;
    use fe2o3_pliron::{
        KirCheckedNeutralOptimizationErrorV1 as Adoption,
        KirNeutralOptimizationErrorV18 as Observation, PlironOptimizationErrorV12 as Execution,
    };
    for error in [
        Observation::Endpoint,
        Observation::Limit,
        Observation::Panicked,
    ] {
        assert!(error.source().is_none());
        assert!(!error.to_string().is_empty());
    }
    for error in [
        Adoption::<Check>::OriginAccounting,
        Adoption::<Check>::Panicked,
    ] {
        assert!(error.source().is_none());
    }
    for error in [Execution::AlreadyExecuted, Execution::Accounting] {
        assert!(error.source().is_none());
    }
    for error in [
        Observation::Pass(fe2o3_pliron::PlironOptimizationErrorV1::WorkLimitExceeded {
            required: 7,
            limit: 6,
        }),
        Observation::Execution(Execution::Execution(
            fe2o3_pliron::PlironOptimizationErrorV1::GraphWorkLimitExceeded {
                required: 9,
                limit: 8,
            },
        )),
    ] {
        let mut cause = error.source().unwrap();
        if let Some(execution) = cause.downcast_ref::<Execution>() {
            cause = execution.source().unwrap();
        }
        assert!(matches!(
            cause.downcast_ref::<fe2o3_pliron::PlironOptimizationErrorV1>(),
            Some(fe2o3_pliron::PlironOptimizationErrorV1::WorkLimitExceeded {
                required: 7,
                limit: 6
            }) | Some(
                fe2o3_pliron::PlironOptimizationErrorV1::GraphWorkLimitExceeded {
                    required: 9,
                    limit: 8
                }
            )
        ));
        assert!(closed_resource_cause_v1762(&error).is_none());
    }
    assert!(
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner
            .source()
            .is_none()
    );
    let error = ProductionClosedScalarHandoffErrorV18::Check(Check::Unsupported(
        "not a closed scalar output",
    ));
    let check = error.source().unwrap().downcast_ref::<Check>().unwrap();
    assert!(check.source().is_none());
    assert!(closed_resource_cause_v1762(&error).is_none());
    assert_eq!(
        error.to_string(),
        "closed scalar handoff check: not a closed scalar output"
    );
}
