#[derive(Clone, Copy)]
pub(super) enum ClosedCaseV1760 {
    Noop,
    Changed,
    Assertion,
    Control,
    Call,
}

pub(super) fn closed_owner_v1760(case: ClosedCaseV1760) -> ProductionSemanticSsaOwnerV1 {
    if matches!(case, ClosedCaseV1760::Assertion) {
        return original_kernel_abi_owner_v18();
    }
    let original = original_kernel_abi_owner_v18();
    let semantic = original.source_semantic();
    let mut functions: Vec<_> = semantic
        .functions()
        .iter()
        .enumerate()
        .map(|(ordinal, prior)| {
            let tag = if ordinal == 0 { 60 } else { 150 };
            let statements = if matches!(case, ClosedCaseV1760::Changed) {
                vec![assign(
                    place(1, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            U32,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(7, 4).unwrap(),
                            ),
                        ),
                    )),
                )]
            } else {
                vec![]
            };
            let blocks = match case {
                ClosedCaseV1760::Control => vec![
                    block(
                        tag + 4,
                        statements,
                        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::Goto,
                            SemanticBlockIdV1::from_index(1),
                        )),
                    ),
                    block(tag + 5, vec![], SemanticTerminatorKindV1::Return),
                ],
                ClosedCaseV1760::Call => vec![
                    block(
                        tag + 4,
                        statements,
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                SemanticCallableIdV1::from_index(2),
                                vec![SemanticOperandV1::Copy(place(1, U32))],
                                Some(SemanticCallDestinationV1::new(
                                    place(0, UNIT),
                                    SemanticControlFlowEdgeV1::new(
                                        SemanticEdgeRoleV1::CallReturn,
                                        SemanticBlockIdV1::from_index(1),
                                    ),
                                )),
                                SemanticUnwindActionV1::Unreachable,
                            )
                            .unwrap(),
                        ),
                    ),
                    block(tag + 5, vec![], SemanticTerminatorKindV1::Return),
                ],
                _ => vec![block(tag + 4, statements, SemanticTerminatorKindV1::Return)],
            };
            function(
                tag,
                prior.role(),
                prior.abi().clone(),
                prior.locals().to_vec(),
                blocks,
            )
            .with_kernel_entry(prior.kernel_entry().unwrap().clone())
        })
        .collect();
    if matches!(case, ClosedCaseV1760::Call) {
        functions.push(function(
            180,
            SemanticFunctionRoleV1::InternalHelper,
            abi(181, false, &[U32]),
            vec![
                local(182, UNIT, SemanticLocalRoleV1::Return),
                local(183, U32, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![block(184, vec![], SemanticTerminatorKindV1::Return)],
        ));
    }
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types()[..2].to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn closed_prepared_v1760(
    case: ClosedCaseV1760,
    profile: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> (ProductionPreparedSourceV18, OriginalKernelAbiFixtureV18) {
    with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        budget,
        || closed_owner_v1760(case),
        |owner, launch, input, _, budget| {
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
            let prepared = if profile {
                let roots = fixture.roots();
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            } else {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            }
            .unwrap();
            (prepared, fixture)
        },
    )
}

#[test]
fn closed_scalar_handoff_checks_actual_changed_and_noop_outputs_and_exact_credit() {
    for (case, explicit_discard) in [
        (ClosedCaseV1760::Noop, true),
        (ClosedCaseV1760::Changed, true),
        (ClosedCaseV1760::Noop, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = closed_prepared_v1760(case, true, &mut budget);
        let roots = fixture.roots();
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let handoff = source.checked_closed_scalar_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                let output = handoff.output(budget)?;
                assert_eq!(
                    output.input_audit_bytes(),
                    source.canonical(budget)?.canonical_bytes()
                );
                assert_eq!(
                    output.owner().canonical_bytes() != output.input_audit_bytes(),
                    matches!(case, ClosedCaseV1760::Changed)
                );
                let independent_header =
                    size_of::<ProductionClosedScalarOutputHandoffV18<'_, '_>>()
                        - size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerV18>()
                        + std::mem::align_of::<ProductionClosedScalarOutputHandoffV18<'_, '_>>();
                assert_eq!(
                    handoff.retained_storage(budget)?,
                    output.storage().retained_storage() + independent_header
                );
                assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
                if explicit_discard {
                    handoff.discard(budget)?;
                } else {
                    let retained = handoff.retained_storage(budget)?;
                    let paid = budget.storage();
                    drop(handoff);
                    assert_eq!(
                        budget.storage(),
                        paid,
                        "ordinary Drop cannot refund a ledger"
                    );
                    // This test owns the disposed concrete output's exact receipt.
                    budget.release_storage(retained)?;
                }
                assert_eq!(budget.storage(), floor);
                Ok::<_, ProductionClosedScalarHandoffErrorV18>(())
            })
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn closed_scalar_handoff_refuses_assertions_calls_and_control_before_optimizer() {
    for (case, expected) in [
        (ClosedCaseV1760::Assertion, "assertion"),
        (ClosedCaseV1760::Control, "control flow"),
        (ClosedCaseV1760::Call, "non-root functions"),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = closed_prepared_v1760(case, true, &mut budget);
        let roots = fixture.roots();
        prepared.with_checked_source_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let error = source.checked_closed_scalar_output_v18(ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget).err().unwrap();
            assert!(matches!(error, ProductionClosedScalarHandoffErrorV18::Check(ProductionClosedScalarCheckErrorV18::Unsupported(detail)) if detail == expected), "{error:?}");
            assert_eq!(budget.storage(), floor);
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn closed_scalar_handoff_refuses_genuine_profiled_memory_source() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        || descriptor_source_owner(DescriptorCase::READ),
        |owner, launch, input, _, budget| {
            let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
            let roots = fixture.roots();
            let prepared =
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap();
            drop(roots);
            (prepared, fixture)
        },
    );
    let roots = fixture.roots();
    prepared
        .with_checked_source_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let error = source
                .checked_closed_scalar_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )
                .err()
                .unwrap();
            assert!(
                matches!(
                    error,
                    ProductionClosedScalarHandoffErrorV18::Check(
                        ProductionClosedScalarCheckErrorV18::Unsupported(
                            "memory or non-scalar type"
                        )
                    )
                ),
                "{error:?}"
            );
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn closed_scalar_handoff_requires_complete_authentic_profile_not_matching_inert_shape() {
    for captured in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, mut fixture) =
            closed_prepared_v1760(ClosedCaseV1760::Noop, captured, &mut budget);
        if captured {
            fixture.bindings[0][0] ^= 1;
        }
        let roots = fixture.roots();
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source
                .checked_closed_scalar_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?
                .discard(budget)?;
            Ok::<_, ProductionClosedScalarHandoffErrorV18>(())
        });
        assert!(
            matches!(
                result,
                Err(ProductionClosedScalarHandoffErrorV18::Check(
                    ProductionClosedScalarCheckErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Binding(_)
                    )
                ))
            ),
            "{result:?}"
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn closed_scalar_handoff_rejects_equal_bytes_foreign_original_owner() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
    let foreign = closed_owner_v1760(ClosedCaseV1760::Noop);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let handoff = source.checked_closed_scalar_output_v18(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        assert_eq!(
            source.source_ssa(budget)?.source_semantic_sha256(),
            foreign.source_semantic_sha256()
        );
        let retained = handoff.retained_storage(budget)?;
        let error = handoff.check_original_source(&foreign, budget).unwrap_err();
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
        ));
        // The query latch must not revoke genuine cleanup custody.
        let before_discard = budget.storage();
        assert!(matches!(
            handoff.discard(budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "foreign original SSA owner"
            ))
        ));
        assert_eq!(budget.storage(), before_discard - retained);
        Err::<(), _>(ProductionClosedScalarHandoffErrorV18::from(error))
    });
    assert!(matches!(
        result,
        Err(ProductionClosedScalarHandoffErrorV18::Check(
            ProductionClosedScalarCheckErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
            )
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn closed_scalar_handoff_foreign_slot_and_undercut_floor_deny_all_containing_refunds() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
        let roots = fixture.roots();
        let foreign_source = closed_owner_v1760(ClosedCaseV1760::Noop);
        let retained = std::cell::Cell::new(0);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let handoff = source.checked_closed_scalar_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let error = if mode == 1 {
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                foreign.reserve_storage(budget.storage())?;
                let before = (foreign.work(), foreign.storage());
                let error = handoff.discard(&mut foreign).unwrap_err();
                assert_eq!((foreign.work(), foreign.storage()), before);
                error
            } else if mode == 0 {
                budget.release_storage(1)?;
                handoff.discard(budget).unwrap_err()
            } else {
                assert!(matches!(
                    handoff.check_original_source(&foreign_source, budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "foreign original SSA owner"
                    ))
                ));
                source.cleanup.deny_refund();
                handoff.discard(budget).unwrap_err()
            };
            if mode == 2 {
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
                ));
            } else {
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
            }
            retained.set(budget.storage());
            Err::<(), _>(ProductionClosedScalarHandoffErrorV18::from(error))
        });
        if mode == 2 {
            assert!(matches!(
                result,
                Err(ProductionClosedScalarHandoffErrorV18::Check(
                    ProductionClosedScalarCheckErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
                    )
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionClosedScalarHandoffErrorV18::Check(
                    ProductionClosedScalarCheckErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                    )
                ))
            ));
        }
        assert!(retained.get() > MODULE_FLOOR);
        assert_eq!(budget.storage(), retained.get());
    }
}

#[test]
fn closed_scalar_handoff_exact_and_one_short_storage_use_real_full_pipeline() {
    fn run(
        limit: usize,
    ) -> (
        Result<(), ProductionClosedScalarHandoffErrorV18>,
        usize,
        Option<usize>,
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) =
            closed_prepared_v1760(ClosedCaseV1760::Changed, true, &mut budget);
        let roots = fixture.roots();
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source
                .checked_closed_scalar_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?
                .discard(budget)?;
            Ok::<_, ProductionClosedScalarHandoffErrorV18>(())
        });
        assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
        (result, budget.peak_storage(), budget.failed_storage())
    }
    let (result, peak, failed) = run(MODULE_LIMIT);
    result.unwrap();
    assert_eq!(failed, None);
    let (result, exact_peak, failed) = run(peak);
    result.unwrap();
    assert_eq!((exact_peak, failed), (peak, None));
    let (result, refused_peak, failed) = run(peak - 1);
    use ProductionClosedScalarCheckErrorV18 as Check;
    use ProductionClosedScalarHandoffErrorV18 as Handoff;
    use ProductionSourceOptimizationErrorV18 as Optimization;
    use fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1 as Adoption;
    let selected = result.err().expect("one-short must refuse");
    assert!(
        matches!(closed_resource_cause_v1762(&selected), Some(ArgumentResourceV1::Storage(error)) if (error.actual(), error.limit()) == (peak, peak - 1))
    );
    let error = match selected {
        Handoff::Check(Check::Source(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(error),
        )))
        | Handoff::Optimization(Optimization::Adoption(Adoption::Origin(Check::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)),
        ))))
        | Handoff::Optimization(Optimization::Adoption(Adoption::Origin(Check::Ranked(
            fe2o3_kernel_analysis::CanonicalRankedViewErrorV1::Resource(
                ArgumentResourceV1::Storage(error),
            ),
        )))) => error,
        Handoff::Optimization(Optimization::Adoption(Adoption::Origin(Check::Native(
            ProductionSourceNativeLifecycleErrorV18::Native(error),
        )))) => match error.failure() {
            fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(
                ArgumentResourceV1::Storage(error),
            )
            | fe2o3_pliron::CanonicalRankedPolicyFailureV1::View(
                fe2o3_kernel_analysis::CanonicalRankedViewErrorV1::Resource(
                    ArgumentResourceV1::Storage(error),
                ),
            )
            | fe2o3_pliron::CanonicalRankedPolicyFailureV1::StorageBridge(
                fe2o3_pliron::KirBridgeErrorV18::Resource(ArgumentResourceV1::Storage(error)),
            )
            | fe2o3_pliron::CanonicalRankedPolicyFailureV1::StorageBridge(
                fe2o3_pliron::KirBridgeErrorV18::Canonical(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(
                        ArgumentResourceV1::Storage(error),
                    ),
                ),
            ) => *error,
            other => panic!("expected actual native Storage refusal: {other:?}"),
        },
        other => panic!("expected selected Storage refusal, not another failure: {other:?}"),
    };
    assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
    assert_eq!(failed, Some(peak));
    assert!(refused_peak <= peak - 1);
}

#[test]
fn closed_scalar_handoff_preflight_work_refusal_preserves_original_resource() {
    let limit = 500_000_000;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
    let roots = fixture.roots();
    let selected = std::cell::Cell::new(None);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        budget.charge_work(limit - budget.work())?;
        let error = source.checked_closed_scalar_output_v18(ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget).err().unwrap();
        if let ProductionClosedScalarHandoffErrorV18::Check(ProductionClosedScalarCheckErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &error { selected.set(Some(*resource)); }
        assert_eq!(closed_resource_cause_v1762(&error).copied(), selected.get());
        assert!(matches!(selected.get(), Some(ArgumentResourceV1::Work(error)) if error.actual() == limit + 1 && error.limit() == limit));
        assert_eq!(budget.work(), limit);
        Err::<(), _>(error)
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

include!("production_closed_scalar_error_sources_v1762_tests.rs");

include!("production_source_retained_custody_v1764_tests.rs");

include!("production_source_closed_unit_constants_v1765_tests.rs");

include!("production_source_helper_credit_v1766_tests.rs");
