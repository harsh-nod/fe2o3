#[test]
fn aggregate_source_binding_keeps_first_exact_refusal_through_later_queries() {
    let result =
        with_aggregate_source_owner_v30(private_entry_root_owner_v20, |source, _, budget| {
            let first = source.retain_aggregate_result_v30::<()>(Err(
                ProductionSourceOwnedViewErrorV18::Binding("first aggregate source binding").into(),
            ));
            let later = source.retain_aggregate_result_v30::<()>(Err(
                ProductionSourceOwnedViewErrorV18::Binding("later aggregate source binding").into(),
            ));
            assert!(matches!(
                later,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Binding("later aggregate source binding")
                ))
            ));
            let before = (budget.work(), budget.storage());
            assert!(matches!(
                source.check_query_v18(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "first aggregate source binding"
                ))
            ));
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(!source.cleanup.is_denied());
            first
        });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding("first aggregate source binding")
        ))
    ));
}

#[test]
fn aggregate_initial_source_nested_binding_keeps_exact_error_and_settles_credit() {
    let result =
        with_aggregate_source_owner_v30(private_entry_root_owner_v20, |source, abi, budget| {
            let floor = budget.storage();
            let handoff = source.aggregate_output_v30(abi, budget)?;
            let retained = budget.storage();
            let result = with_aggregate_initial_source_v30(
                source,
                handoff.output(budget)?,
                budget,
                |_, _, budget| {
                    budget.reserve_storage(11)?;
                    Err::<(), _>(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "exact nested aggregate binding",
                        )
                        .into(),
                    )
                },
            );
            assert!(matches!(
                &result,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Binding("exact nested aggregate binding")
                ))
            ));
            assert_eq!(budget.storage(), retained);
            assert!(!source.cleanup.is_denied());
            assert!(matches!(
                handoff.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "exact nested aggregate binding"
                ))
            ));
            assert_eq!(budget.storage(), floor);
            result
        });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding("exact nested aggregate binding")
        ))
    ));
}

fn with_aggregate_source_owner_v30(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    run: impl FnOnce(
        &ProductionSourceOwnedViewV18<'_>,
        ProductionKernelArgumentAbiInputV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30>,
) -> Result<(), ProductionAggregateSourceErrorV30> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, abi) = integer_handoff_prepared_v18(factory, &mut budget);
    let roots = abi.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        run(
            source,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )
    });
    assert_eq!(budget.storage(), MODULE_FLOOR);
    result
}

#[test]
fn aggregate_source_owner_retains_real_private_chain_and_exact_original_source() {
    for factory in [
        private_entry_root_owner_v20 as fn() -> _,
        private_entry_phi_owner_v20,
    ] {
        with_aggregate_source_owner_v30(factory, |source, abi, budget| {
            let floor = budget.storage();
            let original = source.source_ssa(budget)?;
            let handoff = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                budget,
            )?;
            handoff.check_original_source(original, budget)?;
            handoff.check_original_argument_abi_v30(abi, budget)?;
            handoff.replay(budget)?;
            let output = handoff.output(budget)?;
            assert_eq!((output.policy_version(), output.graph_schema()), (12, 18));
            assert_eq!(
                output.input_audit_bytes(),
                source.canonical(budget)?.canonical_bytes()
            );
            let version = output
                .canonical_bytes()
                .strip_prefix(b"fe2o3.aggregate-fixedpoint.v18.policy12\0")
                .expect("complete Policy12 witness domain")
                .first_chunk::<6>()
                .expect("policy, wire revision, and graph schema");
            assert_eq!(
                (
                    u16::from_le_bytes([version[0], version[1]]),
                    u16::from_le_bytes([version[2], version[3]]),
                    u16::from_le_bytes([version[4], version[5]]),
                ),
                (12, 2, 18)
            );
            assert!(!output.rounds().is_empty());
            assert!(!output.rounds().last().unwrap().changed());
            for pair in output.rounds().windows(2) {
                assert!(pair[0].changed());
                assert_eq!(
                    pair[1].scalar().input_audit_bytes(),
                    pair[0].aggregate().output().canonical_bytes()
                );
            }
            for round in output.rounds() {
                assert_eq!(round.scalar().execution().policy_version(), 11);
                assert_eq!(
                    round.aggregate().input_identity(),
                    round.scalar().owner().identity()
                );
            }
            assert!(!handoff.final_native_completion_is_complete());
            assert!(!handoff.executed_source_refinement_is_complete());
            assert!(!handoff.grants_artifact_or_launch_authority());
            assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
            handoff.discard(budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn aggregate_source_owner_rejects_equal_bytes_foreign_original_and_settles_owned_credit() {
    let foreign = private_entry_root_owner_v20();
    let result =
        with_aggregate_source_owner_v30(private_entry_root_owner_v20, |source, abi, budget| {
            let floor = budget.storage();
            let handoff = source.aggregate_output_v30(abi, budget)?;
            let error = handoff
                .check_original_source(&foreign, budget)
                .err()
                .unwrap();
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
            ));
            assert!(matches!(
                handoff.output(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "foreign original SSA owner"
                ))
            ));
            assert!(matches!(
                handoff.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "foreign original SSA owner"
                ))
            ));
            assert_eq!(budget.storage(), floor);
            Err(error.into())
        });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
        ))
    ));
}

#[test]
fn aggregate_source_owner_sticky_work_refusal_preserves_exact_error_after_disposal() {
    let result = with_aggregate_source_owner_v30(
        private_entry_root_owner_v20,
        |source, abi, budget| {
            let floor = budget.storage();
            let handoff = source.aggregate_output_v30(abi, budget)?;
            let left = OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work();
            let error = budget.charge_work(left + 1).unwrap_err();
            let selected = source.retain_query_resource_error_v18(error);
            assert!(
                matches!(handoff.output(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == error)
            );
            assert!(
                matches!(handoff.discard(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == error)
            );
            assert_eq!(budget.storage(), floor);
            Err(selected.into())
        },
    );
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_))
        ))
    ));
}

#[test]
fn aggregate_storage_policy_preserves_scalar_handoff_layout_and_credit() {
    struct Historical<'v, 's, O> {
        _source: &'v ProductionSourceOwnedViewV18<'s>,
        _output: O,
        _receipt: fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        _required: usize,
        _slot: usize,
        _ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    }
    macro_rules! same {
        ($policy:ty, $owner:ty) => {
            assert_eq!(
                size_of::<SourceOutputHandoffV18<'_, '_, $policy>>(),
                size_of::<Historical<'_, '_, $owner>>()
            );
            assert_eq!(
                std::mem::align_of::<SourceOutputHandoffV18<'_, '_, $policy>>(),
                std::mem::align_of::<Historical<'_, '_, $owner>>()
            );
            assert_eq!(
                source_output_handoff_credit_v18::<$policy>().unwrap(),
                size_of::<Historical<'_, '_, $owner>>()
                    - size_of::<$owner>()
                    - size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
                    + std::mem::align_of::<Historical<'_, '_, $owner>>()
            );
        };
    }
    same!(
        MixedPureCseSourceOptimizerV18,
        fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedPureCseV18
    );
    same!(
        MixedFixedpointSourceOptimizerV18,
        fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18
    );
    assert_ne!(
        std::any::TypeId::of::<ProductionAggregateSourceOutputHandoffV30<'static, 'static>>(),
        std::any::TypeId::of::<
            ProductionConditionalMixedFixedpointOutputHandoffV29<'static, 'static>,
        >()
    );
}

fn aggregate_source_resource_probe_v30(
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionAggregateSourceErrorV30>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, abi) = integer_handoff_prepared_v18(private_entry_root_owner_v20, &mut budget);
    let roots = abi.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let floor = budget.storage();
        let handoff = source.aggregate_output_v30(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        let replay = handoff.replay(budget);
        let settled = handoff.discard(budget);
        assert_eq!(budget.storage(), floor);
        replay?;
        settled?;
        Ok(())
    });
    assert_eq!(budget.storage(), MODULE_FLOOR);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn aggregate_source_owner_exact_and_one_short_complete_source_resources() {
    let (baseline, work, peak) =
        aggregate_source_resource_probe_v30(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    baseline.unwrap();
    let (exact, exact_work, exact_peak) = aggregate_source_resource_probe_v30(work, peak);
    exact.unwrap();
    assert_eq!((exact_work, exact_peak), (work, peak));
    for (work_limit, storage_limit, is_work) in [(work - 1, peak, true), (work, peak - 1, false)] {
        let error = aggregate_source_resource_probe_v30(work_limit, storage_limit)
            .0
            .err()
            .unwrap();
        let resource = aggregate_source_resource_v30(&error).expect("exact typed resource refusal");
        match (resource, is_work) {
            (ArgumentResourceV1::Work(error), true) => {
                assert_eq!((error.actual(), error.limit()), (work, work - 1))
            }
            (ArgumentResourceV1::Storage(error), false) => {
                assert_eq!((error.actual(), error.limit()), (peak, peak - 1))
            }
            other => panic!("wrong resource refusal: {other:?}"),
        }
    }
}

fn aggregate_nested_resource_errors_v30(
    resource: ArgumentResourceV1,
) -> Vec<ProductionAggregateSourceErrorV30> {
    use ProductionAggregateSourceErrorV30 as E;
    use ProductionSourceOwnedViewErrorV18 as S;
    use fe2o3_kernel_analysis::{
        CanonicalKirAggregateSsaErrorV18 as C, CanonicalKirInventoryErrorV1 as I,
        CanonicalKirMemorySsaErrorV1 as M, CanonicalKirPrivateMemoryErrorV1 as P,
        CanonicalKirSparseErrorV1 as Q, CanonicalKirTransitionErrorV1 as T,
    };
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as Verify,
        CanonicalKernelIrReplayAdmissionErrorV12 as K12,
        CanonicalKernelIrReplayAdmissionErrorV18 as K, KernelIrDecodeError as Decode,
        StorageLayoutErrorV1 as Layout,
    };
    use fe2o3_kernel_opt::{OwnedAggregateFixedpointErrorV18 as O, OwnedAggregateSsaErrorV18 as A};
    use fe2o3_pliron::{
        CanonicalAnalysisScopeErrorV1 as Scope, KirBridgeErrorV12 as B12, KirBridgeErrorV18 as B18,
        KirCheckedNeutralOptimizationErrorV1 as D, KirNeutralOptimizationErrorV18 as N,
        KirOptimizationMapErrorV12 as Map, PlironOptimizationErrorV12 as X,
    };
    vec![
        E::Source(S::Resource(resource)),
        E::Inventory(I::Resource(resource)),
        E::Transition(T::Resource(resource)),
        E::Optimization(O::Resource(resource)),
        E::Optimization(O::Inventory(I::Resource(resource))),
        E::Optimization(O::Transition(T::Resource(resource))),
        E::Optimization(O::Map(Map::Resources(resource))),
        E::Optimization(O::Adoption(D::Resource(resource))),
        E::Optimization(O::Adoption(D::Inventory(I::Resource(resource)))),
        E::Optimization(O::Adoption(D::Transition(T::Resource(resource)))),
        E::Optimization(O::Aggregate(A::Resource(resource))),
        E::Optimization(O::Aggregate(A::Inventory(I::Resource(resource)))),
        E::Optimization(O::Aggregate(A::Check(C::Resource(resource)))),
        E::Optimization(O::Aggregate(A::Check(C::Inventory(I::Resource(resource))))),
        E::Optimization(O::Aggregate(A::Admission(K::Resource(resource)))),
        E::Optimization(O::Aggregate(A::Admission(K::Decode(Decode::Resource(
            resource,
        ))))),
        E::Optimization(O::Aggregate(A::Admission(K::Layout(Layout::Resource(
            resource,
        ))))),
        E::Optimization(O::Aggregate(A::Admission(K::Verification(
            Verify::Resource(resource),
        )))),
        E::Optimization(O::Scalar(N::Resource(resource))),
        E::Optimization(O::Scalar(N::Mapping(Map::Resources(resource)))),
        E::Optimization(O::Scalar(N::Bridge(B18::Resource(resource)))),
        E::Optimization(O::Scalar(N::Bridge(B18::Canonical(K::Decode(
            Decode::Resource(resource),
        ))))),
        E::Optimization(O::Scalar(N::Execution(X::Resources(resource)))),
        E::Optimization(O::Scalar(N::Execution(X::Mapping(Map::Resources(
            resource,
        ))))),
        E::Optimization(O::Scalar(N::Execution(X::Bridge(B12::Resource(resource))))),
        E::Optimization(O::Scalar(N::Execution(X::Bridge(B12::Canonical(
            K12::Decode(Decode::Resource(resource)),
        ))))),
        E::Source(S::Analysis(Scope::Resource(resource))),
        E::Source(S::Analysis(Scope::Inventory(I::Resource(resource)))),
        E::Source(S::Analysis(Scope::Sparse(Q::Resource(resource)))),
        E::Source(S::Analysis(Scope::MemorySsa(M::Resource(resource)))),
        E::Source(S::PrivateMemory(P::Resource(resource))),
        E::Source(S::PrivateMemory(P::Inventory(I::Resource(resource)))),
    ]
}

#[test]
fn aggregate_source_typed_nested_resource_classification_keeps_every_payload() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = ArgumentBudgetV1::new(&mut work, 5);
    let work_error = budget.charge_work(4).unwrap_err();
    let storage_error = budget.reserve_storage(6).unwrap_err();
    for resource in [
        ArgumentResourceV1::Accounting,
        ArgumentResourceV1::Arithmetic,
        ArgumentResourceV1::Allocation,
        work_error,
        storage_error,
    ] {
        for error in aggregate_nested_resource_errors_v30(resource) {
            assert_eq!(
                aggregate_source_resource_v30(&error),
                Some(resource),
                "{error:?}"
            );
        }
    }
    use fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18 as O;
    use fe2o3_pliron::{
        KirCheckedNeutralOptimizationErrorV1 as D, KirNeutralOptimizationErrorV18 as N,
        PlironOptimizationErrorV12 as X,
    };
    for error in [
        O::Adoption(D::OriginAccounting),
        O::Scalar(N::Execution(X::Accounting)),
    ] {
        assert_eq!(
            aggregate_optimizer_resource_v30(&error),
            Some(ArgumentResourceV1::Accounting)
        );
    }
    for error in [
        O::ForeignInput,
        O::Inconsistent("relation"),
        O::Panicked,
        O::RoundLimit {
            completed: 32,
            limit: 32,
        },
    ] {
        assert_eq!(aggregate_optimizer_resource_v30(&error), None);
    }
}

#[test]
fn aggregate_initial_source_nested_accounting_denies_refund_before_outer_settlement() {
    for selected in aggregate_nested_resource_errors_v30(ArgumentResourceV1::Accounting) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, abi) =
            integer_handoff_prepared_v18(private_entry_root_owner_v20, &mut budget);
        let roots = abi.roots();
        let callback_floor = std::cell::Cell::new(0);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let handoff = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let outer_floor = budget.storage();
            let result = scoped_source_attempt_v29(source.cleanup, budget, outer_floor, |budget| {
                budget.reserve_storage(7)?;
                let result = with_aggregate_initial_source_v30(
                    source,
                    handoff.output(budget)?,
                    budget,
                    |_, _, budget| {
                        budget.reserve_storage(11)?;
                        callback_floor.set(budget.storage());
                        Err::<(), _>(selected)
                    },
                );
                assert!(source.cleanup.is_denied());
                assert_ne!(
                    callback_floor.get(),
                    0,
                    "the real nested source callback ran"
                );
                assert_eq!(
                    budget.storage(),
                    callback_floor.get(),
                    "inner scopes cannot refund first"
                );
                assert_eq!(
                    aggregate_source_resource_v30(result.as_ref().unwrap_err()),
                    Some(ArgumentResourceV1::Accounting)
                );
                result
            });
            assert_eq!(
                budget.storage(),
                callback_floor.get(),
                "outer attempt cannot refund"
            );
            drop(handoff);
            result
        });
        assert_eq!(
            aggregate_source_resource_v30(result.as_ref().unwrap_err()),
            Some(ArgumentResourceV1::Accounting)
        );
        assert_eq!(
            budget.storage(),
            callback_floor.get(),
            "source entrance cannot refund"
        );
        assert!(budget.storage() > MODULE_FLOOR);
    }
}

#[test]
fn aggregate_initial_source_nested_nonaccounting_refusal_settles_exact_credit() {
    let result =
        with_aggregate_source_owner_v30(private_entry_root_owner_v20, |source, abi, budget| {
            let floor = budget.storage();
            let handoff = source.aggregate_output_v30(abi, budget)?;
            let owned_floor = budget.storage();
            let result = with_aggregate_initial_source_v30(
                source,
                handoff.output(budget)?,
                budget,
                |_, _, budget| {
                    budget.reserve_storage(11)?;
                    Err::<(), _>(ProductionAggregateSourceErrorV30::Transition(
                        fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                            "selected callback refusal",
                        ),
                    ))
                },
            );
            assert!(!source.cleanup.is_denied());
            assert_eq!(budget.storage(), owned_floor);
            assert!(matches!(
                &result,
                Err(ProductionAggregateSourceErrorV30::Transition(
                    fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                        "selected callback refusal"
                    )
                ))
            ));
            assert!(matches!(
                handoff.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "actual source Policy12 chain refused"
                ))
            ));
            assert_eq!(budget.storage(), floor);
            result
        });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding("actual source Policy12 chain refused")
        ))
    ));
}
