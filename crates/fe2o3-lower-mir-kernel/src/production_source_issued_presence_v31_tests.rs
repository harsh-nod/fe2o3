use super::*;

const ISSUED_WORK_V31: usize = 1_000_000_000;
const ISSUED_STORAGE_V31: usize = 256 << 20;

fn with_issued_source_v31(
    looping: bool,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> ProductionOptimizerTestResultV18 {
    let owner = mixed_licm_source_v28(looping);
    let abi = issued_descriptor_role_abi_v18(&owner);
    let roots = abi.roots();
    let semantic = owner.source_semantic();
    let launches: Vec<_> = semantic
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
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launches).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(ISSUED_WORK_V31);
    let mut budget = ArgumentBudgetV1::new(&mut work, ISSUED_STORAGE_V31);
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
        let floor = budget.storage();
        let (output, (), receipt) = source.with_checked_mixed_fixedpoint_optimization_v18(
            budget,
            |original, optimized, budget| {
                consume(original, optimized, budget)?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            },
        )?;
        assert_eq!(
            receipt.retained_storage(),
            size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
        );
        assert_eq!(output.execution().policy_version(), 11);
        drop(output);
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    result
}

#[test]
fn issued_discriminant_source_and_output_share_exact_predicate_after_phi_names() {
    for looping in [false, true] {
        let reached = std::cell::Cell::new(false);
        with_issued_source_v31(looping, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(optimized, 0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22, budget, |leaves, budget| {
                let source = leaves.original.leaves;
                assert!(!source.presences.rows.is_empty());
                for presence in &source.presences.rows {
                    assert!(source.rows.iter().all(|row| row.symbol < presence.symbol));
                    assert!(source.boundaries.rows.iter().all(|row| row.symbol < presence.symbol));
                    assert!(presence.symbol < PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2);
                }
                with_check(leaves, budget, |check, budget| {
                    let function = leaves.original.original_function(0, budget)?;
                    let mut count = 0;
                    for (block, declaration) in function.blocks().iter().enumerate() {
                        for (statement, declaration) in declaration.statements().iter().enumerate() {
                            let SemanticStatementKindV1::Assign(assignment) = declaration.kind() else { continue; };
                            let SemanticRvalueKindV1::Discriminant(_) = assignment.value().kind() else { continue; };
                            let ty = assignment.value().result_type();
                            let scalar = kir_semantic_scalar_v1(&lower_scalar_type(original.source.source_semantic(budget)?.types(), ty)
                                .map_err(source_emission_error_v18)?).unwrap();
                            let mut remaining = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                            let expression = check.index.private_expression_v22(leaves.original, 0, ty, scalar,
                                OriginalPrivateInputV22::Rvalue { block: block as u32, statement: statement as u32, value: assignment.value() },
                                0, &mut remaining, budget)?;
                            assert!(matches!(&expression, ProductionSemanticExpressionV2::Cast {
                                kind: fe2o3_pliron::ProductionSemanticCastV2::Integer,
                                source: ProductionSemanticScalarTypeV2::Bool, target, operand,
                            } if *target == scalar && matches!(operand.as_ref(), ProductionSemanticExpressionV2::Symbol { scalar: ProductionSemanticScalarTypeV2::Bool, .. })));
                            let index = original.assignment_scalar_definition_v30(0, 0,
                                SemanticBlockIdV1::from_index(block as u32), statement as u32, budget)?.unwrap();
                            let input = original.inventory.definitions()[index].coordinate;
                            let descendants = optimized.definition_descendants(input, budget)?;
                            assert_eq!(descendants.len(), 1);
                            let actual = optimized_source_definition_row_v18(check.inventory, descendants[0].output, budget)?.value.unwrap();
                            check.normalize(&expression, actual, budget)?;
                            drop(expression);
                            count += 1;
                        }
                    }
                    assert!(count > 0);
                    reached.set(true);
                    Ok(())
                })
            })
        }).unwrap();
        assert!(reached.get());
    }
}

#[test]
fn issued_discriminant_rejects_copied_source_place_wrong_type_and_output_row() {
    for fault in [7, 0, 1, 2, 3, 4, 5, 6, 8, 7] {
        let reached = std::cell::Cell::new(false);
        let result = with_issued_source_v31(true, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let function = leaves.original.original_function(0, budget)?;
                        let (function_id, _) = original.source.instance(0, 0, budget)?;
                        let (block, statement, assignment, place) = function
                            .blocks()
                            .iter()
                            .enumerate()
                            .find_map(|(block, row)| {
                                row.statements()
                                    .iter()
                                    .enumerate()
                                    .find_map(|(statement, row)| {
                                        let SemanticStatementKindV1::Assign(assignment) =
                                            row.kind()
                                        else {
                                            return None;
                                        };
                                        let SemanticRvalueKindV1::Discriminant(place) =
                                            assignment.value().kind()
                                        else {
                                            return None;
                                        };
                                        Some((block, statement, assignment, place))
                                    })
                            })
                            .unwrap();
                        let ty = assignment.value().result_type();
                        let scalar = kir_semantic_scalar_v1(
                            &lower_scalar_type(
                                original.source.source_semantic(budget)?.types(),
                                ty,
                            )
                            .map_err(source_emission_error_v18)?,
                        )
                        .unwrap();
                        let output = leaves.presences[0];
                        let source = leaves.original.leaves.presences.rows[output.original];
                        leaves.original.leaves.presence_row_v31(&source, budget)?;
                        assert_eq!(
                            leaves.read(leaves.function.function, output.value, budget)?,
                            Some(NormalizedScalarExpressionV1::Symbol {
                                symbol: source.symbol,
                                scalar: ProductionSemanticScalarTypeV2::Bool,
                            })
                        );
                        check.normalize(
                            &ProductionSemanticExpressionV2::Symbol {
                                symbol: source.symbol,
                                scalar: ProductionSemanticScalarTypeV2::Bool,
                            },
                            output.value,
                            budget,
                        )?;
                        reached.set(true);
                        if fault == 7 {
                            return Ok(());
                        }
                        if fault < 2 {
                            let copied = place.clone();
                            let mut remaining =
                                fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                            return check
                                .index
                                .issued_discriminant_v31(
                                    leaves.original,
                                    0,
                                    function_id,
                                    function,
                                    EntrySiteV20::Statement {
                                        block: BoundaryBlockV31::new(block as u32),
                                        statement: statement as u32,
                                    },
                                    assignment.value(),
                                    if fault == 0 { &copied } else { place },
                                    ty,
                                    if fault == 1 {
                                        ProductionSemanticScalarTypeV2::Bool
                                    } else {
                                        scalar
                                    },
                                    0,
                                    &mut remaining,
                                    budget,
                                )
                                .map(|_| ());
                        }
                        if fault < 5 || fault == 8 {
                            let mut changed = source;
                            match fault {
                                2 => changed.instance += 1,
                                3 => {
                                    changed.definition = EntryValueV20::BlockArgument {
                                        block: BoundaryBlockV31::new(0),
                                        variable: BoundaryVariableV31::new(0),
                                    }
                                }
                                8 => changed.issuer = usize::MAX,
                                _ => changed.value = ValueId(u32::MAX),
                            }
                            return leaves.original.leaves.presence_row_v31(&changed, budget);
                        }
                        if fault == 5 {
                            let forged = [OptimizedIssuedPresenceV31 {
                                value: ValueId(u32::MAX),
                                ..output
                            }];
                            let altered = ProductionOptimizedSourceScalarLeavesV18 {
                                original: leaves.original,
                                optimized: leaves.optimized,
                                function: leaves.function,
                                reads: leaves.reads,
                                wrapping: leaves.wrapping,
                                boundaries: leaves.boundaries,
                                presences: &forged,
                                slot: leaves.slot,
                                ledger: leaves.ledger,
                                floor: leaves.floor,
                            };
                            return altered
                                .read(leaves.function.function, ValueId(u32::MAX), budget)
                                .map(|_| ());
                        }
                        let wrong = ProductionSemanticExpressionV2::Symbol {
                            symbol: source.symbol,
                            scalar,
                        };
                        check.normalize(&wrong, output.value, budget)
                    })
                },
            )
        });
        assert!(
            reached.get(),
            "positive same-owner baseline failed: {result:?}"
        );
        let expected = match fault {
            0 => "issued discriminant original place differs",
            1 => "issued discriminant declared encoding or type is unsupported",
            2..=4 => "issued presence original issuer changed",
            5 => "optimized issued presence actual predicate changed",
            6 => "issued presence symbol type differs",
            8 => "issued presence original issuer absent",
            _ => {
                result.unwrap();
                continue;
            }
        };
        assert_binding(result, expected);
    }
}

fn issued_presence_cut_v31(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let measured = std::cell::Cell::new(None);
    let result = with_issued_source_v31(true, |original, optimized, budget| {
        source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
            let remaining = match cut {
                Some((false, amount)) => amount,
                _ => ISSUED_STORAGE_V31 / 2,
            };
            budget.reserve_storage(ISSUED_STORAGE_V31 - budget.storage() - remaining)?;
            if let Some((true, amount)) = cut {
                budget.charge_work(ISSUED_WORK_V31 - budget.work() - amount)?;
            }
            let before = (budget.work(), budget.storage());
            let entered = std::cell::Cell::new(false);
            let result = original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, _| {
                    assert!(!leaves.presences.is_empty());
                    entered.set(true);
                    Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                        "selected issued presence stop",
                    ))
                },
            );
            measured.set(Some((
                budget.work() - before.0,
                budget.peak_storage() - before.1,
                entered.get(),
            )));
            match (&result, entered.get(), cut) {
                (
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "selected issued presence stop",
                    )),
                    true,
                    _,
                ) => {}
                (
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                        error,
                    ))),
                    false,
                    Some((true, _)),
                ) => assert!(error.actual() > error.limit()),
                (
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                        error,
                    ))),
                    false,
                    Some((false, _)),
                ) => assert!(error.actual() > error.limit()),
                _ => panic!("unexpected issued presence cut {result:?}"),
            }
            result
        })
    });
    assert!(result.is_err());
    measured.get().unwrap()
}

#[test]
fn issued_discriminant_exact_and_one_short_work_and_storage_keep_existing_caps() {
    let (work, storage, entered) = issued_presence_cut_v31(None);
    assert!(entered && work > 0 && storage > 0);
    for (work_kind, exact) in [(true, work), (false, storage)] {
        assert!(issued_presence_cut_v31(Some((work_kind, exact))).2);
        assert!(!issued_presence_cut_v31(Some((work_kind, exact - 1))).2);
    }
}

#[test]
fn issued_discriminant_encoding_rejects_changed_variant_value_type_and_inhabitedness() {
    let owner = mixed_licm_source_v28(false);
    let types = owner.source_semantic().types();
    let (option, original, discriminant, variants) = types
        .iter()
        .enumerate()
        .find_map(|(index, ty)| {
            let SemanticTypeShapeV1::Enum {
                discriminant,
                variants,
            } = ty.shape()
            else {
                return None;
            };
            let option = SemanticTypeIdV1::from_index(index as u32);
            option_payload_type_v1(types, option)?;
            Some((option, ty, *discriminant, variants))
        })
        .unwrap();
    let scalar = kir_semantic_scalar_v1(&lower_scalar_type(types, discriminant).unwrap()).unwrap();
    issued_discriminant_encoding_v31(types, option, discriminant, scalar).unwrap();
    for fault in 0..3 {
        let mut changed = types.to_vec();
        let mut cases = variants.to_vec();
        cases[1] = SemanticEnumVariantV1::new_with_inhabitedness(
            if fault == 0 {
                2
            } else {
                cases[1].discriminant()
            },
            cases[1].fields().clone(),
            fault == 1,
        );
        changed[option.index() as usize] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            original.layout().clone(),
            SemanticTypeShapeV1::Enum {
                discriminant,
                variants: cases.into_boxed_slice(),
            },
        )
        .with_rustc_abi_properties(original.abi_properties())
        .with_rust_type_kind(original.rust_type_kind());
        assert!(matches!(
            issued_discriminant_encoding_v31(
                &changed,
                option,
                if fault == 2 { option } else { discriminant },
                scalar
            ),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "issued discriminant declared encoding or type is unsupported"
            ))
        ));
    }
    issued_discriminant_encoding_v31(types, option, discriminant, scalar).unwrap();
}

#[test]
fn issued_presence_fixed_frames_match_independent_fields_and_result_envelopes() {
    type Row = (
        usize,
        usize,
        EntryValueV20,
        fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        ValueId,
        u32,
    );
    type Owner = (
        Vec<Row>,
        Vec<(ValueId, usize)>,
        Vec<(usize, EntryValueV20, usize)>,
    );
    type Output = (
        usize,
        fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        ValueId,
    );
    assert_eq!(size_of::<Row>(), size_of::<SourceIssuedPresenceV31>());
    assert_eq!(
        std::mem::align_of::<Row>(),
        std::mem::align_of::<SourceIssuedPresenceV31>()
    );
    assert_eq!(size_of::<Owner>(), size_of::<SourceIssuedPresencesV31>());
    assert_eq!(size_of::<Output>(), size_of::<OptimizedIssuedPresenceV31>());
    fn envelope<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, ProductionSourceOwnedViewErrorV18>>()
    }
    let query = size_of::<(
        &ProductionSourceCorrespondenceV18<'_>,
        usize,
        &mut ArgumentBudgetV1<'_>,
    )>() + 2 * size_of::<SourceOwnedResultV18<&[PendingSourceIssuedIssuerV29]>>()
        + 2 * size_of::<&()>();
    assert_eq!(
        scoped_raw_admission_v29::source_issuer_query_headers_v31().unwrap(),
        query
    );
    let source = query
        + envelope::<Vec<Row>>()
        + envelope::<Owner>()
        + envelope::<Vec<(ValueId, usize)>>()
        + envelope::<Vec<(usize, EntryValueV20, usize)>>()
        + envelope::<Row>()
        + envelope::<Option<&Row>>()
        + envelope::<&PendingSourceIssuedIssuerV29>()
        + envelope::<&[PendingSourceIssuedIssuerV29]>()
        + envelope::<ProductionSemanticExpressionV2>()
        + envelope::<OriginalEntryDefinitionRowV20>()
        + envelope::<EntryValueV20>()
        + envelope::<(SemanticTypeIdV1, &[SemanticEnumVariantV1])>()
        + envelope::<(ValueId, usize)>()
        + envelope::<(usize, EntryValueV20, usize)>()
        + size_of::<std::slice::Iter<'_, (ValueId, usize)>>()
        + size_of::<std::slice::Iter<'_, (usize, EntryValueV20, usize)>>()
        + size_of::<std::slice::Iter<'_, Row>>()
        + size_of::<std::slice::Iter<'_, PendingSourceIssuedIssuerV29>>()
        + 12 * size_of::<usize>()
        + 12 * size_of::<&()>();
    assert_eq!(source_presence_headers_v31().unwrap(), source);
    type Prepared<'a> = (
        Vec<OptimizedSourceScalarReadV18>,
        Vec<SourceWrappingValueV23>,
        OptimizedSourceScalarBoundariesV31,
        Vec<Output>,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        usize,
    );
    type Capture<'a> = (
        &'a ProductionOptimizedSourceScalarLeavesV18<'a>,
        &'a mut (),
        &'a mut ArgumentBudgetV1<'a>,
    );
    assert_eq!(
        size_of::<Prepared<'_>>(),
        size_of::<OptimizedScalarPreparedV31<'_>>()
    );
    assert_eq!(
        size_of::<Capture<'_>>(),
        size_of::<OptimizedScalarCatchFrameV31<'_>>()
    );
    assert_eq!(
        std::mem::align_of::<Capture<'_>>(),
        std::mem::align_of::<OptimizedScalarCatchFrameV31<'_>>()
    );
    let output = envelope::<Prepared<'_>>()
        + size_of::<Capture<'_>>()
        + std::mem::align_of::<Capture<'_>>()
        + size_of::<std::panic::AssertUnwindSafe<Capture<'_>>>()
        + envelope::<Vec<Output>>()
        + envelope::<Output>()
        + envelope::<Option<&Row>>()
        + envelope::<&PendingSourceIssuedIssuerV29>()
        + size_of::<std::slice::Iter<'_, Row>>()
        + 8 * size_of::<usize>()
        + 8 * size_of::<&()>();
    assert_eq!(
        slice_view_v1::optimized_presence_headers_v31().unwrap(),
        output
    );
    type Query<'a> = (
        &'a OriginalEntryIndexV20<'a, 'a>,
        &'a ProductionSourceScalarLeavesV18<'a>,
        usize,
        SemanticFunctionIdV1,
        &'a SemanticFunctionDeclV1,
        EntrySiteV20,
        &'a PrivateRvalueV22,
        &'a SemanticPlaceV1,
        SemanticTypeIdV1,
        ProductionSemanticScalarTypeV2,
        usize,
        &'a mut usize,
        &'a mut ArgumentBudgetV1<'a>,
    );
    let query = size_of::<Query<'_>>()
        + std::mem::align_of::<Query<'_>>()
        + size_of::<OriginalEntryDefinitionRowV20>()
        + size_of::<EntryValueV20>()
        + envelope::<ProductionSemanticExpressionV2>()
        + size_of::<SourceOwnedResultV18<()>>()
        + 8 * size_of::<&()>();
    assert_eq!(issued_discriminant_query_headers_v31().unwrap(), query);
}
