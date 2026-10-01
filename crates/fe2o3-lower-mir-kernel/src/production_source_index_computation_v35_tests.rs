use super::*;
#[path = "production_source_reference_endpoints_v38_tests.rs"]
mod reference_endpoint_tests;

fn index_production_owner_v35() -> ProductionSemanticSsaOwnerV1 {
    let component = index_reader_owner();
    let semantic = component.source_semantic();
    let mut functions = semantic.functions().to_vec();
    for root in semantic.roots() {
        let function = &mut functions[root.index() as usize];
        let entry = function.kernel_entry().unwrap();
        assert!(entry.source_contract().launch().is_none());
        // The structural component fixture has no launch contract. This
        // production fixture declares one before semantic admission and hashing.
        let entry = SemanticKernelEntryV1::new(
            entry.export_symbol().clone(),
            entry.kernel_binding_identity(),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        None,
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        );
        *function = function.clone().with_kernel_entry(entry);
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
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    // The production entrance captures occurrences in its own paid ledger.
    // A fixture-local capture would require an existing live reservation there.
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn with_index_relation_v35(
    consume: impl FnOnce(
        &ProductionSourceCorrespondenceV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    index_relation_probe_v35(1_000_000_000, 256 << 20, consume).0
}

fn index_relation_probe_v35(
    work_limit: usize,
    storage_limit: usize,
    consume: impl FnOnce(
        &ProductionSourceCorrespondenceV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize, usize) {
    let owner = index_production_owner_v35();
    assert!(owner.occurrence_storage().is_none());
    let semantic = owner.source_semantic();
    let inputs: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            crate::ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(
                    1,
                    Some(
                        entry
                            .source_contract()
                            .launch()
                            .unwrap()
                            .required()
                            .unwrap()
                            .as_array(),
                    ),
                    [1, 1, 1],
                ),
            )
        })
        .collect();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(semantic, &inputs).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes =
        vec![crate::ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let input = crate::ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
        owner,
        launch,
        input,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .and_then(|prepared| {
        prepared.with_checked_source_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, consume)
                })
            })
        })
    });
    if !matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ) {
        assert_eq!(budget.storage(), 0, "{result:?}");
    }
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.storage(),
    )
}

#[test]
fn source_index_computation_retains_both_original_readers_and_explicit_global_x() {
    with_index_relation_v35(|relation, budget| {
        let retained = relation
            .source
            .root_row(0)?
            .rvalue_results
            .as_ref()
            .unwrap();
        assert_eq!(retained.index_readers.len(), 2);
        assert_eq!(
            retained
                .index_readers
                .iter()
                .map(|row| row.block)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(
            retained
                .index_readers
                .iter()
                .map(|row| row.disjoint)
                .collect::<Vec<_>>(),
            [false, true]
        );
        for block in [1, 3] {
            let reader = relation
                .index_reader_computation_v35(0, 0, SemanticBlockIdV1::from_index(block), budget)?
                .unwrap();
            let expression = reader.expression(budget)?;
            assert_eq!(
                expression,
                ProductionSemanticExpressionV2::GlobalInvocation1d {
                    scalar: ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 64
                    },
                }
            );
            let definition = reader.original_definition(budget)?;
            let actual = &relation.inventory.definitions()[definition];
            assert_eq!(actual.ty, &Type::INDEX);
            assert!(relation.inventory.operations().iter().any(|operation| {
                operation
                    .operation
                    .results
                    .iter()
                    .any(|result| Some(result.id) == actual.value)
                    && matches!(&operation.operation.kind, OperationKind::Intrinsic(intrinsic)
                        if *intrinsic == IntrinsicOperation::global_id_1d())
            }));
        }
        for block in [0, 2, 4] {
            assert!(
                relation
                    .index_reader_computation_v35(
                        0,
                        0,
                        SemanticBlockIdV1::from_index(block),
                        budget
                    )?
                    .is_none()
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_index_computation_quotes_original_call_returns_without_fresh_symbols() {
    with_index_relation_v35(|relation, budget| {
        relation.with_source_scalar_leaves_v18(0, budget, |leaves, budget| {
            source_scalar_normalization_scratch_v18(
                relation.source.cleanup,
                budget,
                source_boundary_control_headers_v31()?,
                |budget| {
                    let index = OriginalEntryIndexV20::build(relation, budget)?;
                    for row in &relation
                        .source
                        .root_row(0)?
                        .rvalue_results
                        .as_ref()
                        .unwrap()
                        .index_readers
                    {
                        let definition = index.definition(row.function, row.result, budget)?;
                        let OriginalEntryDefinitionV20::CallReturn { block, edge } =
                            definition.origin
                        else {
                            panic!("reader lost original call return");
                        };
                        let mut remaining =
                            fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                        let expression = index.private_call_expression_v33(
                            leaves,
                            row.instance,
                            row.function,
                            definition,
                            row.result,
                            block,
                            edge,
                            INDEX,
                            ProductionSemanticScalarTypeV2::Integer {
                                signed: false,
                                bits: 64,
                            },
                            0,
                            &mut remaining,
                            budget,
                        )?;
                        assert_eq!(
                            expression,
                            ProductionSemanticExpressionV2::GlobalInvocation1d {
                                scalar: ProductionSemanticScalarTypeV2::Integer {
                                    signed: false,
                                    bits: 64
                                },
                            }
                        );
                        let mut symbols = BTreeSet::new();
                        expression.symbols(&mut symbols);
                        assert!(symbols.is_empty());
                    }
                    drop(index);
                    Ok(())
                },
            )
        })
    })
    .unwrap();
}

#[test]
fn source_index_computation_replay_rejects_callee_loan_origin_and_index_substitutions() {
    with_index_relation_v35(|relation, budget| {
        let original = relation
            .source
            .root_row(0)?
            .rvalue_results
            .as_ref()
            .unwrap();
        for fault in 0..10 {
            let mut changed = OwnedSourceRvaluesV30 {
                source: original.source,
                ledger: original.ledger,
                rows: original.rows.clone(),
                values: original.values.clone(),
                index_readers: original.index_readers.clone(),
                carriers: original.carriers.clone(),
                storage: original.storage,
            };
            let row = &mut changed.index_readers[0];
            match fault {
                0 => {}
                1 => {
                    row.callee =
                        fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1::from_index(4)
                }
                2 => row.loan = usize::MAX,
                3 => row.origin_local = SemanticLocalIdV1::from_index(u32::MAX),
                4 => row.index_space = SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 1 },
                5 => row.emitted_index = ValueId(u32::MAX),
                6 => row.instance = 1,
                7 => row.loan_site.block = SemanticBlockIdV1::from_index(0),
                8 => row.witness_type = DISJOINT,
                _ => {
                    changed.index_readers.pop();
                }
            }
            assert_eq!(
                original
                    .matches_replay_v30(&changed, budget)
                    .map_err(source_emission_error_v18)?,
                fault == 0
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_index_computation_exact_context_refuses_foreign_instance() {
    let result = with_index_relation_v35(|relation, budget| {
        assert!(
            relation
                .index_reader_computation_v35(0, 0, SemanticBlockIdV1::from_index(1), budget)?
                .is_some()
        );
        relation
            .index_reader_computation_v35(0, 1, SemanticBlockIdV1::from_index(1), budget)
            .map(|_| ())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(_))
    ));
}

#[test]
fn source_index_computation_foreign_funded_ledger_is_sticky_before_charge() {
    let at_refusal = std::cell::Cell::new(None);
    let (result, _, _, remaining) =
        index_relation_probe_v35(1_000_000_000, 256 << 20, |relation, budget| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
            let mut foreign = ArgumentBudgetV1::new(&mut work, 256 << 20);
            foreign.reserve_storage(budget.storage())?;
            let before = (foreign.work(), foreign.storage());
            let result = relation.index_reader_computation_v35(
                0,
                0,
                SemanticBlockIdV1::from_index(1),
                &mut foreign,
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((foreign.work(), foreign.storage()), before);
            at_refusal.set(Some(budget.storage()));
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert_eq!(Some(remaining), at_refusal.get());
}

#[test]
fn source_index_computation_capture_and_query_exact_and_one_short_resources() {
    let run = |work, storage| {
        index_relation_probe_v35(work, storage, |relation, budget| {
            for block in [1, 3] {
                let receipt = relation
                    .index_reader_computation_v35(
                        0,
                        0,
                        SemanticBlockIdV1::from_index(block),
                        budget,
                    )?
                    .unwrap();
                receipt.original_definition(budget)?;
                receipt.expression(budget)?;
            }
            Ok(())
        })
    };
    let (result, work, storage, _) = run(1_000_000_000, 256 << 20);
    result.unwrap();
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(run(work - 1, storage).0.is_err());
    assert!(run(work, storage - 1).0.is_err());
}

#[test]
fn source_index_computation_target_law_rejects_other_axes_hierarchies_and_widths() {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 64,
    };
    for kind in [
        IndexKind::Global,
        IndexKind::Local,
        IndexKind::Workgroup,
        IndexKind::WorkgroupSize,
        IndexKind::WorkgroupCount,
    ] {
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let intrinsic =
                IntrinsicOperation::new(IntrinsicKind::InvocationIndex { kind, axis }, Type::INDEX);
            assert_eq!(
                source_global_invocation_intrinsic_v35(&intrinsic, scalar),
                (kind == IndexKind::Global && axis == Axis::X)
                    .then_some(NormalizedScalarExpressionV1::GlobalInvocation1d { scalar })
            );
        }
    }
    assert_eq!(
        source_global_invocation_intrinsic_v35(&IntrinsicOperation::launch_extent_1d(), scalar),
        None
    );
    for changed in [
        ProductionSemanticScalarTypeV2::Bool,
        ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 64,
        },
        ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
    ] {
        assert_eq!(
            source_global_invocation_intrinsic_v35(&IntrinsicOperation::global_id_1d(), changed),
            None
        );
    }
    let changed = IntrinsicOperation::new(
        IntrinsicOperation::global_id_1d().kind,
        Type::Scalar(ScalarType::U64),
    );
    assert_eq!(
        source_global_invocation_intrinsic_v35(&changed, scalar),
        None
    );
}

#[test]
fn source_index_computation_normalizes_against_its_actual_original_index() {
    with_index_relation_v35(|relation, budget| {
        relation.with_source_scalar_leaves_v18(0, budget, |leaves, budget| {
            source_scalar_normalization_scratch_v18(
                relation.source.cleanup,
                budget,
                source_boundary_control_headers_v31()?,
                |budget| {
                    let arguments = SourceRootArgumentsV18::build(relation, 0, budget)?;
                    let inline =
                        Gfx942InlineScalarCorrespondenceV30::build_source_v18(relation, 0, budget)?;
                    let function = relation.inventory.functions()
                        [relation.source.root_row(0)?.function_ordinal]
                        .coordinate;
                    let result = value_origin_v1::with_whole_value_origins_v18(
                        relation,
                        function,
                        budget,
                        |origins, budget| {
                            for block in [1, 3] {
                                let reader = relation
                                    .index_reader_computation_v35(
                                        0,
                                        0,
                                        SemanticBlockIdV1::from_index(block),
                                        budget,
                                    )?
                                    .unwrap();
                                let definition = reader.original_definition(budget)?;
                                let value =
                                    relation.inventory.definitions()[definition].value.unwrap();
                                source_scalar_expression_value_v18(
                                    leaves.leaves,
                                    &arguments,
                                    origins,
                                    &inline,
                                    &reader.expression(budget)?,
                                    value,
                                    budget,
                                )?;
                            }
                            Ok(())
                        },
                    );
                    drop(inline);
                    drop(arguments);
                    result
                },
            )
        })
    })
    .unwrap();
}
