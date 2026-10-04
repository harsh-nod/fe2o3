use super::*;

fn rebuild_owner(
    source: &ProductionSemanticSsaOwnerV1,
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let semantic = source.source_semantic();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
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
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn retained_array_return_rejects_stale_ssa_after_projected_write() {
    for length in [2_u64, 3] {
        let seed = argument_result_owner(false, ArgumentHelperResultShape::Array(length));
        let semantic = seed.source_semantic();
        let helper = &semantic.functions()[0];
        let source = helper.source();
        let mut statements = helper.blocks()[0].statements().to_vec();
        let destination = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: length,
                        from_end: false,
                    },
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap();
        let value = SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(
                fe2o3_mir_model::semantic_mir_v1::SemanticScalarValueV1::new(99, 4).unwrap(),
            ),
        ));
        statements.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(U32, SemanticRvalueKindV1::Use(value)),
            )),
        ));
        let block = SemanticBasicBlockV1::new(
            helper.blocks()[0].identity(),
            source,
            statements,
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap();
        let mut functions = semantic.functions().to_vec();
        functions[0] = SemanticFunctionDeclV1::new(
            helper.identity(),
            helper.role(),
            helper.item_definition_identity(),
            helper.monomorphization_identity(),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            source,
            helper.abi().clone(),
            helper.locals().to_vec(),
            helper.entry(),
            vec![block],
        )
        .unwrap();
        let owner = rebuild_owner(&seed, semantic.types().to_vec(), functions);
        let plan = owner
            .plan_for_function(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        assert!(
            plan.plan()
                .promoted_variables()
                .iter()
                .all(|local| local.get() != 0)
        );
        let error =
            lower_argument_owner(&owner, ProductionSemanticKirLimitsV1::default()).unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: Some(0),
                    statement: None,
                    detail: "aggregate helper return requires a whole SSA local",
                }
            ),
            "{error:?}"
        );
    }
}

#[test]
fn reordered_pair_results_keep_source_order_and_exact_physical_offsets() {
    let original = argument_result_owner(false, ArgumentHelperResultShape::Pair);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let pair = &types[PAIR.index() as usize];
    types[PAIR.index() as usize] = SemanticTypeDeclV1::new(
        pair.identity(),
        pair.layout_identity(),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            4,
            *pair.layout().backend_repr(),
            false,
            SemanticAggregateLayoutV1::new(vec![4, 4, 0], vec![]).unwrap(),
        )
        .unwrap(),
        pair.shape().clone(),
    );
    let owner = materialize(rebuild_owner(
        &original,
        types,
        semantic.functions().to_vec(),
    ));
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let root = SemanticFunctionIdV1::from_index(1);
    owner
        .with_checked_call_v1(
            root,
            root,
            SemanticBlockIdV1::from_index(0),
            &mut budget,
            |view| {
                let first = view.result_component(0).unwrap();
                let second = view.result_component(1).unwrap();
                assert_eq!(first.path(), [SemanticKirParameterProjectionV1::Field(0)]);
                assert_eq!(second.path(), [SemanticKirParameterProjectionV1::Field(2)]);
                assert_eq!((first.byte_offset(), second.byte_offset()), (4, 0));
                Ok(())
            },
        )
        .unwrap();
}

fn replace_result_abi(
    function: &SemanticFunctionDeclV1,
    value: SemanticAbiValueV1,
) -> SemanticFunctionDeclV1 {
    let old = function.abi();
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        old.identity(),
        old.layout_identity(),
        old.canon_abi(),
        old.extern_abi(),
        old.c_variadic(),
        old.can_unwind(),
        old.fixed_count(),
        old.source_input_types().to_vec(),
        old.source_output_type(),
        old.arguments().to_vec(),
        value,
    )
    .unwrap()
    .with_source_argument_ownership(old.source_argument_ownership().to_vec())
    .unwrap();
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        function.locals().to_vec(),
        function.entry(),
        function.blocks().to_vec(),
    )
    .unwrap()
}

#[test]
fn aggregate_result_shape_refuses_malformed_abi_carriers_and_pointer_leaves() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticAbiAdjustedTypeV1;
    for shape in [
        ArgumentHelperResultShape::Pair,
        ArgumentHelperResultShape::Array(2),
        ArgumentHelperResultShape::Array(3),
    ] {
        let source = argument_result_owner(false, shape);
        let semantic = source.source_semantic();
        let original = &semantic.functions()[0];
        let ty = original.abi().source_output_type();
        let result = original.abi().return_value();
        let mut values = vec![
            SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore),
            SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            ),
            SemanticAbiValueV1::new_with_adjusted_type(
                ty,
                SemanticAbiAdjustedTypeV1::new(
                    U32,
                    semantic.types()[U32.index() as usize].layout_identity(),
                    semantic.types()[U32.index() as usize].layout().clone(),
                ),
                result.mode().clone(),
            ),
        ];
        match result.mode() {
            SemanticAbiPassModeV1::Cast { cast, .. } => values.push(SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Cast {
                    pad_i32: true,
                    cast: cast.clone(),
                },
            )),
            SemanticAbiPassModeV1::Indirect { attributes, .. } => {
                values.push(SemanticAbiValueV1::new(
                    ty,
                    SemanticAbiPassModeV1::Indirect {
                        attributes: *attributes,
                        metadata_attributes: None,
                        on_stack: true,
                    },
                ))
            }
            _ => {}
        }
        for value in values {
            let function = replace_result_abi(original, value);
            assert!(
                helper_result_components_v1(
                    semantic.types(),
                    &function,
                    SemanticFunctionIdV1::from_index(0)
                )
                .is_err()
            );
        }
        let mut types = semantic.types().to_vec();
        types[U32.index() as usize] = call_result_reference_type();
        assert!(
            helper_result_components_v1(&types, original, SemanticFunctionIdV1::from_index(0))
                .is_err()
        );
    }
}

fn shapes() -> [(ArgumentHelperResultShape, usize); 10] {
    use ArgumentHelperResultShape::*;
    [
        (Zero, 0),
        (Singleton, 1),
        (Pair, 2),
        (Nominal, 2),
        (Nested, 3),
        (Array(0), 0),
        (Array(1), 1),
        (Array(2), 2),
        (Array(3), 3),
        (Array(4), 4),
    ]
}

#[test]
fn frozen_correspondence_result_guard_preserves_scalars_and_ignored_but_refuses_aggregate_components()
 {
    for (shape, width) in shapes() {
        let source = argument_result_owner(false, shape);
        let semantic = source.source_semantic();
        assert_eq!(
            crate::legacy_correspondence_result_supported_v4(
                semantic.types(),
                semantic.functions()[0].abi()
            ),
            width == 0
        );
        assert_eq!(
            crate::legacy_correspondence_source_results_supported_v4(semantic),
            width == 0
        );
    }
    for result in [ArgumentCallResult::Zero, ArgumentCallResult::Scalar] {
        let source = call_owner(false, result);
        assert!(crate::legacy_correspondence_result_supported_v4(
            source.source_semantic().types(),
            source.source_semantic().functions()[0].abi()
        ));
        assert!(crate::legacy_correspondence_source_results_supported_v4(
            source.source_semantic()
        ));
    }
}

#[test]
fn live_v4_and_nested_v5_producers_refuse_aggregate_result_evidence() {
    for shape in [
        ArgumentHelperResultShape::Singleton,
        ArgumentHelperResultShape::Pair,
    ] {
        let source = argument_result_owner(false, shape);
        let limits = ProductionSemanticKirLimitsV1::default();
        let report = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
            source.source_semantic(),
            SemanticFunctionIdV1::from_index(1),
        )
        .unwrap();
        let roster = argument_launch_roster(&source);
        let roots = materialization_launch_roots_v1(&source, &roster).unwrap();
        let (module, rows) = lower_module(&source, limits, Some(&roots)).unwrap();
        let owner = ProductionSemanticKirOwnerV1 {
            canonical_kernel_ir: ProductionCanonicalKernelIrV1::from_module(module.clone())
                .unwrap(),
            semantic_ssa: source,
            module: RetainedProductionKirModuleV1::Legacy(module),
            correspondence: rows,
            limits,
            launch_roots: Some(roots),
            generic_checks: Box::new([]),
        };
        assert!(matches!(
            crate::InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(&owner, &report),
            Err(crate::ProductionCorrespondenceEvidenceErrorV4::UnsupportedAggregateResult)
        ));
        assert!(matches!(
            crate::InertCanonicalMirToKirCorrespondenceEvidenceV5::from_live_owner(&owner, &report),
            Err(crate::ProductionCorrespondenceEvidenceErrorV5::NestedV4(
                crate::ProductionCorrespondenceEvidenceErrorV4::UnsupportedAggregateResult
            ))
        ));
    }
}

#[test]
fn aggregate_result_visits_meter_repeated_work_and_restore_scratch_after_errors() {
    let owner = materialize(argument_result_owner(
        false,
        ArgumentHelperResultShape::Nested,
    ));
    let root = SemanticFunctionIdV1::from_index(1);
    let run = |budget: &mut ArgumentBudgetV1<'_>| {
        owner.with_checked_call_v1(
            root,
            root,
            SemanticBlockIdV1::from_index(0),
            budget,
            |view| {
                let floor = view.entry.budget.storage();
                let before = view.entry.budget.work();
                view.visit_result_nodes(|_| Ok(()))?;
                let cost = view.entry.budget.work() - before;
                assert!(cost > 0);
                assert_eq!(view.entry.budget.storage(), floor);
                let before = view.entry.budget.work();
                view.visit_result_nodes(|_| Ok(()))?;
                assert_eq!(view.entry.budget.work() - before, cost);
                assert_eq!(view.entry.budget.storage(), floor);
                match view.visit_result_nodes(|_| {
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                }) {
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch) => {}
                    Err(error) => return Err(error),
                    Ok(()) => panic!("consumer error was ignored"),
                }
                assert_eq!(view.entry.budget.storage(), floor);
                Ok(())
            },
        )
    };
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    run(&mut budget).unwrap();
    let required = (budget.work(), budget.peak_storage());
    assert_eq!(budget.storage(), 23);
    for (work_limit, storage_limit) in [
        (required.0, required.1),
        (required.0 - 1, required.1),
        (required.0, required.1 - 1),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        if work_limit == required.0 && storage_limit == required.1 {
            run(&mut budget).unwrap();
        } else {
            assert!(matches!(
                run(&mut budget),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
        }
        assert_eq!(budget.storage(), 23);
    }
}

#[test]
fn aggregate_results_preserve_source_structure_multiple_returns_and_shared_roots() {
    for expanded in [false, true] {
        for (shape, width) in shapes() {
            let owner = materialize(argument_result_owner(expanded, shape));
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(23).unwrap();
            for root in [1, 2].map(SemanticFunctionIdV1::from_index) {
                for block in [0, 1].map(SemanticBlockIdV1::from_index) {
                    owner
                        .with_checked_call_v1(root, root, block, &mut budget, |view| {
                            assert!(view.result_is_aggregate());
                            assert_eq!(view.result_count(), width);
                            assert_eq!(view.result_local().index(), 0);
                            assert_eq!(
                                view.result_source_type(),
                                view.source().destination().unwrap().place().ty()
                            );
                            for ordinal in 0..width {
                                let result = view.result_component(ordinal).unwrap();
                                assert!(!result.path().is_empty());
                                assert_eq!(result.semantic_type(), U32);
                                assert_eq!(result.value(), &view.operation().results[ordinal]);
                                assert_eq!(result.byte_offset(), 4 * ordinal as u64);
                            }
                            assert!(view.result_component(width).is_none());
                            assert!(view.result_transport(width).is_none());
                            let mut nodes = Vec::new();
                            view.visit_result_nodes(|node| {
                                nodes.push((
                                    node.path().len(),
                                    node.semantic_type(),
                                    node.physical_range(),
                                ));
                                assert_eq!(node.results().len(), node.physical_range().len());
                                Ok(())
                            })?;
                            assert_eq!(
                                nodes.last().unwrap(),
                                &(0, view.result_source_type(), 0..width)
                            );
                            if matches!(
                                shape,
                                ArgumentHelperResultShape::Zero
                                    | ArgumentHelperResultShape::Singleton
                                    | ArgumentHelperResultShape::Pair
                                    | ArgumentHelperResultShape::Nominal
                                    | ArgumentHelperResultShape::Nested
                            ) {
                                assert!(nodes.iter().any(|(_, _, range)| range.is_empty()));
                            }
                            let mut returns = 0;
                            view.visit_returns(|site| {
                                returns += 1;
                                assert_eq!(site.component_count(), width);
                                let Some(Terminator::Return { values }) = &site.block().terminator
                                else {
                                    panic!("return")
                                };
                                assert_eq!(values.len(), width);
                                for (ordinal, value) in values.iter().enumerate() {
                                    assert_eq!(site.input(ordinal), Some(*value));
                                    assert!(site.conversion(ordinal).is_none());
                                }
                                if matches!(
                                    shape,
                                    ArgumentHelperResultShape::Pair
                                        | ArgumentHelperResultShape::Nominal
                                ) {
                                    assert_eq!(site.input(0), site.input(1));
                                }
                                assert!(site.input(width).is_none());
                                Ok(())
                            })?;
                            assert_eq!(
                                returns,
                                if shape == ArgumentHelperResultShape::Zero {
                                    1
                                } else {
                                    2
                                }
                            );
                            Ok(())
                        })
                        .unwrap();
                    assert_eq!(budget.storage(), 23);
                }
            }
        }
    }
}

#[test]
fn aggregate_result_phi_transports_each_component_from_the_original_ssa_definition() {
    for prefix in [None, Some(2), Some(4)] {
        for (shape, width) in shapes() {
            let source = transport_tests::diamond_owner_with_prefix(
                argument_result_owner(false, shape),
                prefix,
            );
            let owner = materialize(source);
            let root = SemanticFunctionIdV1::from_index(1);
            let plan = owner.semantic_ssa.plan_for_function(root).unwrap().plan();
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            for block in [1, 2] {
                let edge =
                    fe2o3_mir_model::SsaEdgeIdV1::new(fe2o3_mir_model::SsaBlockIdV1::new(block), 0);
                owner
                    .with_checked_call_v1(
                        root,
                        root,
                        SemanticBlockIdV1::from_index(block),
                        &mut budget,
                        |view| {
                            assert!(std::ptr::eq(
                                view.edge_definitions()?,
                                plan.edge_definitions(edge).unwrap()
                            ));
                            assert!(std::ptr::eq(
                                view.edge_arguments()?,
                                plan.edge_arguments(edge).unwrap()
                            ));
                            let Some(Terminator::Branch { arguments, .. }) =
                                &view.block().terminator
                            else {
                                panic!("branch")
                            };
                            let mut previous = None;
                            if width > 0 && prefix.is_some() {
                                let logical = view
                                    .edge_arguments()?
                                    .iter()
                                    .position(|argument| argument.variable().get() == 6)
                                    .unwrap();
                                let physical = view.result_transport(0).unwrap().slot() as usize;
                                assert_eq!(physical, if prefix == Some(2) { 2 } else { 0 });
                                assert_ne!(
                                    physical, logical,
                                    "SSA ordinal is not a physical component slot"
                                );
                            }
                            for ordinal in 0..width {
                                let result = view
                                    .result_transport(ordinal)
                                    .expect("result phi component");
                                assert_eq!(result.value(), view.operation().results[ordinal].id);
                                assert_eq!(arguments[result.slot() as usize], result.value());
                                assert!(result.conversion().is_none());
                                if let Some(previous) = previous {
                                    assert_eq!(result.slot(), previous + 1);
                                }
                                previous = Some(result.slot());
                            }
                            assert!(view.result_transport(width).is_none());
                            Ok(())
                        },
                    )
                    .unwrap();
                assert_eq!(budget.storage(), 0);
            }
        }
    }
}

#[test]
fn aggregate_component_pool_rejects_gaps_overlap_wrong_kinds_and_noncanonical_empty_spans() {
    let source = argument_result_owner(false, ArgumentHelperResultShape::Pair);
    let limits = ProductionSemanticKirLimitsV1::default();
    let (module, rows) = lower_argument_owner(&source, limits).unwrap();
    let returned = rows
        .call_returns
        .iter()
        .position(|row| row.components().count > 0)
        .unwrap();
    for mutation in 0..6 {
        let mut changed = rows.clone();
        let SemanticKirCallReturnKindV1::Return { components } =
            &mut changed.call_returns[returned].kind
        else {
            panic!("return")
        };
        match mutation {
            0 => components.count -= 1,
            1 => components.first += 1,
            2 => *components = CallComponentSpanV1 { first: 1, count: 0 },
            3 => {
                changed.call_result_components[components.first as usize] =
                    CallResultComponentV1::Transport {
                        slot: 0,
                        conversion: None,
                    }
            }
            4 => {
                let span = *components;
                let next = changed
                    .call_returns
                    .iter_mut()
                    .skip(returned + 1)
                    .find(|row| row.components().count > 0)
                    .unwrap();
                next.kind = SemanticKirCallReturnKindV1::Return { components: span };
            }
            _ => {
                let mut pool = changed.call_result_components.to_vec();
                pool.push(pool[0]);
                changed.call_result_components = pool.into_boxed_slice();
            }
        }
        check_both(
            &source,
            &module,
            source.source_semantic().roots(),
            limits.max_blocks,
            &changed,
            Expected::CorrespondenceMismatch,
        );
    }
}

#[test]
fn coordinated_same_typed_result_swap_still_requires_source_lowering_replay() {
    let source = argument_result_owner(false, ArgumentHelperResultShape::Nested);
    let limits = ProductionSemanticKirLimitsV1::default();
    let roster = argument_launch_roster(&source);
    let roots = materialization_launch_roots_v1(&source, &roster).unwrap();
    let (mut module, mut rows) = lower_module(&source, limits, Some(&roots)).unwrap();
    let helper = module
        .functions
        .iter_mut()
        .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
        .unwrap();
    for block in &mut helper.body.as_mut().unwrap().blocks {
        if let Some(Terminator::Return { values }) = &mut block.terminator {
            assert_eq!(values.len(), 3);
            assert_ne!(values[0], values[1]);
            values.swap(0, 1);
        }
    }
    verify_module(&module).unwrap();
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        limits.max_blocks,
        &rows,
        Expected::CorrespondenceMismatch,
    );
    for row in &rows.call_returns {
        if row.semantic_function.index() == 0 {
            let range = row.components().range().unwrap();
            assert_eq!(range.len(), 3);
            rows.call_result_components
                .swap(range.start, range.start + 1);
        }
    }
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        limits.max_blocks,
        &rows,
        Expected::Accepted,
    );
    let owner = ProductionSemanticKirOwnerV1 {
        canonical_kernel_ir: ProductionCanonicalKernelIrV1::from_module(module.clone()).unwrap(),
        semantic_ssa: source,
        module: RetainedProductionKirModuleV1::Legacy(module),
        correspondence: rows,
        limits,
        launch_roots: Some(roots),
        generic_checks: Box::new([]),
    };
    assert!(matches!(
        owner.verify_equivalence(),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}
