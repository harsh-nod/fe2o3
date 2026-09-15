mod global_ranked_normalization_tests {
    use super::*;
    fn predecessor_normalize_ranked_expression_v1(
        expression: &ProductionSemanticExpressionV2,
        lowering: &ProductionRankedKernelLoweringInputV1,
        ranked: &RankedCorrelationIndexV1,
        depth: usize,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<NormalizedScalarExpressionV1> {
        budget.charge()?;
        if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            return None;
        }
        let next = depth.checked_add(1)?;
        Some(match expression {
            ProductionSemanticExpressionV2::Symbol { symbol, scalar } => {
                NormalizedScalarExpressionV1::Symbol {
                    symbol: *symbol,
                    scalar: *scalar,
                }
            }
            ProductionSemanticExpressionV2::Constant { scalar, bits } => {
                NormalizedScalarExpressionV1::Constant {
                    scalar: *scalar,
                    bits: *bits,
                }
            }
            ProductionSemanticExpressionV2::Load(load) => {
                let site = *ranked
                    .sites_by_ranked_location
                    .get(&(load.block, load.operation))?;
                let source = ranked.sources_by_site.get(&site)?;
                if source.access != dialect_kernel::AccessKindAttr::Read {
                    return None;
                }
                let IndexedRankedAllocationV1::View(source_view) = source.allocation else {
                    return None;
                };
                if source_view != load.view {
                    return None;
                }
                let ProductionRankedValueV1::Local(view) = source_view else {
                    return None;
                };
                let definition = ranked.view_definitions.get(&view)?;
                if definition.memory_space != dialect_kernel::MemorySpaceAttr::Global
                    || definition.allocation_origin != load.allocation_origin
                {
                    return None;
                }
                let operation = lowering
                    .kernel()
                    .blocks()
                    .get(load.block as usize)?
                    .operations()
                    .get(load.operation as usize)?;
                let indices = match operation {
                    ProductionRankedOperationV1::Access { indices, .. }
                    | ProductionRankedOperationV1::ValueAccess { indices, .. }
                    | ProductionRankedOperationV1::AtomicAccess { indices, .. }
                    | ProductionRankedOperationV1::AtomicValueAccess { indices, .. } => indices,
                    _ => return None,
                };
                if indices.as_slice() != load.indices.as_ref() {
                    return None;
                }
                NormalizedScalarExpressionV1::Load {
                    site,
                    scalar: load.scalar,
                }
            }
            ProductionSemanticExpressionV2::Unary {
                operation,
                scalar,
                operand,
            } => NormalizedScalarExpressionV1::Unary {
                operation: *operation,
                scalar: *scalar,
                operand: Box::new(predecessor_normalize_ranked_expression_v1(
                    operand, lowering, ranked, next, budget,
                )?),
            },
            ProductionSemanticExpressionV2::Binary {
                operation,
                scalar,
                overflow,
                lhs,
                rhs,
            } => NormalizedScalarExpressionV1::Binary {
                operation: *operation,
                scalar: *scalar,
                overflow: *overflow,
                lhs: Box::new(predecessor_normalize_ranked_expression_v1(
                    lhs, lowering, ranked, next, budget,
                )?),
                rhs: Box::new(predecessor_normalize_ranked_expression_v1(
                    rhs, lowering, ranked, next, budget,
                )?),
            },
            ProductionSemanticExpressionV2::Compare {
                operation,
                operand_scalar,
                lhs,
                rhs,
            } => NormalizedScalarExpressionV1::Compare {
                operation: *operation,
                operand_scalar: *operand_scalar,
                lhs: Box::new(predecessor_normalize_ranked_expression_v1(
                    lhs, lowering, ranked, next, budget,
                )?),
                rhs: Box::new(predecessor_normalize_ranked_expression_v1(
                    rhs, lowering, ranked, next, budget,
                )?),
            },
            ProductionSemanticExpressionV2::Select {
                scalar,
                condition,
                when_true,
                when_false,
            } => NormalizedScalarExpressionV1::Select {
                scalar: *scalar,
                condition: Box::new(predecessor_normalize_ranked_expression_v1(
                    condition, lowering, ranked, next, budget,
                )?),
                when_true: Box::new(predecessor_normalize_ranked_expression_v1(
                    when_true, lowering, ranked, next, budget,
                )?),
                when_false: Box::new(predecessor_normalize_ranked_expression_v1(
                    when_false, lowering, ranked, next, budget,
                )?),
            },
            ProductionSemanticExpressionV2::Cast {
                kind,
                source,
                target,
                operand,
            } => {
                let operand = predecessor_normalize_ranked_expression_v1(
                    operand, lowering, ranked, next, budget,
                )?;
                if source == target {
                    operand
                } else {
                    NormalizedScalarExpressionV1::Cast {
                        kind: *kind,
                        source: *source,
                        target: *target,
                        operand: Box::new(operand),
                    }
                }
            }
        })
    }

    fn predecessor_normalize_kir_expression_v1(
        function: &Function,
        kir: &KirCorrelationIndexV1<'_>,
        semantic_sites: &BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
        value: ValueId,
        depth: usize,
        visiting: &mut BTreeSet<ValueId>,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<NormalizedScalarExpressionV1> {
        budget.charge()?;
        if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 || !visiting.insert(value) {
            return None;
        }
        let result = predecessor_normalize_kir_expression_inner_v1(
            function,
            kir,
            semantic_sites,
            value,
            depth,
            visiting,
            budget,
        );
        visiting.remove(&value);
        result
    }

    fn predecessor_normalize_kir_expression_inner_v1(
        function: &Function,
        kir: &KirCorrelationIndexV1<'_>,
        semantic_sites: &BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
        value: ValueId,
        depth: usize,
        visiting: &mut BTreeSet<ValueId>,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<NormalizedScalarExpressionV1> {
        let value = predecessor_unique_kir_ssa_origin_v1(kir, value, budget)?;
        let body = function.body.as_ref()?;
        if let Some(parameter) = body
            .parameters
            .iter()
            .position(|candidate| *candidate == value)
        {
            let scalar = kir_semantic_scalar_v1(function.signature.parameters.get(parameter)?)?;
            let argument = u32::try_from(parameter).ok()?;
            let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(argument)?;
            return Some(NormalizedScalarExpressionV1::Symbol { symbol, scalar });
        }
        let operation = kir.definitions.get(&value)?;
        let scalar = operation
            .results
            .iter()
            .find(|result| result.id == value)
            .and_then(|result| kir_semantic_scalar_v1(&result.ty))?;
        let next = depth.checked_add(1)?;
        let recurse = |operand,
                       visiting: &mut BTreeSet<ValueId>,
                       budget: &mut UnsupportedIndexCorrelationBudgetV1| {
            predecessor_normalize_kir_expression_v1(
                function,
                kir,
                semantic_sites,
                operand,
                next,
                visiting,
                budget,
            )
        };
        Some(match &operation.kind {
            OperationKind::Constant(constant) => {
                let (constant_scalar, bits) = normalize_kir_constant_v1(constant)?;
                if constant_scalar != scalar {
                    return None;
                }
                NormalizedScalarExpressionV1::Constant { scalar, bits }
            }
            OperationKind::Unary { op, operand } => NormalizedScalarExpressionV1::Unary {
                operation: match op {
                    UnaryOp::Not => ProductionSemanticUnaryOpV2::Not,
                    UnaryOp::Negate => ProductionSemanticUnaryOpV2::Negate,
                },
                scalar,
                operand: Box::new(recurse(*operand, visiting, budget)?),
            },
            OperationKind::Binary { op, lhs, rhs } => {
                let (operation, overflow) = normalize_kir_binary_v1(*op, operation, value)?;
                NormalizedScalarExpressionV1::Binary {
                    operation,
                    scalar,
                    overflow,
                    lhs: Box::new(recurse(*lhs, visiting, budget)?),
                    rhs: Box::new(recurse(*rhs, visiting, budget)?),
                }
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => {
                let lhs_scalar = kir_value_scalar_v1(function, kir, *lhs)?;
                NormalizedScalarExpressionV1::Compare {
                    operation: normalize_kir_comparison_v1(*predicate),
                    operand_scalar: lhs_scalar,
                    lhs: Box::new(recurse(*lhs, visiting, budget)?),
                    rhs: Box::new(recurse(*rhs, visiting, budget)?),
                }
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => NormalizedScalarExpressionV1::Select {
                scalar,
                condition: Box::new(recurse(*condition, visiting, budget)?),
                when_true: Box::new(recurse(*true_value, visiting, budget)?),
                when_false: Box::new(recurse(*false_value, visiting, budget)?),
            },
            OperationKind::Cast { kind, value, to } => {
                let source = kir_value_scalar_v1(function, kir, *value)?;
                let target = kir_semantic_scalar_v1(to)?;
                let operand = recurse(*value, visiting, budget)?;
                if source == target {
                    operand
                } else {
                    NormalizedScalarExpressionV1::Cast {
                        kind: normalize_kir_cast_v1(*kind, source, target)?,
                        source,
                        target,
                        operand: Box::new(operand),
                    }
                }
            }
            OperationKind::Load { .. } => {
                let location = *kir.definition_locations.get(&value)?;
                let site = *semantic_sites.get(&(location, 0))?;
                NormalizedScalarExpressionV1::Load { site, scalar }
            }
            _ => return None,
        })
    }

    fn predecessor_unique_kir_ssa_origin_v1(
        kir: &KirCorrelationIndexV1<'_>,
        value: ValueId,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<ValueId> {
        let mut pending = vec![value];
        let mut visited = BTreeSet::new();
        let mut origins = BTreeSet::new();
        while let Some(value) = pending.pop() {
            budget.charge()?;
            if !visited.insert(value) {
                continue;
            }
            if let Some(inputs) = kir.block_parameter_inputs.get(&value) {
                if inputs.is_empty() {
                    return None;
                }
                pending.extend(inputs.iter().copied());
            } else {
                origins.insert(value);
                if origins.len() > 1 {
                    return None;
                }
            }
        }
        let mut origins = origins.into_iter();
        let origin = origins.next()?;
        origins.next().is_none().then_some(origin)
    }

    #[test]
    fn legacy_scalar_adapters_preserve_every_result_and_work_refusal_prefix() {
        for operation in [
            ProductionSemanticBinaryOpV2::Add,
            ProductionSemanticBinaryOpV2::Multiply,
        ] {
            let mut fixture = value_translation_fixture(operation, 0x3f80_0000);
            fixture.module.functions[0]
                .signature
                .parameters
                .push(Type::Scalar(ScalarType::F32));
            fixture.module.functions[0]
                .body
                .as_mut()
                .unwrap()
                .parameters
                .push(ValueId(20));
            fixture.module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .extend([
                    Operation::effect_free(
                        ValueDef::new(ValueId(21), Type::Scalar(ScalarType::F32)),
                        OperationKind::Unary {
                            op: UnaryOp::Negate,
                            operand: ValueId(7),
                        },
                    ),
                    Operation::effect_free(
                        ValueDef::new(ValueId(22), Type::BOOL),
                        OperationKind::Compare {
                            predicate: ComparePredicate::LessThan,
                            lhs: ValueId(5),
                            rhs: ValueId(20),
                        },
                    ),
                    Operation::effect_free(
                        ValueDef::new(ValueId(23), Type::Scalar(ScalarType::F32)),
                        OperationKind::Select {
                            condition: ValueId(22),
                            true_value: ValueId(21),
                            false_value: ValueId(20),
                        },
                    ),
                    Operation::effect_free(
                        ValueDef::new(ValueId(24), Type::Scalar(ScalarType::F32)),
                        OperationKind::Cast {
                            kind: CastKind::Bitcast,
                            value: ValueId(23),
                            to: Type::Scalar(ScalarType::F32),
                        },
                    ),
                ]);
            // Ten shared SSA definitions denote 2,047 expanded scalar visits.
            // Both adapters must stop at the same legacy work-refusal prefix.
            let mut repeated = ValueId(20);
            for value in 30..40 {
                fixture.module.functions[0].body.as_mut().unwrap().blocks[0]
                    .operations
                    .push(Operation::effect_free(
                        ValueDef::new(ValueId(value), Type::Scalar(ScalarType::F32)),
                        OperationKind::Binary {
                            op: BinaryOp::Add,
                            lhs: repeated,
                            rhs: repeated,
                        },
                    ));
                repeated = ValueId(value);
            }
            let function = &fixture.module.functions[0];
            let mut indexing = UnsupportedIndexCorrelationBudgetV1 { remaining: 10_000 };
            let kir =
                build_kir_correlation_index(function.body.as_ref().unwrap(), 100, &mut indexing)
                    .unwrap();
            let sites = index_semantic_access_sites(
                &fixture.correspondence,
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
                &kir,
                &mut indexing,
            )
            .unwrap();
            let ranked =
                index_ranked_correlation(&fixture.lowering, &fixture.sources, 100, &mut indexing)
                    .unwrap();
            for value in [
                ValueId(5),
                ValueId(6),
                ValueId(7),
                ValueId(20),
                ValueId(21),
                ValueId(22),
                ValueId(23),
                ValueId(24),
                ValueId(39),
                ValueId(999),
            ] {
                for limit in 0..=128 {
                    for depth in [0, MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1] {
                        for seed in [None, Some(ValueId(7))] {
                            let mut old_visiting = seed.into_iter().collect();
                            let mut new_visiting = seed.into_iter().collect();
                            let mut old = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                            let mut new = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                            let expected = predecessor_normalize_kir_expression_v1(
                                function,
                                &kir,
                                &sites,
                                value,
                                depth,
                                &mut old_visiting,
                                &mut old,
                            );
                            let actual = normalize_kir_expression_v1(
                                function,
                                &kir,
                                &sites,
                                value,
                                depth,
                                &mut new_visiting,
                                &mut new,
                            );
                            assert_eq!(
                                (actual, new.remaining, new_visiting),
                                (expected, old.remaining, old_visiting),
                                "value={value:?}, limit={limit}, depth={depth}"
                            );
                        }
                    }
                }
            }
            let expression = &ranked.semantic_expressions.values().next().unwrap().0;
            let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
            let expressions = [
                expression.clone(),
                ProductionSemanticExpressionV2::Symbol {
                    symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 + 1,
                    scalar,
                },
                ProductionSemanticExpressionV2::Cast {
                    kind: ProductionSemanticCastV2::FloatToFloat,
                    source: scalar,
                    target: scalar,
                    operand: Box::new(expression.clone()),
                },
                ProductionSemanticExpressionV2::Unary {
                    operation: ProductionSemanticUnaryOpV2::Negate,
                    scalar,
                    operand: Box::new(expression.clone()),
                },
            ];
            for expression in &expressions {
                for limit in 0..=128 {
                    for depth in [0, MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1] {
                        let mut old = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                        let mut new = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                        let expected = predecessor_normalize_ranked_expression_v1(
                            expression,
                            &fixture.lowering,
                            &ranked,
                            depth,
                            &mut old,
                        );
                        let actual = normalize_ranked_expression_v1(
                            expression,
                            &fixture.lowering,
                            &ranked,
                            depth,
                            &mut new,
                        );
                        assert_eq!(
                            (actual, new.remaining),
                            (expected, old.remaining),
                            "ranked limit={limit}, depth={depth}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn legacy_scalar_ssa_adapter_preserves_phi_cycles_ambiguity_and_refusals() {
        let fixture = value_translation_fixture(ProductionSemanticBinaryOpV2::Add, 0x3f80_0000);
        let mut indexing = UnsupportedIndexCorrelationBudgetV1 { remaining: 10_000 };
        let mut kir = build_kir_correlation_index(
            fixture.module.functions[0].body.as_ref().unwrap(),
            100,
            &mut indexing,
        )
        .unwrap();
        kir.block_parameter_inputs.extend([
            (ValueId(10), vec![ValueId(11), ValueId(5)]),
            (ValueId(11), vec![ValueId(10)]),
            (ValueId(12), vec![ValueId(5), ValueId(6)]),
            (ValueId(13), vec![]),
            (ValueId(14), vec![ValueId(14)]),
        ]);
        for value in [5, 10, 11, 12, 13, 14, 999] {
            for limit in 0..=16 {
                let mut old = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                let mut new = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                let expected = predecessor_unique_kir_ssa_origin_v1(&kir, ValueId(value), &mut old);
                let actual = unique_kir_ssa_origin_v1(&kir, ValueId(value), &mut new);
                assert_eq!((actual, new.remaining), (expected, old.remaining));
            }
        }
    }

    fn paid_global_vector_boundary<T: Clone>(value: T) {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };
        let record = std::mem::size_of::<T>();
        let header = std::mem::size_of::<Vec<T>>();
        let floor = 23;
        // Initial reserve/push costs 2; the next three pushes cost 3; growing
        // 4 -> 8 records costs 5 relocations/reserve + 1 final push. Both buffers
        // coexist at the peak: 4 + 8 records, with one prepaid vector header.
        for (work_headroom, storage_headroom, succeeds) in [
            (11, header + 12 * record, true),
            (10, header + 12 * record, false),
            (11, header + 12 * record - 1, false),
        ] {
            let mut work = Work::new(7 + work_headroom);
            let mut budget = Budget::new(&mut work, floor + storage_headroom);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let outcome = (|| {
                budget.reserve_storage(header)?;
                let mut values = Vec::new();
                for _ in 0..5 {
                    assert_origin_push_v1(&mut values, value.clone(), &mut budget)?;
                }
                assert_eq!(values.len(), 5);
                Ok::<(), SemanticKirAssertOriginErrorV1>(())
            })();
            assert_eq!(outcome.is_ok(), succeeds);
            let retained = budget.storage() - floor;
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
            if succeeds {
                assert_eq!(budget.work(), 18);
                assert_eq!(budget.peak_storage(), floor + header + 12 * record);
            } else if work_headroom == 10 {
                assert_eq!(budget.work(), 17);
                assert_eq!(work.failed_work(), Some(18));
            } else {
                assert_eq!(budget.failed_storage(), Some(floor + header + 12 * record));
            }
        }
    }

    #[test]
    fn global_ranked_scratch_record_types_have_exact_growth_and_failure_cleanup() {
        let site = SemanticAccessSiteV1 {
            block: 0,
            statement: Some(0),
            ordinal: 0,
        };
        let value = ProductionRankedValueIdV1::new(0);
        let definition = RankedViewDefinitionV1 {
            allocation_origin: 1,
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            noalias_class: 0,
        };
        let source = IndexedRankedAccessSourceV1 {
            ranked_block: 0,
            ranked_operation: 1,
            access: dialect_kernel::AccessKindAttr::Read,
            allocation: IndexedRankedAllocationV1::View(ProductionRankedValueV1::Local(value)),
            value: None,
            atomic: None,
        };
        let original = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
                block: 0,
            },
            operation: 0,
        };
        let expression = ProductionSemanticExpressionV2::Constant {
            scalar: ProductionSemanticScalarTypeV2::Bool,
            bits: 1,
        };
        paid_global_vector_boundary::<GlobalRankedSiteRowV1>((site, source));
        paid_global_vector_boundary::<GlobalRankedLocationRowV1>(((0, 1), site));
        paid_global_vector_boundary::<GlobalOriginalSiteRowV1>((original, site));
        paid_global_vector_boundary((value, definition));
        paid_global_vector_boundary::<GlobalRankedExpressionRowV1<'_>>((
            value,
            &expression,
            ProductionNumericalContractV2::exact_for_expression(&expression),
        ));
        paid_global_vector_boundary(NormalizedScalarNodeV1::<usize>::Constant {
            scalar: ProductionSemanticScalarTypeV2::Bool,
            bits: 1,
        });
    }

    #[test]
    fn canonical_scalar_arena_has_exact_work_storage_and_latched_failure_boundaries() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Owner,
        };
        let mut fixture = value_translation_fixture(ProductionSemanticBinaryOpV2::Add, 0x3f80_0000);
        let mut repeated = ValueId(6);
        for value in 30..40 {
            fixture.module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::effect_free(
                    ValueDef::new(ValueId(value), Type::Scalar(ScalarType::F32)),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: repeated,
                        rhs: repeated,
                    },
                ));
            repeated = ValueId(value);
        }
        let mut setup_work = Work::new(1_000_000);
        let mut setup = Budget::new(&mut setup_work, 1_000_000);
        let (owner, owner_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&fixture.module, &mut setup)
                .unwrap();
        setup
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (inventory, inventory_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&owner, &mut setup).unwrap();
        setup
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let expression = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 + 2,
                scalar,
            }),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 7 }),
        };
        // Two append-only three-node trees: 12 visits + 6 pushes + initial
        // reserve 1 + 4->8 growth 5 = 24. Paired traversal: initial 2,
        // three (shape 9 + child slots 3), and two child-pair pushes 2 = 42.
        // At growth, old 4 and new 8 node buffers coexist.
        let bytes = 12 * std::mem::size_of::<NormalizedScalarNodeV1<usize>>();
        for (headroom, available, success) in [
            (66, bytes, true),
            (65, bytes, false),
            (66, bytes - 1, false),
        ] {
            let ssa = source_output_allocation_scratch_v1(&inventory, &mut setup).unwrap();
            let scratch_storage = ssa.storage;
            let floor = setup.storage()
                + std::mem::size_of::<SourceOutputScalarNormalizationV1<'_, '_, '_, '_, '_>>()
                - std::mem::size_of::<SourceOutputAllocationScratchV1>();
            let mut work = Work::new(7 + headroom);
            let mut budget = Budget::new(&mut work, floor + available);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let mut context = SourceOutputScalarNormalizationV1 {
                inventory: &inventory,
                function: &inventory.functions()[0],
                lowering: &fixture.lowering,
                sources: Vec::new(),
                locations: Vec::new(),
                sites: Vec::new(),
                views: Vec::new(),
                expressions: Vec::new(),
                nodes: Vec::new(),
                comparison: [(0, 0); 2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting_len: 0,
                normalization_steps: 0,
                ssa,
                error: None,
                budget: &mut budget,
            };
            let result = (|| {
                let left = normalize_ranked_expression_core_v1(&expression, 0, &mut context)?;
                let right = normalize_ranked_expression_core_v1(&expression, 0, &mut context)?;
                context.equivalent(left, right)
            })();
            assert_eq!(result.is_some(), success);
            if success {
                assert_eq!(result, Some(true));
                assert_eq!(context.nodes.len(), 6);
            } else {
                assert!(context.error.is_some());
            }
            drop(context);
            let retained = budget.storage() - floor;
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
            if success {
                assert_eq!(budget.work(), 73);
                assert_eq!(budget.peak_storage(), floor + bytes);
            } else if headroom == 65 {
                assert_eq!(budget.work(), 72);
            } else {
                assert_eq!(budget.failed_storage(), Some(floor + bytes));
            }
            if headroom == 65 {
                assert_eq!(work.failed_work(), Some(73));
            }
            setup.release_storage(scratch_storage).unwrap();
        }
        let shape_floor = setup.storage();
        source_output_global_scratch_scope_v1(&mut setup, |budget| {
            budget
                .reserve_storage(
                    std::mem::size_of::<SourceOutputScalarNormalizationV1<'_, '_, '_, '_, '_>>()
                        - std::mem::size_of::<SourceOutputAllocationScratchV1>(),
                )
                .unwrap();
            let ssa = source_output_allocation_scratch_v1(&inventory, budget)?;
            let mut context = SourceOutputScalarNormalizationV1 {
                inventory: &inventory,
                function: &inventory.functions()[0],
                lowering: &fixture.lowering,
                sources: Vec::new(),
                locations: Vec::new(),
                sites: Vec::new(),
                views: Vec::new(),
                expressions: Vec::new(),
                nodes: Vec::new(),
                comparison: [(0, 0); 2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting_len: 0,
                normalization_steps: 0,
                ssa,
                error: None,
                budget,
            };
            let mut triples = [[0; 3]; 2];
            let mut selects = [0; 2];
            for pair in 0..2 {
                triples[pair][0] = context
                    .emit(NormalizedScalarNodeV1::Constant {
                        scalar: ProductionSemanticScalarTypeV2::Bool,
                        bits: 1,
                    })
                    .unwrap();
                triples[pair][1] = context
                    .emit(NormalizedScalarNodeV1::Constant { scalar, bits: 7 })
                    .unwrap();
                triples[pair][2] = context
                    .emit(NormalizedScalarNodeV1::Constant { scalar, bits: 8 })
                    .unwrap();
                selects[pair] = context
                    .emit(NormalizedScalarNodeV1::Select {
                        scalar,
                        condition: triples[pair][0],
                        when_true: triples[pair][1],
                        when_false: triples[pair][2],
                    })
                    .unwrap();
            }
            assert_ne!(selects[0], selects[1]);
            assert_eq!(context.equivalent(selects[0], selects[1]), Some(true));
            let reversed = context
                .emit(NormalizedScalarNodeV1::Select {
                    scalar,
                    condition: triples[1][0],
                    when_true: triples[1][2],
                    when_false: triples[1][1],
                })
                .unwrap();
            assert_eq!(context.equivalent(selects[0], reversed), Some(false));
            let target = ProductionSemanticScalarTypeV2::Float { bits: 32 };
            let cast = context
                .emit(NormalizedScalarNodeV1::Cast {
                    kind: ProductionSemanticCastV2::IntegerToFloat,
                    source: scalar,
                    target,
                    operand: triples[0][1],
                })
                .unwrap();
            for (kind, source, target, equal) in [
                (
                    ProductionSemanticCastV2::IntegerToFloat,
                    scalar,
                    target,
                    true,
                ),
                (
                    ProductionSemanticCastV2::FloatToFloat,
                    scalar,
                    target,
                    false,
                ),
                (
                    ProductionSemanticCastV2::IntegerToFloat,
                    ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 64,
                    },
                    target,
                    false,
                ),
                (
                    ProductionSemanticCastV2::IntegerToFloat,
                    scalar,
                    ProductionSemanticScalarTypeV2::Float { bits: 64 },
                    false,
                ),
            ] {
                let other = context
                    .emit(NormalizedScalarNodeV1::Cast {
                        kind,
                        source,
                        target,
                        operand: triples[1][1],
                    })
                    .unwrap();
                assert_eq!(context.equivalent(cast, other), Some(equal));
            }
            let mut repeated = [0; 2];
            for pair in 0..2 {
                repeated[pair] = context
                    .emit(NormalizedScalarNodeV1::Binary {
                        operation: ProductionSemanticBinaryOpV2::Add,
                        scalar,
                        overflow: ProductionOverflowContractV2::Wrapping,
                        lhs: selects[pair],
                        rhs: selects[pair],
                    })
                    .unwrap();
            }
            assert_eq!(context.equivalent(repeated[0], repeated[1]), Some(true));
            Ok(())
        })
        .unwrap();
        assert_eq!(setup.storage(), shape_floor);
        // Repeated KIR definitions retain the old expanded-tree semantics, but
        // every visit and arena growth still consumes the same live query ledger.
        let ssa = source_output_allocation_scratch_v1(&inventory, &mut setup).unwrap();
        let scratch_storage = ssa.storage;
        let floor = setup.storage()
            + std::mem::size_of::<SourceOutputScalarNormalizationV1<'_, '_, '_, '_, '_>>()
            - std::mem::size_of::<SourceOutputAllocationScratchV1>();
        let mut work = Work::new(1_007);
        let mut budget = Budget::new(&mut work, floor + 1_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        source_output_global_scratch_scope_v1(&mut budget, |budget| {
            let mut context = SourceOutputScalarNormalizationV1 {
                inventory: &inventory,
                function: &inventory.functions()[0],
                lowering: &fixture.lowering,
                sources: Vec::new(),
                locations: Vec::new(),
                sites: Vec::new(),
                views: Vec::new(),
                expressions: Vec::new(),
                nodes: Vec::new(),
                comparison: [(0, 0); 2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting_len: 0,
                normalization_steps: 0,
                ssa,
                error: None,
                budget,
            };
            assert!(normalize_kir_expression_core_v1(ValueId(39), 0, &mut context).is_none());
            assert_eq!(context.visiting_len, 0);
            assert!(context.normalization_steps > 10 && context.normalization_steps < 2_047);
            assert!(context.nodes.len() <= context.normalization_steps);
            Err::<(), _>(context.failure())
        })
        .unwrap_err();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() <= 1_007);
        assert!(work.failed_work().is_some_and(|failed| failed > 1_007));
        setup.release_storage(scratch_storage).unwrap();
        drop(inventory);
        setup
            .release_storage(inventory_storage.retained_storage())
            .unwrap();
        drop(owner);
        setup
            .release_storage(owner_storage.retained_storage())
            .unwrap();
        assert_eq!(setup.storage(), 0);
    }

    #[test]
    fn global_scratch_scope_restores_floor_after_partial_allocation_error_and_unwind() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };
        for panic in [false, true] {
            let mut work = Work::new(1_000);
            let mut budget = Budget::new(&mut work, 1_000);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(23).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    budget
                        .reserve_storage(std::mem::size_of::<Vec<usize>>())
                        .map_err(ProductionSourceOutputErrorV1::Resource)?;
                    let mut values = Vec::new();
                    assert_origin_push_v1(&mut values, 5usize, budget)
                        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                    assert_eq!(values.as_slice(), [5]);
                    assert!(!panic, "injected scope unwind");
                    Err::<(), _>(ProductionSourceOutputErrorV1::Invalid(
                        "injected scope refusal",
                    ))
                })
            }));
            assert_eq!(result.is_err(), panic);
            if let Ok(result) = result {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "injected scope refusal"
                    ))
                ));
            }
            assert_eq!(budget.storage(), 23);
            assert_eq!(budget.work(), 9);
            assert_eq!(
                budget.peak_storage(),
                23 + std::mem::size_of::<Vec<usize>>() + 4 * std::mem::size_of::<usize>()
            );
            source_output_global_scratch_scope_v1(&mut budget, |budget| {
                budget
                    .charge_work(1)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                budget
                    .reserve_storage(3)
                    .map_err(ProductionSourceOutputErrorV1::Resource)
            })
            .unwrap();
            assert_eq!(budget.storage(), 23);
            assert_eq!(budget.work(), 10);
        }
    }

    #[test]
    fn global_allocation_capacity_excess_is_prepaid_before_publication() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };
        let requested = std::mem::size_of::<SourceOutputAllocationScratchV1>()
            + std::mem::size_of::<ValueId>()
            + 3 * std::mem::size_of::<usize>();
        let actual = std::mem::size_of::<SourceOutputAllocationScratchV1>()
            + 2 * std::mem::size_of::<ValueId>()
            + 6 * std::mem::size_of::<usize>();
        for headroom in [actual, actual - 1] {
            let mut work = Work::new(1_000);
            let mut budget = Budget::new(&mut work, 23 + headroom);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(23).unwrap();
            let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                budget
                    .reserve_storage(requested)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                let mut storage = requested;
                for bytes in [
                    std::mem::size_of::<ValueId>(),
                    std::mem::size_of::<usize>(),
                    std::mem::size_of::<usize>(),
                    std::mem::size_of::<usize>(),
                ] {
                    budget
                        .charge_work(7)
                        .map_err(ProductionSourceOutputErrorV1::Resource)?;
                    storage =
                        source_output_allocation_capacity_storage_v1(1, 2, bytes, storage, budget)?;
                }
                Ok(storage)
            });
            assert_eq!(result.is_ok(), headroom == actual);
            if let Ok(storage) = result {
                assert_eq!(storage, actual);
            }
            assert_eq!(budget.storage(), 23);
            assert_eq!(budget.work(), 35);
            if headroom < actual {
                assert_eq!(budget.failed_storage(), Some(23 + actual));
            }
        }
    }

    #[test]
    fn global_allocation_capacity_excess_is_recorded_before_a_later_work_failure() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };
        let header = std::mem::size_of::<Vec<usize>>();
        let word = std::mem::size_of::<usize>();
        let mut work = Work::new(14);
        let mut budget = Budget::new(&mut work, 1_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(23).unwrap();
        let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
            let requested = header + word;
            budget
                .reserve_storage(requested)
                .map_err(ProductionSourceOutputErrorV1::Resource)?;
            budget
                .charge_work(7)
                .map_err(ProductionSourceOutputErrorV1::Resource)?;
            let mut values = Vec::<usize>::new();
            // Model an allocator returning two slots for the one-slot request.
            values.try_reserve_exact(2).unwrap();
            let actual = source_output_allocation_capacity_storage_v1(
                1,
                values.capacity(),
                word,
                requested,
                budget,
            )?;
            assert_eq!(budget.storage(), 23 + actual);
            budget
                .charge_work(7)
                .map_err(ProductionSourceOutputErrorV1::Resource)
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), 23);
        assert!(budget.peak_storage() >= 23 + header + 2 * word);
        assert_eq!(budget.work(), 14);
        assert_eq!(work.failed_work(), Some(21));
    }

    #[test]
    fn global_allocation_constructor_work_includes_each_prepaid_capacity_reconciliation() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Owner,
        };
        let fixture = value_translation_fixture(ProductionSemanticBinaryOpV2::Add, 0x3f80_0000);
        let mut setup_work = Work::new(1_000_000);
        let mut setup = Budget::new(&mut setup_work, 1_000_000);
        let (owner, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&fixture.module, &mut setup)
                .unwrap();
        setup.reserve_storage(receipt.retained_storage()).unwrap();
        let (inventory, inventory_receipt) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&owner, &mut setup).unwrap();
        setup
            .reserve_storage(inventory_receipt.retained_storage())
            .unwrap();
        assert_eq!(inventory.definitions().len(), 8);
        assert!(inventory.edge_arguments().is_empty());
        // Sizing12 + four allocate/reconcile7 + initialize16 = 56. The
        // one-under limit fails before the entire final initialization charge.
        for available in [56, 55] {
            let mut work = Work::new(7 + available);
            let floor = setup.storage();
            let mut budget = Budget::new(&mut work, floor + 1_000_000);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                let scratch = source_output_allocation_scratch_v1(&inventory, budget)?;
                let actual = std::mem::size_of::<SourceOutputAllocationScratchV1>()
                    + scratch.pending.capacity() * std::mem::size_of::<ValueId>()
                    + (scratch.visited.capacity()
                        + scratch.incoming.capacity()
                        + scratch.next.capacity())
                        * std::mem::size_of::<usize>();
                assert_eq!(scratch.storage, actual);
                Ok(())
            });
            assert_eq!(result.is_ok(), available == 56);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), if available == 56 { 63 } else { 47 });
            assert_eq!(work.failed_work(), (available == 55).then_some(63));
        }
        drop(inventory);
        setup
            .release_storage(inventory_receipt.retained_storage())
            .unwrap();
        drop(owner);
        setup.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(setup.storage(), 0);
    }
}
