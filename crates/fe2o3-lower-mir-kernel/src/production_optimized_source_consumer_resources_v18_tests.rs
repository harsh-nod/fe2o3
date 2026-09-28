fn visit_two_contracts_v18(
    collector: &mut TranslationContractCollectorV18<'_, '_, u64>,
) -> Result<(), ProductionMirPlironTranslationErrorV1> {
    for value in [2u64, 1] {
        collector.charge(1)?;
        collector.push(value)?;
    }
    Ok(())
}

fn normalized_node_cleanup_fixture_v18() -> ScopedSourceCleanupV29 {
    ScopedSourceCleanupV29 {
        denied: std::cell::Cell::new(false),
        fault: std::cell::RefCell::new(None),
        fault_storage: std::cell::Cell::new(None),
        fault_skip: std::cell::Cell::new(0),
    }
}

fn strict_normalized_header_oracle_v18() -> usize {
    use std::mem::size_of;
    type Node = NormalizedScalarExpressionV1;
    size_of::<SourceTranslationChargeV18<'_, '_, '_, '_>>()
        + size_of::<SourceOwnedResultV18<SourceTranslationChargeV18<'_, '_, '_, '_>>>()
        + size_of::<Vec<Node>>()
        + size_of::<Result<Vec<Node>, ProductionSemanticKirErrorV1>>()
        + size_of::<Result<Box<[Node; 1]>, Vec<Node>>>()
        + size_of::<Box<[Node]>>()
        + size_of::<NormalizedScalarNodeV18>()
        + 2 * size_of::<Option<Node>>()
        + size_of::<ProductionSourceOwnedViewErrorV18>()
}

#[test]
fn qualified_ranked_query_header_oracle_preserves_exact_first_storage_refusal() {
    let expected = size_of::<QualifiedSourceRankedIndexV18<'_, '_, '_, '_, '_, '_, '_, '_>>()
        + size_of::<
            SourceOwnedResultV18<QualifiedSourceRankedIndexV18<'_, '_, '_, '_, '_, '_, '_, '_>>,
        >()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Result<SourceOwnedResultV18<()>, ArgumentResourceV1>>()
        + size_of::<&QualifiedSourceRankedDataV18<'_, '_, '_, '_>>()
        + size_of::<&fe2o3_pliron::ProductionRankedKernelV1>();
    assert_eq!(
        qualified_source_ranked_query_headers_v18().unwrap(),
        expected
    );
    for one_short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(one_short));
        let cleanup = normalized_node_cleanup_fixture_v18();
        {
            let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
            let reservation = ledger.with_budget(|budget| {
                budget.reserve_storage(qualified_source_ranked_query_headers_v18()?)
            });
            if one_short {
                let Err(ArgumentResourceV1::Storage(error)) = reservation else {
                    panic!("unpaid query header");
                };
                assert_eq!((error.actual(), error.limit()), (expected, expected - 1));
                assert_eq!(
                    ledger.failure.get(),
                    Some(ArgumentResourceV1::Storage(error))
                );
                assert_eq!(
                    ledger.with_budget(|_| panic!("first refusal must stop the query")),
                    Err::<(), _>(ArgumentResourceV1::Storage(error))
                );
            } else {
                reservation.unwrap();
                assert_eq!(ledger.failure.get(), None);
            }
        }
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), if one_short { 0 } else { expected });
        assert_eq!(budget.failed_storage(), one_short.then_some(expected));
    }
}

#[test]
fn normalized_boxed_array_wrapper_preserves_the_measured_legacy_layout() {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    enum LegacyShape {
        Symbol {
            symbol: u32,
            scalar: ProductionSemanticScalarTypeV2,
        },
        Constant {
            scalar: ProductionSemanticScalarTypeV2,
            bits: u64,
        },
        Load {
            site: SemanticAccessSiteV1,
            scalar: ProductionSemanticScalarTypeV2,
        },
        Unary {
            operation: ProductionSemanticUnaryOpV2,
            scalar: ProductionSemanticScalarTypeV2,
            operand: Box<Self>,
        },
        Binary {
            operation: ProductionSemanticBinaryOpV2,
            scalar: ProductionSemanticScalarTypeV2,
            overflow: ProductionOverflowContractV2,
            lhs: Box<Self>,
            rhs: Box<Self>,
        },
        Compare {
            operation: ProductionSemanticComparisonV2,
            operand_scalar: ProductionSemanticScalarTypeV2,
            lhs: Box<Self>,
            rhs: Box<Self>,
        },
        Select {
            scalar: ProductionSemanticScalarTypeV2,
            condition: Box<Self>,
            when_true: Box<Self>,
            when_false: Box<Self>,
        },
        Cast {
            kind: ProductionSemanticCastV2,
            source: ProductionSemanticScalarTypeV2,
            target: ProductionSemanticScalarTypeV2,
            operand: Box<Self>,
        },
    }
    assert_eq!(
        size_of::<NormalizedScalarNodeV18>(),
        size_of::<Box<LegacyShape>>()
    );
    assert_eq!(
        align_of::<NormalizedScalarNodeV18>(),
        align_of::<Box<LegacyShape>>()
    );
    assert_eq!(
        size_of::<NormalizedScalarExpressionV1>(),
        size_of::<LegacyShape>()
    );
    assert_eq!(
        align_of::<NormalizedScalarExpressionV1>(),
        align_of::<LegacyShape>()
    );
    let legacy = NormalizedScalarExpressionV1::Constant {
        scalar: ProductionSemanticScalarTypeV2::Bool,
        bits: 1,
    };
    let expected = format!("{legacy:?}");
    let boxed = NormalizedScalarNodeV18::legacy(legacy);
    assert_eq!(format!("{boxed:?}"), expected);
    assert_eq!(
        boxed.clone(),
        boxed,
        "legacy clone remains the same expression algebra"
    );
}

#[test]
fn strict_normalized_header_one_short_refuses_before_node_or_allocation_work() {
    let headers = strict_normalized_header_oracle_v18();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, headers - 1);
    let cleanup = normalized_node_cleanup_fixture_v18();
    {
        let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
        let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error))) =
            SourceTranslationChargeV18::new(&ledger, 0)
        else {
            panic!("header must be prepaid");
        };
        assert_eq!(error.actual(), headers);
        assert_eq!(
            ledger.failure.get(),
            Some(ArgumentResourceV1::Storage(error))
        );
    }
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), 0);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.failed_storage(), Some(headers));
}

#[test]
fn strict_normalized_node_prepays_exact_conversion_and_payload_storage() {
    use std::mem::size_of;
    const FLOOR: usize = 11;
    let headers = strict_normalized_header_oracle_v18();
    assert_eq!(SourceTranslationChargeV18::headers().unwrap(), headers);
    for one_short in [false, true] {
        let required = FLOOR + headers + size_of::<NormalizedScalarExpressionV1>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
        let mut budget = ArgumentBudgetV1::new(&mut work, required - usize::from(one_short));
        budget.reserve_storage(FLOOR).unwrap();
        let cleanup = normalized_node_cleanup_fixture_v18();
        {
            let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
            let mut charge = SourceTranslationChargeV18::new(&ledger, 4).unwrap();
            charge.begin_normalized_tree().unwrap();
            charge.normalized_node().unwrap();
            let boxed = charge.boxed_normalized_node(NormalizedScalarExpressionV1::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1,
            });
            if one_short {
                assert!(boxed.is_none());
                let Some(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                    error,
                ))) = charge.error.as_ref()
                else {
                    panic!("payload must fail before allocation");
                };
                assert_eq!(error.actual(), required);
                assert_eq!(
                    ledger.failure.get(),
                    Some(ArgumentResourceV1::Storage(*error))
                );
                assert_eq!(charge.node_credits, 0);
                assert!(charge.finish_normalized_pair().is_none());
            } else {
                let boxed = boxed.unwrap();
                assert!(matches!(
                    boxed.as_ref(),
                    NormalizedScalarExpressionV1::Constant { bits: 1, .. }
                ));
                assert_eq!(
                    charge.node_credits,
                    size_of::<NormalizedScalarExpressionV1>()
                );
                assert_eq!(
                    ledger.with_budget(|budget| Ok(budget.storage())).unwrap(),
                    required
                );
                drop(boxed);
                charge.finish_normalized_pair().unwrap();
                assert_eq!(charge.node_credits, 0);
                assert_eq!(
                    ledger.with_budget(|budget| Ok(budget.storage())).unwrap(),
                    FLOOR + headers
                );
            }
        }
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.storage(), FLOOR + headers);
        assert_eq!(budget.failed_storage(), one_short.then_some(required));
        budget.release_storage(headers).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn strict_expected_and_actual_nodes_coexist_until_both_are_dropped() {
    use std::mem::size_of;
    let headers = strict_normalized_header_oracle_v18();
    let bytes = size_of::<NormalizedScalarExpressionV1>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
    let mut budget = ArgumentBudgetV1::new(&mut work, headers + 2 * bytes);
    let cleanup = normalized_node_cleanup_fixture_v18();
    {
        let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
        let mut charge = SourceTranslationChargeV18::new(&ledger, 8).unwrap();
        let mut build = || {
            charge.begin_normalized_tree().unwrap();
            charge.normalized_node().unwrap();
            charge
                .boxed_normalized_node(NormalizedScalarExpressionV1::Constant {
                    scalar: ProductionSemanticScalarTypeV2::Bool,
                    bits: 0,
                })
                .unwrap()
        };
        let expected = build();
        let actual = build();
        drop(build);
        assert_eq!(charge.node_credits, 2 * bytes);
        assert_eq!(
            ledger.with_budget(|budget| Ok(budget.storage())).unwrap(),
            headers + 2 * bytes
        );
        drop(expected);
        assert_eq!(
            ledger.with_budget(|budget| Ok(budget.storage())).unwrap(),
            headers + 2 * bytes
        );
        drop(actual);
        charge.finish_normalized_pair().unwrap();
        assert_eq!(
            ledger.with_budget(|budget| Ok(budget.storage())).unwrap(),
            headers
        );
    }
    budget.release_storage(headers).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 8);
}

#[test]
fn strict_expanded_node_limit_cannot_be_reset_after_refusal() {
    let nodes = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(nodes + 1);
    let mut budget = ArgumentBudgetV1::new(&mut work, strict_normalized_header_oracle_v18());
    let cleanup = normalized_node_cleanup_fixture_v18();
    {
        let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
        let mut charge = SourceTranslationChargeV18::new(&ledger, nodes + 1).unwrap();
        charge.begin_normalized_tree().unwrap();
        for _ in 0..nodes {
            charge.normalized_node().unwrap();
        }
        assert!(charge.normalized_node().is_none());
        assert!(matches!(
            charge.error,
            Some(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        assert!(charge.begin_normalized_tree().is_none());
        assert!(charge.charge_many(1).is_none());
        assert!(
            ledger.failure.get().is_none(),
            "a node limit is not a fabricated work denial"
        );
    }
    assert_eq!(budget.work(), nodes);
    assert_eq!(budget.failed_storage(), None);
}

struct StrictConstantLeavesV18;

impl SemanticExpressionLeavesV18 for StrictConstantLeavesV18 {
    fn symbol(
        &self,
        _: u32,
        _: ProductionSemanticScalarTypeV2,
        _: &mut dyn CorrelationChargeV18,
    ) -> Option<NormalizedScalarExpressionV1> {
        panic!("strict normalization leaf panic")
    }

    fn load(
        &self,
        _: &fe2o3_pliron::ProductionSemanticLoadV2,
        _: &mut dyn CorrelationChargeV18,
    ) -> Option<NormalizedScalarExpressionV1> {
        None
    }
}

fn strict_binary_expression_v18(panic_rhs: bool) -> ProductionSemanticExpressionV2 {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    ProductionSemanticExpressionV2::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 7 }),
        rhs: Box::new(if panic_rhs {
            ProductionSemanticExpressionV2::Symbol { symbol: 0, scalar }
        } else {
            ProductionSemanticExpressionV2::Constant { scalar, bits: 9 }
        }),
    }
}

#[test]
fn strict_real_normalization_prepays_each_child_without_a_maximum_tree_reservation() {
    let expression = strict_binary_expression_v18(false);
    let headers = strict_normalized_header_oracle_v18();
    let payload = 2 * size_of::<NormalizedScalarExpressionV1>();
    // Three visited nodes each pay construction and future destruction;
    // two one-element allocations each pay emission's three fixed steps.
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(12);
        let mut budget = ArgumentBudgetV1::new(&mut work, headers + payload - usize::from(short));
        let cleanup = normalized_node_cleanup_fixture_v18();
        {
            let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
            let mut charge = SourceTranslationChargeV18::new(&ledger, 12).unwrap();
            charge.begin_normalized_tree().unwrap();
            let normalized = normalize_semantic_expression_v18(
                &expression,
                &StrictConstantLeavesV18,
                0,
                &mut charge,
            );
            assert_eq!(
                charge.remaining_nodes,
                Some(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 - 3)
            );
            if short {
                assert!(normalized.is_none());
                assert_eq!(charge.node_credits, payload / 2);
                let Some(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                    error,
                ))) = charge.error.as_ref()
                else {
                    panic!("exact second child allocation must refuse");
                };
                assert_eq!(error.actual(), headers + payload);
                assert!(charge.finish_normalized_pair().is_none());
            } else {
                let normalized = normalized.unwrap();
                let NormalizedScalarExpressionV1::Binary { lhs, rhs, .. } = &normalized else {
                    panic!();
                };
                assert!(matches!(
                    lhs.as_ref(),
                    NormalizedScalarExpressionV1::Constant { bits: 7, .. }
                ));
                assert!(matches!(
                    rhs.as_ref(),
                    NormalizedScalarExpressionV1::Constant { bits: 9, .. }
                ));
                assert_eq!(charge.node_credits, payload);
                drop(normalized);
                charge.finish_normalized_pair().unwrap();
            }
        }
        assert_eq!(budget.work(), 12);
        assert_eq!(
            budget.peak_storage(),
            headers + if short { payload / 2 } else { payload }
        );
        assert_eq!(
            budget.storage(),
            headers + if short { payload / 2 } else { 0 }
        );
        assert_eq!(budget.failed_storage(), short.then_some(headers + payload));
    }
}

#[test]
fn strict_partial_tree_unwind_drops_before_eligible_scope_refund() {
    let expression = strict_binary_expression_v18(true);
    let headers = strict_normalized_header_oracle_v18();
    let bytes = size_of::<NormalizedScalarExpressionV1>();
    const FLOOR: usize = 13;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(9);
    let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + headers + bytes);
    budget.reserve_storage(FLOOR).unwrap();
    let cleanup = normalized_node_cleanup_fixture_v18();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scoped_source_attempt_v29(&cleanup, &mut budget, FLOOR, |budget| {
            let ledger = CorrelationLedgerV18::new(budget, &cleanup);
            let mut charge = SourceTranslationChargeV18::new(&ledger, 9)?;
            charge.begin_normalized_tree().unwrap();
            let _tree = normalize_semantic_expression_v18(
                &expression,
                &StrictConstantLeavesV18,
                0,
                &mut charge,
            );
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
    }));
    assert!(caught.is_err());
    assert_eq!(budget.work(), 9);
    assert_eq!(budget.peak_storage(), FLOOR + headers + bytes);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_storage(), None);
    assert!(!cleanup.denied.get());
}

fn strict_shared_dag_function_v18() -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    ));
    for index in 1..=2 {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(index), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(index - 1),
                rhs: ValueId(index - 1),
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::kernel_entry(
        "strict_shared_dag",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    )
}

#[test]
fn strict_shared_dag_expansion_pays_all_seven_nodes_and_six_distinct_children() {
    let function = strict_shared_dag_function_v18();
    let mut setup = UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 };
    let kir =
        build_kir_correlation_index(function.body.as_ref().unwrap(), 100, &mut setup).unwrap();
    let cleanup = normalized_node_cleanup_fixture_v18();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    {
        let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
        let mut charge = SourceTranslationChargeV18::new(&ledger, 100_000).unwrap();
        charge.begin_normalized_tree().unwrap();
        let mut visiting = SourceScalarVisitingV18 {
            rows: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
            length: 0,
        };
        let normalized =
            native_helper_value_expansion_v1::with_source_value_expansion_v18(&ledger, |helpers| {
                normalize_kir_expression_with_visiting_v18(
                    &function,
                    &kir,
                    &BTreeMap::new(),
                    ValueId(2),
                    0,
                    &mut visiting,
                    &mut charge,
                    helpers,
                )
                .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)
            })
            .unwrap();
        assert_eq!(visiting.length, 0);
        assert_eq!(
            charge.remaining_nodes,
            Some(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 - 7)
        );
        assert_eq!(
            charge.node_credits,
            6 * size_of::<NormalizedScalarExpressionV1>()
        );
        let NormalizedScalarExpressionV1::Binary { lhs, rhs, .. } = &normalized else {
            panic!();
        };
        assert_eq!(lhs, rhs);
        assert!(
            !std::ptr::eq(lhs.as_ref(), rhs.as_ref()),
            "tree children cannot share allocation credits"
        );
        drop(normalized);
        charge.finish_normalized_pair().unwrap();
        assert_eq!(
            ledger.with_budget(|budget| Ok(budget.storage())).unwrap(),
            strict_normalized_header_oracle_v18()
        );
    }
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn source_helper_entrance_refuses_before_legacy_argument_clone_or_template_expansion() {
    let mut function = strict_shared_dag_function_v18();
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: FunctionId::new("unadmitted_helper"),
                arguments: vec![ValueId(2)],
            },
        ));
    let mut setup = UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 };
    let kir =
        build_kir_correlation_index(function.body.as_ref().unwrap(), 100, &mut setup).unwrap();
    let cleanup = normalized_node_cleanup_fixture_v18();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    {
        let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
        let mut charge = SourceTranslationChargeV18::new(&ledger, 0).unwrap();
        let mut visiting = SourceScalarVisitingV18 {
            rows: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
            length: 0,
        };
        native_helper_value_expansion_v1::with_source_value_expansion_v18(&ledger, |helpers| {
            let before = ledger
                .with_budget(|budget| Ok((budget.work(), budget.storage())))
                .unwrap();
            let result = helpers.call(
                &function,
                &kir,
                &BTreeMap::new(),
                FunctionOperationLocation::new(BlockId(0), 3),
                &function.body.as_ref().unwrap().blocks[0].operations[3],
                &[ValueId(2)],
                0,
                &mut visiting,
                &mut charge,
            );
            assert!(result.is_none());
            assert_eq!(
                ledger
                    .with_budget(|budget| Ok((budget.work(), budget.storage())))
                    .unwrap(),
                before
            );
            assert_eq!(visiting.length, 0);
            assert_eq!(charge.node_credits, 0);
            assert!(charge.error.is_none());
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), strict_normalized_header_oracle_v18());
}

#[test]
fn qualified_solver_keys_preserve_original_ordinals_and_separate_invocations() {
    let value = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    let extent = ProductionRankedOutputExtentSourceV1::new(2, value, value, value);
    let base = ProductionRankedAccessSourceV1::new(3, Some(1), 5, 4, 7).with_output_extent(extent);
    let mut rows = [
        ProductionSourceRankedAccessV18::new(0, SemanticFunctionIdV1::from_index(2), base),
        ProductionSourceRankedAccessV18::new(
            0,
            SemanticFunctionIdV1::from_index(2),
            ProductionRankedAccessSourceV1::new(3, Some(1), 9, 4, 8),
        ),
        ProductionSourceRankedAccessV18::new(1, SemanticFunctionIdV1::from_index(2), base),
    ];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(24);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let mut previous = None;
    for row in &mut rows {
        let site = row.original();
        let key = source_ranked_key_after_v18(previous, site, &mut budget).unwrap();
        row.solver = Some(key);
        previous = Some((site, key));
    }
    assert_eq!(rows[0].solver.unwrap().block, 0);
    assert_eq!(rows[0].solver.unwrap().ordinal, 0);
    assert_eq!(rows[1].solver.unwrap().block, 0);
    assert_eq!(rows[1].solver.unwrap().ordinal, 1);
    assert_eq!(rows[2].solver.unwrap().block, 1);
    assert_eq!(rows[2].solver.unwrap().ordinal, 0);
    assert_eq!(
        (rows[0].original().ordinal, rows[1].original().ordinal),
        (5, 9)
    );
    assert_eq!(rows[0].original().instance, 0);
    assert_eq!(rows[2].original().instance, 1);
    let reader = RankedIndexSourcesV18::Qualified(&rows);
    let decoded = reader.source(0).unwrap();
    assert_eq!(decoded.output_extent(), Some(extent));
    assert_eq!((decoded.ranked_block(), decoded.ranked_operation()), (4, 7));
    assert_eq!(
        rows[0].projected, base,
        "decoding cannot rewrite original source or extent metadata"
    );
    assert!(reader.source(3).is_none());
    assert_eq!(budget.work(), 24);
    assert_eq!(budget.storage(), 0);
    // This is only key algebra. Equal ranked locations above must still fail
    // the complete index/census constructor, not be treated as two accesses.
}

#[test]
fn qualified_key_work_refusal_and_duplicates_cannot_assign_a_fresh_site() {
    let row = ProductionSourceRankedAccessV18::new(
        0,
        SemanticFunctionIdV1::from_index(0),
        ProductionRankedAccessSourceV1::new(0, None, 0, 0, 0),
    );
    let site = row.original();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    assert!(
        matches!(source_ranked_key_after_v18(None, site, &mut budget),
        Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
            if error.actual() == 8)
    );
    assert_eq!(budget.work(), 0);
    assert!(row.solver.is_none());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let key = source_ranked_key_after_v18(None, site, &mut budget).unwrap();
    assert!(matches!(
        source_ranked_key_after_v18(Some((site, key)), site, &mut budget),
        Err(ProductionSourceOwnedViewErrorV18::Binding(_))
    ));
    assert_eq!(budget.work(), 16);
    assert!(row.solver.is_none());
}

#[test]
fn translation_contract_collection_has_independent_exact_storage_and_work_oracles() {
    use std::mem::size_of;
    // A function item captures zero bytes. Two fixed-size records pay their
    // backing before allocation. [2, 1] already forms a max-heap: its build
    // costs 1 + 1 + 8, and the final swap/empty sift costs 1 + 1.
    let headers = size_of::<Vec<u64>>()
        + size_of::<TranslationContractCollectorV18<'_, '_, u64>>()
        + size_of::<Option<ArgumentResourceV1>>()
        + size_of::<Result<(), ProductionMirPlironTranslationErrorV1>>()
        + size_of::<Result<Vec<u64>, SourceTranslationCoreErrorV18>>()
        + size_of::<usize>();
    let bytes = headers + 2 * size_of::<u64>();
    let work_limit = 23;
    for (storage, work, accepted) in [
        (bytes, work_limit, true),
        (bytes - 1, work_limit, false),
        (headers - 1, work_limit, false),
        (bytes, work_limit - 1, false),
    ] {
        let mut work_budget = CanonicalKernelIrWorkBudgetV1::new(work);
        let mut budget = ArgumentBudgetV1::new(&mut work_budget, storage);
        let result = source_translation_contracts_v18(visit_two_contracts_v18, &mut budget);
        match result {
            Ok(rows) => {
                assert!(accepted);
                assert_eq!(rows, [1, 2]);
                assert_eq!(rows.capacity(), 2);
                assert_eq!(budget.storage(), bytes);
                assert_eq!(budget.peak_storage(), bytes);
                assert_eq!(budget.work(), work_limit);
                drop(rows);
                budget.release_storage(bytes).unwrap();
                assert_eq!(budget.storage(), 0);
            }
            Err(SourceTranslationCoreErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            )) => {
                assert!(!accepted);
                match error {
                    ArgumentResourceV1::Storage(error) if storage == headers - 1 => {
                        assert_eq!(error.actual(), headers);
                        assert_eq!(budget.storage(), 0);
                        assert_eq!(budget.work(), 0);
                    }
                    ArgumentResourceV1::Storage(error) => {
                        assert_eq!(error.actual(), bytes);
                        assert_eq!(budget.storage(), headers);
                        assert_eq!(budget.work(), 7);
                    }
                    ArgumentResourceV1::Work(error) => {
                        assert_eq!(error.actual(), work_limit);
                        assert_eq!(budget.work(), work_limit - 1);
                        assert_eq!(budget.storage(), bytes);
                    }
                    error => panic!("unexpected contract refusal: {error:?}"),
                }
            }
            other => panic!("unexpected contract collection result: {other:?}"),
        }
    }
}

#[test]
fn legacy_contract_collection_preserves_its_zero_charge_path() {
    let mut rows = Vec::new();
    visit_two_contracts_v18(&mut TranslationContractCollectorV18::Legacy(&mut rows)).unwrap();
    assert_eq!(rows, [2, 1]);
}

#[test]
fn actual_output_allocation_scratch_uses_dense_rows_and_exact_incidence_capacity() {
    run_production_optimized_consumer_v18(
        two_descriptor_reads_at_one_source_site_v18,
        |original, optimized, budget| {
            use std::mem::size_of;
            let floor = budget.storage();
            let inventory = optimized.output_inventory(budget)?;
            let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
            let definitions = function.definitions.end - function.definitions.start;
            let incidences = (function.uses.end - function.uses.start)
                + (function.edge_arguments.end - function.edge_arguments.start)
                + 1;
            let mut scratch =
                SourceAllocationScratchV18::build(inventory, function.coordinate, budget)?;
            assert_eq!(scratch.marks.len(), definitions);
            assert_eq!(scratch.pending_limit, incidences);
            assert_eq!(scratch.pending.capacity(), incidences);
            assert_eq!(scratch.marks.capacity(), definitions);
            assert_eq!(
                budget.storage() - floor,
                size_of::<SourceAllocationScratchV18>()
                    + size_of::<SourceOwnedResultV18<SourceAllocationScratchV18>>()
                    + size_of::<Vec<usize>>()
                    + size_of::<std::ops::Range<usize>>()
                    + 3 * size_of::<usize>()
                    + definitions * size_of::<usize>()
                    + incidences * size_of::<ValueId>()
            );
            budget.reserve_storage(
                size_of::<CorrelationLedgerV18<'_, '_, '_>>()
                    + size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>()
                    + 2 * size_of::<InventoryCorrelationV18<'_, '_, '_, '_, '_, '_>>()
                    + size_of::<Gfx942InlineScalarCorrespondenceV30<'_>>()
                    + size_of::<AllocationWalkScratchV18<'_>>(),
            )?;
            {
                let ledger = CorrelationLedgerV18::new(budget, original.source.cleanup);
                let inline = Gfx942InlineScalarCorrespondenceV30::empty();
                let graph = InventoryCorrelationV18 {
                    origins: None,
                    inventory,
                    function: function.coordinate,
                    ledger: &ledger,
                    inline_scalar: &inline,
                    scalar_source: None,
                };
                let foreign = InventoryCorrelationV18 {
                    origins: None,
                    inventory: original.inventory,
                    function: function.coordinate,
                    ledger: &ledger,
                    inline_scalar: &inline,
                    scalar_source: None,
                };
                let mut charge = SourceCorrelationChargeV18 {
                    ledger: &ledger,
                    finite: UnsupportedIndexCorrelationBudgetV1 {
                        remaining: 1_000_000,
                    },
                    finite_denied: false,
                };
                let body = function.function.body.as_ref().unwrap();
                let mut tested = 0usize;
                for (parameter, value) in body.parameters.iter().enumerate() {
                    if !matches!(
                        function.function.signature.parameters[parameter],
                        Type::Pointer(_) | Type::Slice(_)
                    ) {
                        continue;
                    }
                    assert!(scratch.begin(*value, &foreign, &mut charge).is_none());
                    for _ in 0..2 {
                        scratch.begin(*value, &graph, &mut charge).unwrap();
                        assert_eq!(
                            external_allocation_parameter_core_v18(
                                function.function,
                                &graph,
                                &mut AllocationWalkScratchV18::Source(&mut scratch),
                                &mut charge
                            ),
                            Some(parameter as u32)
                        );
                    }
                    tested += 1;
                }
                assert!(
                    tested > 0,
                    "actual output root must have pointer-bearing arguments"
                );
                scratch
                    .begin(ValueId(u32::MAX), &graph, &mut charge)
                    .unwrap();
                assert!(
                    external_allocation_parameter_core_v18(
                        function.function,
                        &graph,
                        &mut AllocationWalkScratchV18::Source(&mut scratch),
                        &mut charge
                    )
                    .is_none()
                );
                assert_eq!(
                    scratch.marks.len(),
                    definitions,
                    "sparse hostile IDs cannot resize marks"
                );
                assert_eq!(scratch.pending.capacity(), incidences);
                assert!(ledger.failure.get().is_none());
            }
            drop(scratch);
            budget.release_storage(budget.storage() - floor)?;
            Ok(())
        },
    );
}

fn optimized_analysis_header_oracle_v18<T, E, F>(_: &F) -> usize {
    use std::{
        mem::{align_of, size_of},
        panic::AssertUnwindSafe,
    };
    type Payload = Box<dyn std::any::Any + Send>;
    type EntryCapture<'a, F> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        F,
    );
    type EntryCatch<'a, 'work, F> = (
        EntryCapture<'a, F>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
    );
    type Entry<'a, F> = (
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        usize,
        F,
    );
    type EntryResult<'a, F> = Result<Entry<'a, F>, ProductionSourceOwnedViewErrorV18>;
    type Invoke<'a, 'work, F> = (
        F,
        &'a mut ProductionOptimizedSourceAnalysisV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
    );
    let disposal = size_of::<[Option<Payload>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + size_of::<AssertUnwindSafe<Payload>>()
        + 2 * size_of::<Result<(), Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    2 * size_of::<EntryCapture<'_, F>>()
        + 2 * align_of::<EntryCapture<'_, F>>()
        + size_of::<EntryCatch<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<EntryCatch<'_, '_, F>>>()
        + size_of::<Entry<'_, F>>()
        + 2 * size_of::<EntryResult<'_, F>>()
        + size_of::<std::thread::Result<EntryResult<'_, F>>>()
        + size_of::<AssertUnwindSafe<Entry<'_, F>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<std::cell::Cell<usize>>()
        + 2 * size_of::<usize>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Invoke<'_, '_, F>>()
        + size_of::<AssertUnwindSafe<Invoke<'_, '_, F>>>()
        + size_of::<ProductionOptimizedSourceAnalysisV18<'_>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<AssertUnwindSafe<Result<T, E>>>()
        + disposal
}

include!("production_optimized_source_analysis_callback_v18_tests.rs");
include!("production_optimized_source_entry_callbacks_v18_tests.rs");

#[test]
fn retained_optimizer_transfer_is_one_atomic_exact_or_one_short_reservation() {
    const FLOOR: usize = 11;
    const CHECKED: usize = 137;
    const ORIGIN: usize = 29;
    for one_short in [false, true] {
        let limit = FLOOR + CHECKED + ORIGIN - usize::from(one_short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = optimized_source_consumer_resources_v18::reserve_optimizer_transfer(
            CHECKED,
            ORIGIN,
            &mut budget,
        );
        assert_eq!(budget.work(), 0, "an inert receipt join performs no work");
        if one_short {
            let Err(ArgumentResourceV1::Storage(error)) = result else {
                panic!("one-short transfer must refuse without partial credit");
            };
            assert_eq!(error.actual(), FLOOR + CHECKED + ORIGIN);
            assert_eq!(error.limit(), limit);
            assert_eq!(budget.failed_storage(), Some(error.actual()));
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), FLOOR + CHECKED + ORIGIN);
            assert_eq!(budget.peak_storage(), FLOOR + CHECKED + ORIGIN);
            assert_eq!(budget.failed_storage(), None);
        }
    }
}

#[test]
fn retained_optimizer_transfer_overflow_never_spends_or_forges_storage_denial() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, 7);
    budget.reserve_storage(7).unwrap();
    assert!(matches!(
        optimized_source_consumer_resources_v18::reserve_optimizer_transfer(
            usize::MAX,
            1,
            &mut budget,
        ),
        Err(ArgumentResourceV1::Arithmetic)
    ));
    assert_eq!(budget.storage(), 7);
    assert_eq!(budget.peak_storage(), 7);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn retained_optimizer_header_accounts_actual_aligned_closure_and_caught_result() {
    use std::{
        mem::{size_of, size_of_val},
        panic::AssertUnwindSafe,
    };
    type Payload = [u128; 3];
    type Error = ProductionSourceOwnedViewErrorV18;
    type Output = (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
        Payload,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    );
    type Result = std::result::Result<Output, ProductionSourceOptimizationErrorV18<Error>>;
    type Panic = Box<dyn std::any::Any + Send>;
    let captured = [7u128; 5];
    let closure = move || std::hint::black_box(captured);
    let cleanup = size_of::<[Option<Panic>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Panic>; 2]>>()
        + 2 * size_of::<Panic>()
        + size_of::<AssertUnwindSafe<Panic>>()
        + 2 * size_of::<std::result::Result<(), Panic>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    let expected = size_of_val(&closure)
        + size_of::<std::cell::Cell<usize>>()
        + size_of::<Output>()
        + size_of::<Result>()
        + size_of::<std::thread::Result<Result>>()
        + size_of::<AssertUnwindSafe<Output>>()
        + size_of::<ProductionSourceOptimizationErrorV18<Error>>()
        + cleanup;
    assert_eq!(
        optimized_source_consumer_resources_v18::optimizer_transfer_headers::<Payload, Error>(
            size_of_val(&closure),
        )
        .unwrap(),
        expected,
    );
    assert!(matches!(
        optimized_source_consumer_resources_v18::optimizer_transfer_headers::<Payload, Error>(
            usize::MAX
        ),
        Err(ArgumentResourceV1::Arithmetic),
    ));
}

#[test]
fn independent_analysis_header_oracle_matches_coexisting_source_and_output_scope() {
    run_production_optimized_consumer_v18(
        folding_source_owner_v18,
        |original, optimized, budget| {
            let floor = budget.storage();
            let expected = std::cell::Cell::new(0);
            let consume = |analyses: &mut ProductionOptimizedSourceAnalysisV18<'_>,
                           budget: &mut ArgumentBudgetV1<'_>| {
                assert_eq!(budget.storage() - floor, expected.get());
                assert_eq!(analyses.storage, expected.get());
                assert_eq!(analyses.floor, floor + expected.get());
                assert!(analyses.input_sparse.is_none() && analyses.sparse.is_none());
                assert!(analyses.input_effects.is_none() && analyses.output_effects.is_none());
                assert!(analyses.input_memory.is_none() && analyses.output_memory.is_none());
                assert!(!std::ptr::eq(analyses.input, analyses.output));
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            };
            expected.set(optimized_analysis_header_oracle_v18::<
                (),
                ProductionSourceOwnedViewErrorV18,
                _,
            >(&consume));
            original.with_optimized_analysis_v18(optimized, budget, consume)?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    );
}

#[test]
fn independent_analysis_header_minus_one_records_exact_first_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let recorded = std::cell::Cell::new(None);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted =
            source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                let consume = |_: &mut ProductionOptimizedSourceAnalysisV18<'_>,
                               _: &mut ArgumentBudgetV1<'_>|
                 -> SourceOwnedResultV18<()> {
                    panic!("header minus one admitted")
                };
                let expected = optimized_analysis_header_oracle_v18::<
                    (),
                    ProductionSourceOwnedViewErrorV18,
                    _,
                >(&consume);
                let filler = MODULE_LIMIT - budget.storage() - expected + 1;
                budget.reserve_storage(filler)?;
                let denied = original.with_optimized_analysis_v18(optimized, budget, consume);
                let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                    error,
                ))) = denied
                else {
                    panic!("expected exact analysis header storage refusal");
                };
                assert_eq!(error.actual(), MODULE_LIMIT + 1);
                assert_eq!(error.limit(), MODULE_LIMIT);
                recorded.set(Some(error));
                budget.release_storage(filler)?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
        assert!(attempted.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(recorded.get().is_some());
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(error)))
        if Some(error) == recorded.get())
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
#[test]
fn nominal_generated_iterator_preserves_the_old_enclosing_enum_layout() {
    #[allow(dead_code)]
    enum Before<'a> {
        Legacy {
            operations: &'a [Operation],
            block: BlockId,
            first: usize,
        },
        Source {
            rows: &'a [SourceAttachmentV18],
            inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            meter: &'a dyn NeutralRecipeMeterV18,
        },
    }
    assert_eq!(
        std::mem::size_of::<NeutralRecipeOperationsV18<'_>>(),
        std::mem::size_of::<Before<'_>>()
    );
    assert_eq!(
        std::mem::align_of::<NeutralRecipeOperationsV18<'_>>(),
        std::mem::align_of::<Before<'_>>()
    );
}
fn independent_source_span_query_work_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    target: [usize; 5],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let mut keys = Vec::new();
    for root in 0..original.source.root_count(budget)? {
        for row in &original.source.root_row(root)?.coordinates.spans.rows {
            let (block, statement) = match row.source {
                InstanceSpanSourceV1::Statement(site) => {
                    (site.semantic_block, Some(site.statement_ordinal))
                }
                InstanceSpanSourceV1::Terminator(site) => (site.semantic_block, None),
                InstanceSpanSourceV1::Synthetic(_) | InstanceSpanSourceV1::InvocationEntry(_) => {
                    continue;
                }
            };
            keys.push([
                root,
                row.instance.index(),
                block.index() as usize,
                usize::from(statement.is_some()),
                statement.unwrap_or(0) as usize,
            ]);
        }
    }
    keys.sort_unstable();
    assert!(keys.binary_search(&target).is_ok());
    // Two original custody queries, the terminal binary-search probe, seven
    // exact site/range checks, and one entry-count/index access.
    let mut work = 11;
    let (mut first, mut end) = (0, keys.len());
    while first < end {
        let middle = first + (end - first) / 2;
        let row = keys[middle];
        let comparisons = row
            .iter()
            .zip(target)
            .position(|(left, right)| *left != right)
            .map_or(5, |index| index + 1);
        let less = row < target;
        work += 6 + comparisons + usize::from(less);
        if less {
            first = middle + 1;
        } else {
            end = middle;
        }
    }
    Ok(work)
}

#[test]
fn indexed_source_span_query_has_independent_exact_and_one_short_work_oracles() {
    for one_short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let admitted = std::cell::Cell::new(None);
        let denied = std::cell::Cell::new(None);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let attempted =
                source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                    let expected =
                        independent_source_span_query_work_v18(original, [0, 0, 0, 0, 0], budget)?;
                    let block = SemanticBlockIdV1::from_index(0);
                    let before = budget.work();
                    let count = optimized.source_span_entry_count(0, 0, block, None, budget)?;
                    assert!(count > 0);
                    assert_eq!(budget.work() - before, expected);
                    let remaining = expected - usize::from(one_short);
                    let fill = OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - remaining;
                    budget.charge_work(fill)?;
                    let query = optimized.source_span_entry(0, 0, block, None, 0, budget);
                    admitted.set(Some(query.is_ok()));
                    if one_short {
                        let Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Work(error),
                        )) = query
                        else {
                            panic!("one-short exact source-span index query must refuse");
                        };
                        assert_eq!(error.actual(), OPTIMIZED_SOURCE_WORK_LIMIT_V18 + 1);
                        denied.set(Some(error));
                    }
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                });
            // Any later required work is still unpaid; the query oracle does
            // not claim that this remaining adoption suffix can also complete.
            if one_short {
                assert!(attempted.is_err());
            }
            drop(attempted);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert_eq!(admitted.get(), Some(!one_short));
        if one_short {
            assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))) if Some(error) == denied.get())
            );
        } else {
            assert!(matches!(
                result,
                Ok(())
                    | Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Work(_)
                    ))
            ));
        }
    }
}
#[test]
fn nested_optimizer_resource_payload_classification_preserves_exact_counts_without_meter_activity()
{
    use fe2o3_pliron::{
        KirBridgeErrorV18 as Bridge, KirNeutralOptimizationErrorV18 as Error,
        KirOptimizationMapErrorV12 as Mapping, PlironOptimizationErrorV12 as Execution,
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
    let mut budget = ArgumentBudgetV1::new(&mut work, 3);
    budget.reserve_storage(3).unwrap();
    let error = budget.reserve_storage(1).unwrap_err();
    let before = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    for wrapper in [
        Error::Resource(error),
        Error::Bridge(Bridge::Resource(error)),
        Error::Bridge(Bridge::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(error),
        )),
        Error::Execution(Execution::Resources(error)),
        Error::Execution(Execution::Mapping(Mapping::Resources(error))),
        Error::Mapping(Mapping::Resources(error)),
    ] {
        assert!(matches!(observed_optimizer_refusal_v18(&wrapper),
            SourceOwnedQueryFailureV18::Resource(actual) if actual == error));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            before
        );
    }
}
