fn run_original_repeated_source_v29(
    factory: impl Fn() -> ProductionSemanticSsaOwnerV1,
    observer: ScopedSlotObserverV29,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    run_original_source_fixture_v29(factory, false, true, 1, observer, work_limit, storage_limit)
}

pub(in super::super) fn run_original_source_fixture_v29(
    factory: impl Fn() -> ProductionSemanticSsaOwnerV1,
    branches: bool,
    repeated: bool,
    minimum_effects: usize,
    observer: ScopedSlotObserverV29,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let projection = factory();
    let owner = factory();
    assert_eq!(
        owner.source_semantic_sha256(),
        projection.source_semantic_sha256()
    );
    assert_eq!(owner.identity(), projection.identity());
    assert!(owner.occurrence_storage().is_none());
    let semantic = projection.source_semantic();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let roots = [root_input(&projection)];
    let mut classes = vec![
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Provider {
            function: HELPER,
            identity: semantic.functions()[1].identity(),
        },
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Derive {
            binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
            operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
            context: CONTEXT,
            workgroup: semantic.functions()[2].abi().source_input_types()[0],
        },
    ];
    if repeated {
        classes.insert(3, ProductionScopeCallableCandidateV29::Ordinary);
    }
    let SemanticTerminatorKindV1::Call(derive) =
        semantic.functions()[1].blocks()[0].terminator().kind()
    else {
        panic!("original repeated fixture derive call");
    };
    let mut events = vec![
        (
            ROOT,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        ),
        (
            HELPER,
            0,
            semantic.functions()[1].blocks()[0].statements().len(),
            ProductionScopeEventKindV29::Call {
                callee: derive.callee(),
                kind: ProductionScopeCallKindV29::Derive,
            },
        ),
        (
            HELPER,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(2),
                kind: ProductionScopeCallKindV29::Ordinary,
            },
        ),
    ];
    for block in if branches { &[3, 4][..] } else { &[2][..] } {
        events.push((HELPER, *block, 0, ProductionScopeEventKindV29::Return));
    }
    let events: Vec<_> = events
        .into_iter()
        .map(
            |(function, block, statement_count, kind)| crate::ProductionScopeEventCandidateV29 {
                function,
                block: SemanticBlockIdV1::from_index(block),
                statement_count,
                kind,
            },
        )
        .collect();
    let _observer = ObserverGuard::install(observer);
    OBSERVED.set(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = (|| -> SourceOwnedResultV18<()> {
        let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: projection.source_semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )?;
        // The observer mutates the candidate consumed by assembly. It stays
        // installed through reconstruction and immutable source replay.
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_sparse_and_memory_ssa_v1(|_, versions, budget| {
                    let inventory = versions.inventory();
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        scoped_raw_admission_v29::with_checked_source_memory_v29(
                            relation,
                            0,
                            Some(versions),
                            budget,
                            |physical, budget| -> SourceOwnedResultV18<()> {
                                let mut effects = 0;
                                physical.visit_effects(budget, |_, _| {
                                    effects += 1;
                                    Ok(())
                                })?;
                                assert!(
                                    effects >= minimum_effects,
                                    "the original fixture must retain its physical memory census"
                                );
                                completed = true;
                                Ok(())
                            },
                        )
                    })
                })
            })
        })
    })();
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    let completed = completed && result.is_ok();
    (result, budget.work(), budget.peak_storage(), completed)
}

pub(in super::super) fn original_repeated_source_resource_v29(
    error: ProductionSourceOwnedViewErrorV18,
) -> ArgumentResourceV1 {
    use ProductionPendingScopedSourceErrorV29 as Pending;
    use ProductionSourceOwnedViewErrorV18 as View;
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as V, CanonicalKernelIrReplayAdmissionErrorV18 as C,
        KernelIrDecodeError as D, KernelIrEncodeError as E, StorageLayoutErrorV1 as L,
    };
    match error {
        View::Resource(error)
        | View::Source(Pending::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
        ))
        | View::Source(Pending::Source(ProductionSemanticKirErrorV1::AssertOrigin(
            SemanticKirAssertOriginErrorV1::Resource(error),
        )))
        | View::Source(Pending::Canonical(C::Resource(error)))
        | View::Source(Pending::Canonical(C::Decode(D::Resource(error))))
        | View::Source(Pending::Canonical(C::Layout(L::Resource(error))))
        | View::Source(Pending::Canonical(C::Verification(V::Resource(error))))
        | View::Source(Pending::Occurrences(
            fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error),
        )) => error,
        View::Source(Pending::Canonical(C::Encode(E::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::Encode(E::WorkLimit(limit))))) => {
            ArgumentResourceV1::Work(limit)
        }
        other => panic!("exact original-source resource refusal required: {other:?}"),
    }
}
