fn aggregate_runtime_launches_v30(
    source: &ProductionSourceOwnedViewV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Vec<fe2o3_kernel_ir::ExplicitLaunchExtent> {
    vec![
        fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1]
        };
        source.root_count(budget).unwrap()
    ]
}

fn with_aggregate_global_runtime_v30(
    looping: bool,
    run: impl FnOnce(
        &ProductionSourceOwnedViewV18<'_>,
        ProductionKernelArgumentAbiInputV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30>,
) -> Result<(), ProductionAggregateSourceErrorV30> {
    let owner = mixed_licm_source_v28(looping);
    let abi = issued_descriptor_role_abi_v18(&owner);
    let semantic = owner.source_semantic();
    let launch_roots: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_roots).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let roots = abi.roots();
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 256 << 20);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
            owner,
            launch,
            input,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
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
fn aggregate_runtime_rejoins_genuine_external_occurrences_after_every_actual_stage() {
    for looping in [false, true] {
        with_aggregate_global_runtime_v30(looping, |source, abi, budget| {
            let floor = budget.storage();
            let launches = aggregate_runtime_launches_v30(source, budget);
            let chain = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                budget,
            )?;
            let completed = chain.complete_native_v30(
                abi,
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            let owner = completed.output(budget)?.owner();
            let (inventory, credit) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(owner, budget)
                    .map_err(ProductionAggregateSourceErrorV30::Inventory)?;
            budget.reserve_storage(credit.retained_storage())?;
            let occurrences = completed.runtime_occurrences(budget)?;
            assert!(!occurrences.is_empty());
            assert!(occurrences.iter().any(|row| row.domain().writing()));
            assert!(occurrences.iter().any(|row| !row.domain().writing()));
            assert!(
                occurrences
                    .iter()
                    .any(|row| row.original_operation() != row.output_operation()
                        || row.original_address_formation() != row.output_address_formation())
            );
            for occurrence in occurrences {
                let at = aggregate_operation_index_v30(
                    &inventory,
                    occurrence.output_operation(),
                    budget,
                )?;
                let actual = inventory.operations()[at].operation;
                assert!(matches!(
                    actual.kind,
                    OperationKind::Load { .. } | OperationKind::Store { .. }
                ));
                assert_eq!(
                    occurrence.domain().writing(),
                    matches!(actual.kind, OperationKind::Store { .. })
                );
                assert!(occurrence.premise_index() < completed.runtime_premises(budget)?.len());
            }
            drop(inventory);
            budget.release_storage(credit.retained_storage())?;
            assert_eq!(
                completed
                    .native_histories(budget)?
                    .iter()
                    .filter(|row| row.is_some())
                    .count(),
                owner
                    .module()
                    .functions
                    .iter()
                    .filter(|function| function.body.is_some())
                    .count()
            );
            completed.discard(budget)?;
            chain.discard(budget)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn aggregate_runtime_final_consumer_rejects_incomplete_foreign_and_missing_occurrence_state() {
    for fault in [1, 2, 3, 4, 5] {
        let result = with_aggregate_global_runtime_v30(false, |source, abi, budget| {
            let launches = aggregate_runtime_launches_v30(source, budget);
            let chain = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                budget,
            )?;
            let baseline = chain.complete_native_v30(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            assert!(!baseline.runtime_occurrences(budget)?.is_empty());
            baseline.discard(budget)?;
            let error = match chain.complete_native_inner_v30(
                abi,
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
                Some(fault),
            ) {
                Err(error) => error,
                Ok(_) => panic!("mutated final aggregate state admitted"),
            };
            let expected = match fault {
                1 | 2 => "aggregate final source role owner or census",
                3 => "aggregate complete external chain census",
                4 | 5 => "aggregate final external definition lacks complete source transport",
                _ => unreachable!(),
            };
            assert!(
                matches!(&error, ProductionAggregateSourceErrorV30::Source(ProductionSourceOwnedViewErrorV18::Binding(message)) if *message == expected)
            );
            let _ = chain.discard(budget);
            Err(error)
        });
        let expected = match fault {
            1 | 2 => "aggregate final source role owner or census",
            3 => "aggregate complete external chain census",
            4 | 5 => "aggregate final external definition lacks complete source transport",
            _ => unreachable!(),
        };
        assert!(
            matches!(result, Err(ProductionAggregateSourceErrorV30::Source(ProductionSourceOwnedViewErrorV18::Binding(message))) if message == expected)
        );
    }
}

#[test]
fn aggregate_runtime_completes_actual_private_stages_and_final_native_reports() {
    for factory in [
        private_entry_root_owner_v20 as fn() -> _,
        private_entry_phi_owner_v20,
    ] {
        with_aggregate_source_owner_v30(factory, |source, abi, budget| {
            let floor = budget.storage();
            let launches = aggregate_runtime_launches_v30(source, budget);
            let original = source.source_ssa(budget)?;
            let chain = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                budget,
            )?;
            let chain_floor = budget.storage();
            let result = (|| {
                let completed = chain.complete_native_v30(
                    abi,
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                )?;
                completed.check_original_source(original, budget)?;
                let actual = completed.output(budget)?;
                assert!(std::ptr::eq(actual, chain.output(budget)?));
                assert_eq!(actual.policy_version(), 12);
                assert!(actual.rounds().iter().any(|round| {
                    round.aggregate().witness().actions().iter().any(|action| {
                        !matches!(
                            action,
                            fe2o3_kernel_analysis::CanonicalKirAggregateSsaActionV18::Retain
                        )
                    })
                }));
                assert_eq!(
                    completed.native_histories(budget)?.len(),
                    actual.owner().module().functions.len()
                );
                assert!(
                    completed
                        .native_histories(budget)?
                        .iter()
                        .all(Option::is_some)
                );
                assert!(completed.runtime_occurrences(budget)?.is_empty());
                assert!(completed.runtime_premises(budget)?.is_empty());
                assert!(completed.source_roles_are_complete());
                assert!(completed.final_native_completion_is_complete());
                assert!(!completed.executed_source_refinement_is_complete());
                assert!(!completed.runtime_requirements_are_discharged());
                assert!(!completed.grants_artifact_or_launch_authority());
                assert_eq!(
                    budget.storage(),
                    chain_floor + completed.retained_storage(budget)?
                );
                completed.discard(budget)?;
                assert_eq!(budget.storage(), chain_floor);
                Ok::<_, ProductionAggregateSourceErrorV30>(())
            })();
            let released = chain.discard(budget);
            result?;
            released?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn aggregate_runtime_stage_fold_visits_the_entire_ordered_nominal_chain() {
    with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
        let floor = budget.storage();
        let chain = source.aggregate_output_v30(abi, budget)?;
        let retained_floor = budget.storage();
        let stages = chain.output(budget)?.rounds().len() * 2;
        let mut visited = 0usize;
        let transported = fold_aggregate_source_stages_v30(
            &chain,
            |stage, prior, budget| {
                assert_eq!(stage.ordinal, visited);
                visited += 1;
                let state = match prior {
                    Some(state) => state,
                    None => AggregateDefinitionTransportV30::seed(stage.input, &[], budget)?,
                };
                state.advance(stage, budget)
            },
            budget,
        )?;
        assert_eq!(visited, stages);
        assert_eq!(transported.next_stage, stages);
        assert_eq!(
            transported.owner,
            std::ptr::from_ref(chain.output(budget)?.owner()) as usize
        );
        let credit = transported.retained_storage()?;
        drop(transported);
        budget.release_storage(credit)?;
        assert_eq!(budget.storage(), retained_floor);
        chain.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
}

#[test]
fn aggregate_runtime_stage_transport_rejects_wrong_owner_and_reordered_stage() {
    for wrong_owner in [false, true] {
        let result =
            with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
                let chain = source.aggregate_output_v30(abi, budget)?;
                let result = fold_aggregate_source_stages_v30(
                    &chain,
                    |stage, prior: Option<AggregateDefinitionTransportV30>, budget| {
                        let mut state = match prior {
                            Some(state) => state,
                            None => {
                                AggregateDefinitionTransportV30::seed(stage.input, &[], budget)?
                            }
                        };
                        if stage.ordinal == 0 {
                            if wrong_owner {
                                state.owner = 0;
                            } else {
                                state.next_stage += 1;
                            }
                        }
                        state.advance(stage, budget)
                    },
                    budget,
                );
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("foreign or reordered aggregate stage admitted"),
                };
                assert!(matches!(
                    &error,
                    ProductionAggregateSourceErrorV30::Source(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate definition transport owner or stage order"
                        )
                    )
                ));
                let _ = chain.discard(budget);
                Err(error)
            });
        assert!(matches!(
            result,
            Err(ProductionAggregateSourceErrorV30::Source(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate definition transport owner or stage order"
                )
            ))
        ));
    }
}

#[test]
fn aggregate_runtime_stage_unwind_drops_owned_transport_before_scope_refund() {
    struct Tracked<'a> {
        transport: Option<AggregateDefinitionTransportV30>,
        dropped: &'a std::cell::Cell<usize>,
    }
    impl AggregateStageStateV30 for Tracked<'_> {
        fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
            self.transport.as_ref().unwrap().retained_storage()
        }
    }
    impl Drop for Tracked<'_> {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
        }
    }
    with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
        let floor = budget.storage();
        let chain = source.aggregate_output_v30(abi, budget)?;
        let chain_floor = budget.storage();
        let dropped = std::cell::Cell::new(0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fold_aggregate_source_stages_v30(
                &chain,
                |stage, prior: Option<Tracked<'_>>, budget| {
                    if stage.ordinal == 1 {
                        assert!(prior.is_some());
                        panic!("aggregate stage unwind sentinel");
                    }
                    assert_eq!(stage.ordinal, 0);
                    assert!(prior.is_none());
                    let coordinate = stage.input.definitions().first().unwrap().coordinate;
                    let transport =
                        AggregateDefinitionTransportV30::seed(stage.input, &[coordinate], budget)?
                            .advance(stage, budget)?;
                    assert!(transport.retained_storage()? > 0);
                    Ok(Tracked {
                        transport: Some(transport),
                        dropped: &dropped,
                    })
                },
                budget,
            )
        }));
        let payload = match caught {
            Err(payload) => payload,
            Ok(_) => panic!("stage panic did not propagate"),
        };
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"aggregate stage unwind sentinel")
        );
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), chain_floor);
        chain.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
}

fn aggregate_runtime_resource_probe_v30(
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionAggregateSourceErrorV30>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_phi_owner_v20, &mut budget);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let launches = aggregate_runtime_launches_v30(source, budget);
        let chain = source.aggregate_output_v30(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        let result = chain.complete_native_v30(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &launches,
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            budget,
        );
        let result = match result {
            Ok(completed) => completed.discard(budget).map_err(Into::into),
            Err(error) => Err(error),
        };
        let released = chain.discard(budget);
        result?;
        released?;
        Ok(())
    });
    assert_eq!(budget.storage(), MODULE_FLOOR);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn aggregate_runtime_complete_native_has_exact_and_one_short_resources() {
    let measured = aggregate_runtime_resource_probe_v30(1_000_000_000, 256 << 20);
    measured.0.unwrap();
    let (work, storage) = (measured.1, measured.2);
    let exact = aggregate_runtime_resource_probe_v30(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    for (w, s, work_short) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let error = aggregate_runtime_resource_probe_v30(w, s).0.unwrap_err();
        match (work_short, aggregate_source_resource_v30(&error)) {
            (true, Some(ArgumentResourceV1::Work(limit))) => {
                assert_eq!((limit.actual(), limit.limit()), (work, work - 1))
            }
            (false, Some(ArgumentResourceV1::Storage(limit))) => {
                assert_eq!((limit.actual(), limit.limit()), (storage, storage - 1))
            }
            _ => panic!("exact complete native resource refusal required: {error:?}"),
        }
    }
}

#[test]
fn aggregate_runtime_sticky_work_refusal_settles_only_its_owned_rows() {
    let result =
        with_aggregate_source_owner_v30(private_entry_root_owner_v20, |source, abi, budget| {
            let source_floor = budget.storage();
            let launches = aggregate_runtime_launches_v30(source, budget);
            let chain = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                budget,
            )?;
            let chain_floor = budget.storage();
            let completed = chain.complete_native_v30(
                abi,
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            let credit = completed.retained_storage(budget)?;
            assert_eq!(budget.storage(), chain_floor + credit);
            let remaining = OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work();
            let refused = budget.charge_work(remaining + 1).unwrap_err();
            let selected = source.retain_query_resource_error_v18(refused);
            assert!(matches!(completed.runtime_occurrences(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == refused));
            assert!(matches!(completed.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == refused));
            assert_eq!(budget.storage(), chain_floor);
            assert!(matches!(chain.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(actual)) if actual == refused));
            assert_eq!(budget.storage(), source_floor);
            Err(selected.into())
        });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_))
        ))
    ));
}

#[test]
fn aggregate_runtime_funded_foreign_account_cannot_query_or_refund_credit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_root_owner_v20, &mut budget);
    let roots = fixture.roots();
    let retained = std::cell::Cell::new(0);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let launches = aggregate_runtime_launches_v30(source, budget);
        let chain = source.aggregate_output_v30(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        let completed = chain.complete_native_v30(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &launches,
            fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            budget,
        )?;
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        foreign.reserve_storage(budget.storage()).unwrap();
        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
        assert!(matches!(
            completed.runtime_occurrences(&foreign),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(matches!(
            completed.discard(&mut foreign),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!(
            (foreign.work(), foreign.storage(), foreign.peak_storage()),
            before
        );
        assert!(source.cleanup.is_denied());
        retained.set(budget.storage());
        drop(chain);
        Err::<(), _>(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting),
        ))
    });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ))
    ));
    assert!(retained.get() > MODULE_FLOOR);
    assert!(budget.storage() >= retained.get());
}

fn aggregate_runtime_nested_errors_v30(
    resource: ArgumentResourceV1,
) -> Vec<ProductionAggregateSourceErrorV30> {
    use ProductionAggregateSourceErrorV30 as E;
    use ProductionMixedSourceCheckErrorV26 as M;
    use ProductionSourceNativeLifecycleErrorV18 as N;
    use ProductionSourceOwnedViewErrorV18 as S;
    use fe2o3_kernel_analysis::CanonicalRankedViewErrorV1 as R;
    use fe2o3_pliron::{
        CanonicalRankedPolicyFailureV1 as P, KirBridgeErrorV12 as B12, KirBridgeErrorV18 as B18,
    };
    vec![
        E::Ranked(R::Resource(resource)),
        E::InitialCompletion(M::Source(S::Resource(resource))),
        E::InitialCompletion(M::Ranked(R::Resource(resource))),
        E::InitialCompletion(M::Native(N::Source(S::Resource(resource)))),
        E::InitialCompletion(M::Native(N::Pending(P::Resource(resource)))),
        E::Native(N::Source(S::Resource(resource))),
        E::Native(N::Pending(P::Resource(resource))),
        E::Native(N::Pending(P::View(R::Resource(resource)))),
        E::Native(N::Pending(P::Bridge(B12::Resource(resource)))),
        E::Native(N::Pending(P::StorageBridge(B18::Resource(resource)))),
    ]
}

#[test]
fn aggregate_runtime_typed_nested_resource_errors_survive_pending_scope_cleanup() {
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
        for error in aggregate_runtime_nested_errors_v30(resource) {
            assert_eq!(
                aggregate_source_resource_v30(&error),
                Some(resource),
                "{error:?}"
            );
        }
    }
    for selected in aggregate_runtime_nested_errors_v30(ArgumentResourceV1::Accounting) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) =
            integer_handoff_prepared_v18(private_entry_root_owner_v20, &mut budget);
        let roots = fixture.roots();
        let callback_floor = std::cell::Cell::new(None);
        let protected_floor = std::cell::Cell::new(0);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let chain = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let floor = budget.storage();
            let result = scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
                let (inventory, receipt) =
                    fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(
                        chain.output(budget)?.owner(),
                        budget,
                    )
                    .map_err(ProductionAggregateSourceErrorV30::Inventory)?;
                budget.reserve_storage(receipt.retained_storage())?;
                let layouts = source.limits(budget)?.storage_layout_limits();
                protected_floor.set(budget.storage());
                with_source_pending_native_v30(
                    &inventory,
                    layouts,
                    budget,
                    |message| ProductionSourceOwnedViewErrorV18::Binding(message).into(),
                    |error| source.deny_aggregate_accounting_v30(error),
                    |_, budget| {
                        callback_floor.set(Some(budget.storage()));
                        Err(selected)
                    },
                )
            });
            assert!(source.cleanup.is_denied());
            let error = result.unwrap_err();
            assert_eq!(
                aggregate_source_resource_v30(&error),
                Some(ArgumentResourceV1::Accounting)
            );
            // Nested ranked scopes may settle only their own intact scratch;
            // the outer source attempt must retain its abandoned owner credit.
            assert!(callback_floor.get().is_some());
            assert!(protected_floor.get() > floor);
            assert!(budget.storage() >= protected_floor.get());
            drop(chain);
            Err::<(), _>(error)
        });
        assert_eq!(
            aggregate_source_resource_v30(&result.unwrap_err()),
            Some(ArgumentResourceV1::Accounting)
        );
        assert!(budget.storage() >= protected_floor.get());
    }
}

#[test]
fn aggregate_runtime_header_oracles_include_owned_arguments_and_result_envelopes() {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct Owner<'a> {
        chain: &'a ProductionAggregateSourceOutputHandoffV30<'a, 'a>,
        premises: Vec<ProductionMixedSliceRuntimePremiseV26>,
        occurrences: Vec<ProductionMixedRuntimeOccurrenceV26>,
        histories: Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
        launches: &'a [fe2o3_kernel_ir::ExplicitLaunchExtent],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        retained: usize,
        required: usize,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    }
    assert_eq!(
        size_of::<Owner<'_>>(),
        size_of::<ProductionConditionalAggregateOutputHandoffV30<'_, '_, '_>>()
    );
    assert_eq!(
        align_of::<Owner<'_>>(),
        align_of::<ProductionConditionalAggregateOutputHandoffV30<'_, '_, '_>>()
    );
    let owner = size_of::<Owner<'_>>()
        + align_of::<Owner<'_>>()
        + 4 * size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
        + 4 * size_of::<&()>()
        + 5 * size_of::<usize>();
    assert_eq!(aggregate_completion_owner_headers_v30().unwrap(), owner);

    #[repr(align(128))]
    #[allow(dead_code)]
    struct Callback([u8; 131]);
    #[repr(align(64))]
    #[allow(dead_code)]
    struct Observer([u8; 67]);
    type E = ProductionAggregateSourceErrorV30;
    type Frame<'a> = (
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        fe2o3_kernel_ir::StorageLayoutLimitsV1,
        fe2o3_kernel_analysis::CanonicalRankedMetadataV18<'a, 'a>,
        fe2o3_kernel_analysis::CanonicalRankedCandidateV18<'a, 'a, 'a>,
        fe2o3_kernel_analysis::CanonicalRankedCandidateStorageV1,
        Option<Result<(), E>>,
        Result<(), E>,
        Callback,
        Observer,
        fn(&'static str) -> E,
        [usize; 4],
        [&'a (); 12],
    );
    type CandidateResult<'a> = Result<
        (
            fe2o3_kernel_analysis::CanonicalRankedCandidateV18<'a, 'a, 'a>,
            fe2o3_kernel_analysis::CanonicalRankedCandidateStorageV1,
        ),
        fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
    >;
    let pending = size_of::<Frame<'_>>()
        + align_of::<Frame<'_>>()
        + size_of::<Option<Result<(), E>>>()
        + size_of::<Result<(), E>>()
        + size_of::<Observer>()
        + align_of::<Observer>()
        + size_of::<CandidateResult<'_>>()
        + size_of::<
            Result<
                Result<(), fe2o3_pliron::CanonicalRankedPolicyFailureV1>,
                fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
            >,
        >()
        + size_of::<Result<usize, fe2o3_kernel_analysis::CanonicalRankedViewErrorV1>>();
    assert_eq!(
        source_pending_native_headers_v30::<E, Callback, Observer>().unwrap(),
        pending
    );
    let definitions = aggregate_definition_header_oracle_v30();
    let coordinates = aggregate_coordinate_header_oracle_v30();
    let roles = aggregate_role_header_oracle_v30();
    assert_eq!(aggregate_definition_headers_v30().unwrap(), definitions);
    assert_eq!(aggregate_coordinate_headers_v30().unwrap(), coordinates);
    assert_eq!(aggregate_role_headers_v30().unwrap(), roles);
    let globals =
        slice_view_v1::aggregate_global_local_header_oracle_v30() + definitions + coordinates;
    assert_eq!(
        slice_view_v1::aggregate_global_headers_v30().unwrap(),
        globals
    );
    assert_eq!(
        aggregate_completion_headers_v30().unwrap(),
        aggregate_completion_header_oracle_v30::<Owner<'_>>(owner, roles, globals)
    );
}

fn aggregate_definition_header_oracle_v30() -> usize {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct Fields {
        owner: usize,
        next: usize,
        rows: usize,
        columns: usize,
        relation: Vec<u8>,
    }
    assert_eq!(
        size_of::<Fields>(),
        size_of::<AggregateDefinitionTransportV30>()
    );
    type Frame<'a> = (
        Fields,
        Vec<u8>,
        Result<Fields, ProductionAggregateSourceErrorV30>,
        SourceOwnedResultV18<Vec<u8>>,
        SourceOwnedResultV18<usize>,
        SourceOwnedResultV18<bool>,
        [&'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>; 2],
        &'a [fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1],
        &'a AggregateSourceStageV30<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        std::ops::Range<usize>,
        [fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1; 2],
        [usize; 16],
        [Option<usize>; 4],
    );
    size_of::<Frame<'_>>() + align_of::<Frame<'_>>()
}

fn aggregate_coordinate_header_oracle_v30() -> usize {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct Fields {
        definitions: AggregateDefinitionTransportV30,
        operations: Vec<AggregateOperationV30>,
        edges: Vec<AggregateEdgeV30>,
        functions: Vec<AggregateFunctionV30>,
    }
    assert_eq!(
        size_of::<Fields>(),
        size_of::<AggregateCoordinateTransportV30>()
    );
    type Frame<'a> = (
        Fields,
        Result<Fields, ProductionAggregateSourceErrorV30>,
        AggregateDefinitionTransportV30,
        Result<AggregateDefinitionTransportV30, ProductionAggregateSourceErrorV30>,
        [&'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>; 2],
        &'a AggregateSourceStageV30<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        Vec<AggregateUniqueCoordinateV30<AggregateOperationV30>>,
        Vec<AggregateUniqueCoordinateV30<AggregateEdgeV30>>,
        Vec<AggregateUniqueCoordinateV30<AggregateFunctionV30>>,
        [AggregateOperationV30; 3],
        [AggregateEdgeV30; 3],
        [Option<usize>; 3],
        [usize; 16],
        Result<usize, ProductionSourceOwnedViewErrorV18>,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_ir::CanonicalKirOperationTransitionV1,
        &'a fe2o3_kernel_ir::CanonicalKirEdgeTransitionV1,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
        Result<
            Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
    );
    size_of::<Frame<'_>>() + align_of::<Frame<'_>>()
}

fn aggregate_role_header_oracle_v30() -> usize {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct Fields {
        owner: usize,
        next: usize,
        rows: Vec<Option<AggregateSourceRequirementV30>>,
        initial: usize,
        retired: usize,
    }
    assert_eq!(size_of::<Fields>(), size_of::<AggregateSourceRolesV30>());
    type Frame<'a> = (
        Fields,
        Vec<Option<AggregateSourceRequirementV30>>,
        Vec<AggregateUniqueCoordinateV30<AggregateOperationV30>>,
        Result<Fields, ProductionAggregateSourceErrorV30>,
        SourceOwnedResultV18<Fields>,
        Result<(), ProductionSourceNativeLifecycleErrorV18>,
        Result<(), ProductionAggregateSourceErrorV30>,
        &'a AggregateSourceStageV30<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalMixedMemoryPoliciesV26<'a, 'a>,
        &'a mut [bool],
        &'a mut Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
        &'a mut ArgumentBudgetV1<'a>,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
        Result<
            Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
        [usize; 12],
        [Option<AggregateSourceRequirementV30>; 3],
        Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>,
        Result<
            Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        Result<
            Option<&'a fe2o3_pliron::CanonicalMixedPipelineReportV26>,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        Result<
            &'a [fe2o3_pliron::CanonicalRankedSourceObligationV18],
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        Result<usize, fe2o3_pliron::CanonicalRankedPolicyFailureV1>,
        Result<
            &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
    );
    size_of::<Frame<'_>>()
        + align_of::<Frame<'_>>()
        + size_of::<
            Result<
                &[fe2o3_pliron::CanonicalRankedSourceObligationV18],
                ProductionSourceNativeLifecycleErrorV18,
            >,
        >()
}

fn aggregate_completion_header_oracle_v30<Owner>(
    owner: usize,
    roles: usize,
    globals: usize,
) -> usize {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct State {
        globals: slice_view_v1::AggregateGlobalTransportV30,
        roles: AggregateSourceRolesV30,
    }
    assert_eq!(size_of::<State>(), size_of::<AggregateRuntimeStateV30>());
    type Rows = (
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<ProductionMixedRuntimeOccurrenceV26>,
        Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
    );
    type Frame<'a> = (
        State,
        Option<State>,
        Result<State, ProductionAggregateSourceErrorV30>,
        &'a ProductionAggregateSourceOutputHandoffV30<'a, 'a>,
        [&'a (); 32],
        [usize; 24],
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'a, 'a>,
        Vec<bool>,
        Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>,
        Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
        Option<Result<(), ProductionAggregateSourceErrorV30>>,
        Option<Result<(), fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1>>,
        Result<Option<()>, fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryStorageV1,
    );
    size_of::<Frame<'_>>()
        + align_of::<Frame<'_>>()
        + aggregate_source_headers_v30().unwrap()
        + mixed_source_completion_headers_v26().unwrap()
        + owner
        + roles
        + size_of::<Result<&(), ProductionSourceNativeLifecycleErrorV18>>()
        + globals
        + size_of::<Rows>()
        + size_of::<Result<Rows, ProductionAggregateSourceErrorV30>>()
        + size_of::<
            Result<
                (
                    Vec<ProductionMixedSliceRuntimePremiseV26>,
                    Vec<ProductionMixedRuntimeOccurrenceV26>,
                    Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
                    usize,
                ),
                ProductionAggregateSourceErrorV30,
            >,
        >()
        + size_of::<Result<Owner, ProductionAggregateSourceErrorV30>>()
        + size_of::<
            Result<
                (
                    fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
                    fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
                ),
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >()
        + size_of::<
            Result<
                (
                    fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
                    fe2o3_kernel_analysis::CanonicalKirPrivateMemoryStorageV1,
                ),
                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1,
            >,
        >()
}
