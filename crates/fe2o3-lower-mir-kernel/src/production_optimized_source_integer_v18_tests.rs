mod formal_report_tests_v19 {
    use super::*;
    include!("production_optimized_source_formal_reports_v19_tests.rs");
}

mod formal_path_tests_v20 {
    use super::*;
    include!("production_optimized_source_formal_paths_v20_tests.rs");
}

fn integer_identity_source_v18(
    operation: SemanticBinaryOpV1,
    neutral: u128,
) -> ProductionSemanticSsaOwnerV1 {
    let base = scalar_payload_owner_v18();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut statements = helper.blocks()[0].statements().to_vec();
    // The left operand is the actual preceding memory read, not a constant
    // substituted into an inert canonical fixture. Its result feeds a live store.
    statements.insert(
        3,
        assign(
            place(1, U32),
            SemanticRvalueKindV1::Binary {
                operation,
                left: SemanticOperandV1::Copy(place(1, U32)),
                right: literal(neutral),
            },
        ),
    );
    statements[4] = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(2, U32),
            SemanticOperandV1::Copy(place(1, U32)),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    functions[2] = function(
        210,
        helper.role(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        vec![block(214, statements, SemanticTerminatorKindV1::Return)],
    );
    // This fixture supplies a complete scalar descriptor at the owning handoff.
    // Author its root ownership before admission; unspecified ownership must
    // continue to fail the production ABI contract.
    for &root in semantic.roots() {
        let declaration = &functions[root.index() as usize];
        assert_eq!(declaration.role(), SemanticFunctionRoleV1::KernelRoot);
        assert_eq!(declaration.abi().source_input_types(), &[U32]);
        assert_eq!(
            declaration.abi().source_argument_ownership(),
            &[SemanticSourceArgumentOwnershipV1::Unspecified]
        );
        functions[root.index() as usize] = SemanticFunctionDeclV1::new(
            declaration.identity(),
            declaration.role(),
            declaration.item_definition_identity(),
            declaration.monomorphization_identity(),
            declaration.generic_type_arguments_identity(),
            declaration.const_generic_arguments_identity(),
            declaration.source(),
            declaration
                .abi()
                .clone()
                .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
                .unwrap(),
            declaration.locals().to_vec(),
            declaration.entry(),
            declaration.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(declaration.kernel_entry().unwrap().clone());
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
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

fn integer_add_source_v18() -> ProductionSemanticSsaOwnerV1 {
    integer_identity_source_v18(SemanticBinaryOpV1::Add, 0)
}
fn integer_multiply_source_v18() -> ProductionSemanticSsaOwnerV1 {
    integer_identity_source_v18(SemanticBinaryOpV1::Multiply, 1)
}
fn integer_and_source_v18() -> ProductionSemanticSsaOwnerV1 {
    integer_identity_source_v18(SemanticBinaryOpV1::BitAnd, u32::MAX.into())
}
fn integer_non_neutral_source_v18() -> ProductionSemanticSsaOwnerV1 {
    integer_identity_source_v18(SemanticBinaryOpV1::Add, 1)
}
fn integer_divide_source_v18() -> ProductionSemanticSsaOwnerV1 {
    integer_identity_source_v18(SemanticBinaryOpV1::Divide, 2)
}

fn integer_noop_consumer_v18(
    _: &ProductionSourceCorrespondenceV18<'_>,
    _: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    _: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<((), usize)> {
    Ok(((), 0))
}

fn integer_entry_header_oracle_v18<F>(_: &F) -> usize {
    type Output = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerContinuationV18,
        (),
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    type Error = ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>;
    type Observed = fe2o3_pliron::KirNeutralOptimizationOutputIntegerContinuationV18<'static>;
    type Entry<F> = (Observed, usize, usize, F);
    type EntryResult<F> = Result<Entry<F>, SourceConsumerErrorV18<Error>>;
    type Capture<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        usize,
        F,
    );
    type Adoption<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        Observed,
        F,
    );
    type Adopted = Result<
        Output,
        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1<ProductionSourceOwnedViewErrorV18>,
    >;
    type Settled = Result<Output, SourceConsumerErrorV18<Error>>;
    2 * size_of::<Capture<'_, '_, F>>()
        + 2 * std::mem::align_of::<Capture<'_, '_, F>>()
        + size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<Entry<F>>()
        + 2 * size_of::<EntryResult<F>>()
        + size_of::<std::thread::Result<EntryResult<F>>>()
        + size_of::<std::panic::AssertUnwindSafe<Entry<F>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<std::cell::Cell<usize>>()
        + 3 * size_of::<usize>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<SourceOwnedResultV18<()>>()
        + 4 * size_of::<Adoption<'_, '_, F>>()
        + 4 * std::mem::align_of::<Adoption<'_, '_, F>>()
        + size_of::<std::panic::AssertUnwindSafe<Adoption<'_, '_, F>>>()
        + size_of::<Adopted>()
        + size_of::<Settled>()
        + size_of::<std::thread::Result<Settled>>()
        + size_of::<Result<Output, Error>>()
        + size_of::<std::panic::AssertUnwindSafe<Result<Output, Error>>>()
        + size_of::<Error>()
        + source_reference_cleanup_headers_v29().unwrap()
}

#[test]
fn source_integer_continuation_rewrites_dynamic_values_and_preserves_actual_memory() {
    for factory in [
        integer_add_source_v18 as fn() -> _,
        integer_multiply_source_v18,
        integer_and_source_v18,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let (output, (), receipt) = source
                    .with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                        let input = optimized.input_inventory(budget)?;
                        let output = optimized.output_inventory(budget)?;
                        assert!(std::ptr::eq(input, original.inventory(budget)?));
                        assert!(std::ptr::eq(
                            optimized.original_source(budget)?,
                            original.source
                        ));
                        let before = input
                            .operations()
                            .iter()
                            .filter(|row| {
                                matches!(row.operation.kind, OperationKind::Binary { .. })
                            })
                            .count();
                        let after = output
                            .operations()
                            .iter()
                            .filter(|row| {
                                matches!(row.operation.kind, OperationKind::Binary { .. })
                            })
                            .count();
                        assert!(before > 0);
                        assert_eq!(after, 0);
                        let mut loads = 0;
                        let mut stores = 0;
                        for row in input.operations() {
                            if matches!(
                                row.operation.kind,
                                OperationKind::Load { .. } | OperationKind::Store { .. }
                            ) {
                                assert!(matches!(
                                    optimized.operation(row.coordinate, budget)?,
                                    ProductionOptimizedSourceOperationV18::Retained { .. }
                                ));
                                loads += usize::from(matches!(
                                    row.operation.kind,
                                    OperationKind::Load { .. }
                                ));
                                stores += usize::from(matches!(
                                    row.operation.kind,
                                    OperationKind::Store { .. }
                                ));
                            }
                        }
                        assert!(loads > 0 && stores > 0);
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                    })
                    .unwrap();
                assert_eq!(
                    output
                        .report()
                        .passes()
                        .iter()
                        .map(|row| row.pass())
                        .collect::<Vec<_>>(),
                    vec![
                        fe2o3_pliron::PlironOptimizationPassV1::IntegerNeutralCanonicalization,
                        fe2o3_pliron::PlironOptimizationPassV1::DeadCodeElimination,
                    ]
                );
                assert!(output.report().passes()[0].changed());
                assert_eq!(
                    &output.execution().canonical_bytes()[..8],
                    &[6, 0, 1, 0, 2, 0, 18, 0]
                );
                assert_eq!(output.execution().graph_schema(), 18);
                assert_eq!(output.execution().policy_version(), 6);
                assert!(!output.grants_authority() && !output.execution().grants_authority());
                assert!(!receipt.grants_authority());
                assert_eq!(output.map().output_identity(), output.owner().identity());
                drop(output);
                assert_eq!(budget.storage(), floor);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_integer_continuation_keeps_non_neutral_and_unsupported_rules_unchanged() {
    for factory in [
        integer_non_neutral_source_v18 as fn() -> _,
        integer_divide_source_v18,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let (output, (), _) = source
                    .with_checked_integer_optimization_v18(budget, |_, optimized, budget| {
                        let input = optimized.input_inventory(budget)?;
                        let output = optimized.output_inventory(budget)?;
                        let before = input
                            .operations()
                            .iter()
                            .filter(|row| {
                                matches!(row.operation.kind, OperationKind::Binary { .. })
                            })
                            .count();
                        let after = output
                            .operations()
                            .iter()
                            .filter(|row| {
                                matches!(row.operation.kind, OperationKind::Binary { .. })
                            })
                            .count();
                        assert!(before > 0);
                        assert_eq!(before, after);
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                    })
                    .unwrap();
                assert!(!output.report().passes()[0].changed());
                drop(output);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_integer_continuation_does_not_replace_or_relabel_policy3() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let original_identity = *source.canonical(budget)?.identity();
            let (scalar, (), _) = source
                .with_checked_optimization_v18(budget, integer_noop_consumer_v18)
                .unwrap();
            assert_eq!(scalar.report().passes().len(), 8);
            assert_eq!(
                &scalar.execution().canonical_bytes()[..8],
                &[3, 0, 1, 0, 8, 0, 18, 0]
            );
            let scalar_identity = *scalar.owner().identity();
            drop(scalar);
            let (integer, (), _) = source
                .with_checked_integer_optimization_v18(budget, integer_noop_consumer_v18)
                .unwrap();
            assert_eq!(integer.report().passes().len(), 2);
            assert_eq!(integer.map().input_identity(), &original_identity);
            assert_ne!(integer.owner().identity(), &scalar_identity);
            drop(integer);
            assert_eq!(source.canonical(budget)?.identity(), &original_identity);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_integer_continuation_preserves_balanced_callback_error_and_panic() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let outcome = source.with_checked_integer_optimization_v18(budget, |_, _, _| {
                reached.set(true);
                match mode {
                    0 => Ok(((), 0)),
                    1 => Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "integer callback selected error",
                    )),
                    _ => std::panic::resume_unwind(Box::new(0x1779_u32)),
                }
            });
            match (mode, outcome) {
                (0, Ok((owner, (), _))) => drop(owner),
                (
                    1,
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "integer callback selected error",
                            ),
                        ),
                    )),
                ) => {}
                (
                    2,
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Panicked,
                    )),
                ) => {}
                _ => panic!("integer continuation changed callback disposition"),
            }
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(reached.get());
        if mode == 2 {
            assert!(result.is_err());
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
    }
}

#[test]
fn source_integer_continuation_header_refusal_is_sticky_after_padding_release() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let consume = integer_noop_consumer_v18;
        let (owner, (), _) = source
            .with_checked_integer_optimization_v18(budget, consume)
            .unwrap();
        drop(owner);
        let headers = integer_entry_header_oracle_v18(&consume);
        assert_eq!(
            headers,
            IntegerSourceOptimizerV18::headers::<(), ProductionSourceOwnedViewErrorV18, _>(
                &consume
            )
            .unwrap()
        );
        let padding = MODULE_LIMIT - budget.storage() - headers + 1;
        budget.reserve_storage(padding).unwrap();
        let first = match source.with_checked_integer_optimization_v18(budget, consume) {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            )) => error,
            _ => panic!("integer entry must refuse its one-short header"),
        };
        assert!(matches!(first, ArgumentResourceV1::Storage(_)));
        budget.release_storage(padding).unwrap();
        let before = (budget.work(), budget.storage());
        match source.with_checked_integer_optimization_v18(budget, consume) {
            Err(ProductionSourceOptimizationErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            )) => assert_eq!(error, first),
            _ => panic!("integer entry forgot its first refusal"),
        }
        assert_eq!((budget.work(), budget.storage()), before);
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_integer_observation_refuses_foreign_adoption_without_refunding_original_custody() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let input = source.canonical(budget)?;
            let layouts = source.limits(budget)?.storage_layout_limits();
            let floor = budget.storage();
            let observed = fe2o3_pliron::optimize_neutral_kernel_ir_integer_continuation_v18(
                input, layouts, budget,
            )
            .unwrap();
            let retained = observed.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            let mut foreign_work =
                CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
            foreign.reserve_storage(floor + retained).unwrap();
            let foreign_before = (foreign.work(), foreign.storage());
            let result = observed.try_check_and_finish_v18(&mut foreign);
            assert!(matches!(
                result,
                Err(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!((foreign.work(), foreign.storage()), foreign_before);
            assert_eq!(budget.storage(), floor + retained);
            budget.release_storage(retained).unwrap();
            assert_eq!(source.canonical(budget)?.identity(), input.identity());
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_integer_owned_callback_error_keeps_residual_and_first_source_refusal() {
    for first_refusal in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(integer_add_source_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result: Result<(), OwnedCallbackErrorV18> =
            prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                let result =
                    source.with_checked_integer_optimization_v18(budget, |original, _, budget| {
                        if first_refusal {
                            let error = budget.charge_work(usize::MAX).unwrap_err();
                            let _ = original.retain_query_resource_error_v18(error);
                        }
                        let payload = owned_callback_payload_v18(budget);
                        reached.set(true);
                        Err::<((), usize), _>(OwnedCallbackErrorV18::Payload(payload))
                    });
                match result {
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(error),
                    )) => Err(error),
                    Err(ProductionSourceOptimizationErrorV18::Source(error)) => Err(error.into()),
                    _ => panic!("integer callback did not preserve its selected owned error"),
                }
            });
        assert!(reached.get());
        assert!(budget.storage() > MODULE_FLOOR + 64 * size_of::<u64>());
        match (first_refusal, result) {
            (false, Err(OwnedCallbackErrorV18::Payload(payload))) => {
                assert_eq!(payload, vec![0x271; 64])
            }
            (
                true,
                Err(OwnedCallbackErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Work(error),
                ))),
            ) => assert_eq!(error.actual(), usize::MAX),
            other => panic!("integer callback first refusal changed: {other:?}"),
        }
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.failed_work(), first_refusal.then_some(usize::MAX));
    }
}
fn integer_handoff_noop_source_v18() -> ProductionSemanticSsaOwnerV1 {
    use super::super::closed_scalar_handoff_tests::{ClosedCaseV1760, closed_owner_v1760};
    closed_owner_v1760(ClosedCaseV1760::Noop)
}

#[test]
fn integer_handoff_exact_noop_keeps_distinct_policy_and_requires_owned_disposal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(integer_handoff_noop_source_v18, &mut budget);
    let roots = fixture.roots();
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let handoff = source.unqualified_integer_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let output = handoff.output(budget)?;
            assert_eq!(output.input_audit_bytes(), output.owner().canonical_bytes());
            assert_eq!(
                &output.execution().canonical_bytes()[..8],
                &[6, 0, 1, 0, 2, 0, 18, 0]
            );
            assert!(output.report().passes().iter().all(|pass| !pass.changed()));
            let retained = handoff.retained_storage(budget)?;
            let paid = budget.storage();
            drop(handoff);
            assert_eq!(
                budget.storage(),
                paid,
                "ordinary Drop must not refund the ledger"
            );
            budget.release_storage(retained)?;
            assert_eq!(budget.storage(), floor);
            Ok::<_, ProductionUnqualifiedIntegerHandoffErrorV18>(())
        })
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn integer_handoff_early_header_refusal_stays_selected_after_padding_release() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = integer_handoff_prepared_v18(integer_add_source_v18, &mut budget);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let floor = budget.storage();
        let padding = budget.storage_limit() - floor;
        budget.reserve_storage(padding).unwrap();
        let error = source
            .unqualified_integer_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .err()
            .unwrap();
        let ProductionUnqualifiedIntegerHandoffErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(refusal)),
        ) = &error
        else {
            panic!("not an original header refusal: {error:?}");
        };
        assert!(refusal.actual() > refusal.limit());
        assert_eq!(refusal.limit(), MODULE_LIMIT);
        let selected = error.to_string();
        budget.release_storage(padding).unwrap();
        let retry = source
            .unqualified_integer_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .err()
            .unwrap();
        assert_eq!(retry.to_string(), selected);
        assert_eq!(budget.storage(), floor);
        Err::<(), _>(error)
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn integer_handoff_prepared_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> (ProductionPreparedSourceV18, OriginalKernelAbiFixtureV18) {
    with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        budget,
        factory,
        |owner, launch, input, _, budget| {
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
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
            (prepared, fixture)
        },
    )
}

#[test]
fn integer_handoff_owns_actual_memory_outputs_with_neutral_and_unsupported_rules() {
    for (factory, changed) in [
        (integer_add_source_v18 as fn() -> _, true),
        (integer_non_neutral_source_v18, false),
        (integer_divide_source_v18, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let handoff = source.unqualified_integer_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                let output = handoff.output(budget)?;
                assert_eq!(
                    output.input_audit_bytes(),
                    source.canonical(budget)?.canonical_bytes()
                );
                assert_eq!(output.report().passes()[0].changed(), changed);
                if changed {
                    assert_ne!(output.owner().canonical_bytes(), output.input_audit_bytes());
                }
                assert_eq!(output.report().passes().len(), 2);
                let operations = |owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18| {
                    owner
                        .module()
                        .functions
                        .iter()
                        .filter_map(|f| f.body.as_ref())
                        .flat_map(|body| &body.blocks)
                        .flat_map(|block| &block.operations)
                        .filter(|operation| {
                            matches!(
                                operation.kind,
                                OperationKind::Load { .. } | OperationKind::Store { .. }
                            )
                        })
                        .count()
                };
                let memory = operations(source.canonical(budget)?);
                assert!(memory > 0);
                assert_eq!(operations(output.owner()), memory);
                let header = size_of::<ProductionUnqualifiedIntegerOutputHandoffV18<'_, '_>>()
                    - size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerContinuationV18>()
                    + std::mem::align_of::<ProductionUnqualifiedIntegerOutputHandoffV18<'_, '_>>();
                let retained = output.storage().retained_storage() + header;
                assert_eq!(handoff.retained_storage(budget)?, retained);
                assert_eq!(budget.storage(), floor + retained);
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                Ok::<_, ProductionUnqualifiedIntegerHandoffErrorV18>(())
            })
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn integer_handoff_rejects_foreign_original_and_keeps_first_refusal_after_discard() {
    let foreign = integer_add_source_v18();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = integer_handoff_prepared_v18(integer_add_source_v18, &mut budget);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let floor = budget.storage();
        let handoff = source.unqualified_integer_output_v18(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
        assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
        let first = handoff.check_original_source(&foreign, budget).unwrap_err();
        assert!(matches!(
            first,
            ProductionSourceOwnedViewErrorV18::Binding(_)
        ));
        let selected = first.to_string();
        assert_eq!(handoff.output(budget).err().unwrap().to_string(), selected);
        assert_eq!(handoff.discard(budget).unwrap_err().to_string(), selected);
        assert_eq!(budget.storage(), floor);
        let retry = source
            .unqualified_integer_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .err()
            .unwrap();
        assert!(matches!(
            retry,
            ProductionUnqualifiedIntegerHandoffErrorV18::Source(_)
        ));
        assert_eq!(retry.to_string(), selected);
        Err::<(), _>(ProductionUnqualifiedIntegerHandoffErrorV18::from(first))
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn integer_handoff_undercut_or_foreign_custody_never_refunds_original_credit() {
    for foreign_account in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(integer_add_source_v18, &mut budget);
        let roots = fixture.roots();
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let handoff = source.unqualified_integer_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let first = if foreign_account {
                let mut foreign_work =
                    CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                foreign.reserve_storage(budget.storage()).unwrap();
                let first = handoff.output(&foreign).err().unwrap();
                assert_eq!(foreign.storage(), budget.storage());
                first
            } else {
                budget.release_storage(1).unwrap();
                handoff.output(budget).err().unwrap()
            };
            assert!(matches!(
                first,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            let paid = budget.storage();
            assert!(handoff.discard(budget).is_err());
            assert_eq!(budget.storage(), paid);
            Err::<(), _>(ProductionUnqualifiedIntegerHandoffErrorV18::from(first))
        });
        assert!(result.is_err());
        assert!(budget.storage() > MODULE_FLOOR);
    }
}

#[test]
fn integer_handoff_requires_full_original_abi_before_optimization_and_on_retry() {
    for missing in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(integer_add_source_v18, &mut budget);
        let mut roots = fixture.roots();
        if !missing {
            roots[0].kernel_binding = roots[1].kernel_binding;
        }
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let first = source
                .unqualified_integer_output_v18(
                    ProductionKernelArgumentAbiInputV18 {
                        roots: if missing { &[] } else { &roots },
                    },
                    budget,
                )
                .err()
                .unwrap();
            assert!(matches!(
                first,
                ProductionUnqualifiedIntegerHandoffErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(_)
                )
            ));
            assert_eq!(budget.storage(), floor);
            let original_roots = fixture.roots();
            let retry = source
                .unqualified_integer_output_v18(
                    ProductionKernelArgumentAbiInputV18 {
                        roots: &original_roots,
                    },
                    budget,
                )
                .err()
                .unwrap();
            assert_eq!(first.to_string(), retry.to_string());
            Err::<(), _>(first)
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
