use super::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;
use production_call_instances_v1::{
    ProductionCallInstanceErrorV1, with_production_call_instances_v1,
};

#[path = "production_instance_coordinates_owner_v1_tests.rs"]
mod coordinates_owner_tests;

fn with_plan(test: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut ArgumentBudgetV1<'_>)) {
    with_plan_owner(resource_tests::helper_closure_semantic_owner(), test);
}

fn with_plan_owner(
    owner: ProductionSemanticMirOwnerV1,
    test: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut ArgumentBudgetV1<'_>),
) {
    let mut ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let root = ssa.source_semantic().roots()[0];
    with_production_call_instances_v1(&ssa, root, &mut budget, |plan, budget| {
        test(plan, budget);
        Ok::<_, ProductionCallInstanceErrorV1>(())
    })
    .unwrap();
    assert_eq!(budget.storage(), capture.retained_storage());
}

fn statement_order_owner() -> ProductionSemanticMirOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let original = resource_tests::helper_closure_semantic_owner();
    let semantic = original.semantic();
    let root = &semantic.functions()[0];
    let source = root.source();
    let scalar = SemanticTypeIdV1::from_index(1);
    let mut types = semantic.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([150; 32]),
        SemanticLayoutIdentityV1::from_sha256([151; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX as u128),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    let mut locals = root.locals().to_vec();
    // The inherited return local has identity 207; appended locals stay canonical.
    for tag in [220, 221] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            scalar,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    let assignment = |local, bits| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], scalar).unwrap(),
                SemanticRvalueV1::new(
                    scalar,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            scalar,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(bits, 4).unwrap(),
                            ),
                        ),
                    )),
                ),
            )),
        )
    };
    let nop = || SemanticStatementV1::new(source, SemanticStatementKindV1::Nop);
    let mut blocks = root.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        source,
        vec![nop(), assignment(1, 3), nop(), assignment(2, 7), nop()],
        blocks[0].terminator().clone(),
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        source,
        root.abi().clone(),
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let mut functions = semantic.functions().to_vec();
    functions[0] = root;
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn instance_wrapper_preserves_statement_order_and_zero_operation_boundaries() {
    with_plan_owner(statement_order_owner(), |plan, budget| {
        for mutation in 0..5 {
            let floor = budget.storage();
            let (mut caller, callee, storage) = lower_pair(plan, budget);
            let rows = &mut caller.statement_operation_spans;
            assert_eq!(rows.len(), 5);
            assert_eq!(
                [
                    rows[0].operation_count,
                    rows[2].operation_count,
                    rows[4].operation_count
                ],
                [0; 3]
            );
            assert!(rows[1].operation_count > 0 && rows[3].operation_count > 0);
            match mutation {
                0 => {}
                1 => {
                    let first = rows[1].first_operation_ordinal;
                    rows[1].first_operation_ordinal = rows[3].first_operation_ordinal;
                    rows[3].first_operation_ordinal = first;
                }
                2 => rows[2].first_operation_ordinal = rows[1].first_operation_ordinal,
                3 => rows[4].first_operation_ordinal = rows[1].first_operation_ordinal,
                4 => {
                    caller.terminator_operation_spans[0].first_operation_ordinal =
                        rows[1].first_operation_ordinal
                }
                _ => unreachable!(),
            }
            let result = with_production_instance_correspondence_v1(plan, budget, |map, budget| {
                map.append_lowered(plan.root(), &caller, budget)
            });
            if mutation == 0 {
                result.unwrap();
            } else {
                assert_eq!(
                    result,
                    Err(InstanceCorrespondenceErrorV1::Source),
                    "mutation {mutation}"
                );
            }
            drop(caller);
            drop(callee);
            budget.release_storage(storage).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    });
}

fn lower_pair(
    plan: &ProductionCallInstancePlanV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> (LoweredFunctionResultV1, LoweredFunctionResultV1, usize) {
    use resource_tests::emission_placement_lowering_tests::lower_placed_function;
    let root = plan.instance(plan.root()).unwrap().function();
    let child = plan.calls(plan.root()).unwrap()[0].child().unwrap();
    let caller = lower_placed_function(
        plan.owner(),
        root,
        SemanticEmissionPlacementV1::default(),
        budget,
    )
    .unwrap();
    let callee = lower_placed_function(
        plan.owner(),
        plan.instance(child).unwrap().function(),
        SemanticEmissionPlacementV1 {
            first_block: 17,
            first_value: 100,
        },
        budget,
    )
    .unwrap();
    let storage = CallReturnBufferV1::bytes(
        caller.call_returns.sites.rows.len() + callee.call_returns.sites.rows.len(),
        caller.call_returns.components.rows.len() + callee.call_returns.components.rows.len(),
    )
    .unwrap();
    (caller, callee, storage)
}

#[test]
fn cursor_selected_instance_cannot_be_relabelled_on_append() {
    use resource_tests::emission_placement_lowering_tests::lower_placed_function_with_availability_v29;
    with_plan(|plan, budget| {
        let floor = budget.storage();
        let mut caller =
            with_execution_availability_v29(plan, plan.root(), budget, |cursor, budget| {
                lower_placed_function_with_availability_v29(
                    plan.owner(),
                    plan.instance(plan.root()).unwrap().function(),
                    SemanticEmissionPlacementV1::default(),
                    budget,
                    Some(cursor),
                )
            })
            .unwrap();
        assert_eq!(caller.source_call_instance, Some(plan.root()));
        let storage = CallReturnBufferV1::bytes(
            caller.call_returns.sites.rows.len(),
            caller.call_returns.components.rows.len(),
        )
        .unwrap();
        caller.source_call_instance = plan.calls(plan.root()).unwrap()[0].child();
        let storage = storage
            + caller
                .scoped_initialization
                .as_ref()
                .unwrap()
                .retained_storage
            + caller
                .scoped_memory_anchors
                .as_ref()
                .unwrap()
                .retained_storage()
                .unwrap();
        let rejected = with_production_instance_correspondence_v1(plan, budget, |map, budget| {
            map.append_lowered(plan.root(), &caller, budget)
        });
        assert_eq!(rejected, Err(InstanceCorrespondenceErrorV1::Source));
        drop(caller);
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn instance_wrapper_rejects_owner_relabel_and_invalid_source_census() {
    with_plan(|plan, budget| {
        for mutation in 0..9 {
            let floor = budget.storage();
            let (mut caller, callee, storage) = lower_pair(plan, budget);
            let foreign = plan
                .instance(plan.calls(plan.root()).unwrap()[0].child().unwrap())
                .unwrap()
                .function();
            match mutation {
                0 => {
                    for row in &mut caller.blocks {
                        row.correspondence_owner = foreign;
                    }
                    for row in &mut caller.statement_operation_spans {
                        row.correspondence_owner = foreign;
                    }
                    for row in &mut caller.terminator_operation_spans {
                        row.correspondence_owner = foreign;
                    }
                    for row in &mut caller.synthetic_operation_spans {
                        row.correspondence_owner = foreign;
                    }
                    for row in &mut caller.call_returns.sites.rows {
                        row.correspondence_owner = foreign;
                    }
                }
                1 => {
                    caller.terminator_operation_spans[1].semantic_block =
                        SemanticBlockIdV1::from_index(u32::MAX)
                }
                2 => {
                    caller.terminator_operation_spans[1].kernel_ir_block =
                        caller.blocks[0].kernel_ir_block
                }
                3 => {
                    caller.terminator_operation_spans.pop();
                }
                4 => {
                    caller
                        .terminator_operation_spans
                        .push(caller.terminator_operation_spans[1]);
                }
                5 => {
                    caller.blocks.pop();
                }
                6 => caller.blocks[1].source_statement_count = 1,
                7 => caller
                    .statement_operation_spans
                    .push(SemanticKirStatementOperationSpanV1 {
                        correspondence_owner: caller.blocks[0].correspondence_owner,
                        semantic_function: caller.blocks[0].semantic_function,
                        semantic_block: caller.blocks[0].semantic_block,
                        statement_ordinal: u32::MAX,
                        kernel_ir_block: caller.blocks[0].kernel_ir_block,
                        first_operation_ordinal: 0,
                        operation_count: 0,
                    }),
                8 => caller
                    .synthetic_operation_spans
                    .push(SemanticKirSyntheticOperationSpanV1 {
                        correspondence_owner: caller.blocks[0].correspondence_owner,
                        semantic_function: caller.blocks[0].semantic_function,
                        rule: SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap,
                        kernel_ir_block: caller.blocks[1].kernel_ir_block,
                        first_operation_ordinal: 0,
                        operation_count: 1,
                    }),
                _ => unreachable!(),
            }
            let result = with_production_instance_correspondence_v1(plan, budget, |map, budget| {
                map.append_lowered(plan.root(), &caller, budget)
            });
            assert_eq!(
                result,
                Err(InstanceCorrespondenceErrorV1::Source),
                "mutation {mutation}"
            );
            drop(caller);
            drop(callee);
            budget.release_storage(storage).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn instance_wrapper_checks_generated_edges_and_preserves_original_return_anchors() {
    with_plan(|plan, budget| {
        let floor = budget.storage();
        let (caller, callee, storage) = lower_pair(plan, budget);
        let call = &plan.calls(plan.root()).unwrap()[0];
        let child = call.child().unwrap();
        with_production_instance_correspondence_v1(plan, budget, |map, budget| {
            map.append_lowered(plan.root(), &caller, budget)?;
            map.append_lowered(child, &callee, budget)?;
            let mut expanded = map.splice(call, caller.function, callee.function, BlockId(18), BlockId(19), budget)?;
            assert!(map.returns.rows.iter().any(|row| row.instance == child));
            assert_eq!(map.spans().iter().filter(|row| row.removed_call.is_some()).count(), 1);
            assert!(map.controls().iter().any(|row| matches!(
                row.origin,
                InstanceControlOriginV1::ExpandedReturn { call: actual } if actual == call.occurrence()
            )));
            map.check_coordinates(plan.root(), &expanded.caller, budget)?;
            let prefix = expanded.caller.body.as_mut().unwrap().blocks.iter_mut()
                .find(|block| block.id == expanded.split.call.block).unwrap();
            let Some(Terminator::Branch { target, .. }) = &mut prefix.terminator else { panic!("generated branch"); };
            *target = expanded.split.continuation;
            assert_eq!(map.check_coordinates(plan.root(), &expanded.caller, budget), Err(InstanceCorrespondenceErrorV1::Control));
            let retained = expanded.additional_storage_bytes;
            drop(expanded);
            budget.release_storage(retained)?;
            Ok::<_, InstanceCorrespondenceErrorV1>(())
        }).unwrap();
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn instance_wrapper_budget_failure_after_splice_releases_output_not_owner_floor() {
    with_plan(|plan, source_budget| {
        let source_floor = source_budget.storage();
        let mut run = |limit| {
            let (caller, callee, storage) = lower_pair(plan, source_budget);
            let floor = source_budget.storage();
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
            // Retained source/plan/lowering payload remains charged independently.
            budget.reserve_storage(floor).unwrap();
            let mut committed = false;
            let result =
                with_production_instance_correspondence_v1(plan, &mut budget, |map, budget| {
                    let call = &plan.calls(plan.root()).unwrap()[0];
                    map.append_lowered(plan.root(), &caller, budget)?;
                    map.append_lowered(call.child().unwrap(), &callee, budget)?;
                    let result = map.splice(
                        call,
                        caller.function,
                        callee.function,
                        BlockId(18),
                        BlockId(19),
                        budget,
                    );
                    // This flag is set only after the underlying splicer returned Ok
                    // and its generated-edge check completed, before final bounds checking.
                    committed = map.anchors.rows.iter().any(|row| row.removed);
                    match result {
                        Ok(expanded) => {
                            let retained = expanded.additional_storage_bytes;
                            drop(expanded);
                            budget.release_storage(retained)?;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    }
                });
            assert_eq!(budget.storage(), floor);
            source_budget.release_storage(storage).unwrap();
            assert_eq!(source_budget.storage(), source_floor);
            (result, budget.work(), committed)
        };
        let (result, exact, committed) = run(1_000_000);
        result.unwrap();
        assert!(committed);
        let (result, _, committed) = run(exact);
        result.unwrap();
        assert!(committed);
        let (result, _, committed) = run(exact - 1);
        assert!(matches!(
            result,
            Err(InstanceCorrespondenceErrorV1::Resource(
                ArgumentResourceV1::Work(_)
            ))
        ));
        assert!(
            committed,
            "failure must occur after a successful underlying splice"
        );
    });
}

fn span(
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    first: u32,
    count: u32,
) -> InstanceMappedSpanV1 {
    let source = SemanticKirTerminatorOperationSpanV1 {
        correspondence_owner: function,
        semantic_function: function,
        semantic_block: SemanticBlockIdV1::from_index(0),
        kernel_ir_block: BlockId(7),
        first_operation_ordinal: first,
        operation_count: count,
    };
    InstanceMappedSpanV1 {
        instance,
        source: InstanceSpanSourceV1::Terminator(source),
        segments: [
            Some(InstancePhysicalSpanV1 {
                block: BlockId(7),
                first,
                count,
            }),
            None,
        ],
        removed_call: None,
    }
}

fn split(ordinal: usize) -> CallInstanceSplitV1 {
    CallInstanceSplitV1 {
        call: FunctionOperationLocation::new(BlockId(7), ordinal),
        entry: BlockId(31),
        callee_entry: BlockId(17),
        continuation: BlockId(32),
        callee_blocks: 3,
        returns: 2,
        result_components: 1,
    }
}

#[test]
fn instance_span_splits_around_removed_call_and_keeps_original_source() {
    with_plan(|plan, _| {
        let instance = plan.root();
        let function = plan.instance(instance).unwrap().function();
        let call = plan.calls(instance).unwrap()[0].occurrence();
        let original = span(instance, function, 0, 5);
        let mapped = original.after_splice(call, split(2)).unwrap();
        assert_eq!(mapped.source, original.source);
        assert_eq!(mapped.instance, instance);
        assert_eq!(mapped.removed_call, Some(call));
        assert_eq!(
            mapped.segments,
            [
                Some(InstancePhysicalSpanV1 {
                    block: BlockId(7),
                    first: 0,
                    count: 2
                }),
                Some(InstancePhysicalSpanV1 {
                    block: BlockId(32),
                    first: 0,
                    count: 2
                }),
            ]
        );
        let suffix = span(instance, function, 4, 1)
            .after_splice(call, split(2))
            .unwrap();
        assert_eq!(
            suffix.segments[0],
            Some(InstancePhysicalSpanV1 {
                block: BlockId(32),
                first: 1,
                count: 1
            })
        );
        assert_eq!(suffix.removed_call, None);
    });
}

#[test]
fn instance_empty_spans_keep_exact_boundaries_and_call_only_span_has_tombstone() {
    with_plan(|plan, _| {
        let instance = plan.root();
        let function = plan.instance(instance).unwrap().function();
        let call = plan.calls(instance).unwrap()[0].occurrence();
        for (first, block, expected) in [(0, 7, 0), (2, 7, 2), (3, 32, 0), (5, 32, 2)] {
            let mapped = span(instance, function, first, 0)
                .after_splice(call, split(2))
                .unwrap();
            assert_eq!(
                mapped.segments,
                [
                    Some(InstancePhysicalSpanV1 {
                        block: BlockId(block),
                        first: expected,
                        count: 0
                    }),
                    None
                ]
            );
            assert_eq!(mapped.removed_call, None);
        }
        let mapped = span(instance, function, 2, 1)
            .after_splice(call, split(2))
            .unwrap();
        assert_eq!(mapped.segments, [None, None]);
        assert_eq!(mapped.removed_call, Some(call));
    });
}

#[test]
fn instance_span_rejects_foreign_instance_second_removal_and_overflow() {
    with_plan(|plan, _| {
        let call = &plan.calls(plan.root()).unwrap()[0];
        let child = call.child().unwrap();
        let function = plan.instance(child).unwrap().function();
        assert_eq!(
            span(child, function, 0, 5).after_splice(call.occurrence(), split(2)),
            Err(InstanceCorrespondenceErrorV1::SpanCoverage)
        );
        let mut original = span(plan.root(), function, 0, 5);
        original.removed_call = Some(call.occurrence());
        assert_eq!(
            original.after_splice(call.occurrence(), split(2)),
            Err(InstanceCorrespondenceErrorV1::SpanCoverage)
        );
        let overflowing = span(plan.root(), function, u32::MAX, 2);
        assert_eq!(
            overflowing.after_splice(call.occurrence(), split(2)),
            Err(InstanceCorrespondenceErrorV1::Resource(
                ArgumentResourceV1::Arithmetic
            ))
        );
    });
}

#[test]
fn instance_control_checks_ordered_values_targets_and_parameter_ids() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000);
    let mut block = BasicBlock::new(BlockId(8));
    block.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![ValueId(2), ValueId(1)],
    });
    assert_eq!(
        instance_check_branch_v1(&block, BlockId(9), &[ValueId(1), ValueId(2)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Control)
    );
    assert_eq!(
        instance_check_branch_v1(&block, BlockId(7), &[ValueId(2), ValueId(1)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Control)
    );
    instance_check_branch_v1(&block, BlockId(9), &[ValueId(2), ValueId(1)], &mut budget).unwrap();
    block.parameters = vec![
        ValueDef::new(ValueId(4), Type::INDEX),
        ValueDef::new(ValueId(3), Type::INDEX),
    ];
    assert_eq!(
        instance_check_parameters_v1(&block, &[ValueId(3), ValueId(4)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Control)
    );
    instance_check_parameters_v1(&block, &[ValueId(4), ValueId(3)], &mut budget).unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = ArgumentBudgetV1::new(&mut work, 7);
    budget.reserve_storage(7).unwrap();
    assert!(matches!(
        instance_check_branch_v1(&block, BlockId(9), &[ValueId(2), ValueId(1)], &mut budget),
        Err(InstanceCorrespondenceErrorV1::Resource(
            ArgumentResourceV1::Work(_)
        ))
    ));
    assert_eq!(budget.storage(), 7);
}

#[test]
fn instance_row_growth_prepays_relocation_coexistence_and_exact_storage_boundary() {
    let run = |limit| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(7).unwrap();
        let mut storage = 0;
        let mut rows = InstanceRowsV1::<u32>::new();
        rows.reserve(1, &mut budget, &mut storage).unwrap();
        rows.rows.push(1);
        let result = rows.reserve(4, &mut budget, &mut storage);
        let peak = budget.peak_storage();
        drop(rows);
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), 7);
        (result, peak, budget.work())
    };
    let (result, peak, work) = run(1_000);
    result.unwrap();
    assert_eq!(peak, 7 + 12 * std::mem::size_of::<u32>());
    assert_eq!(work, 1);
    run(peak).0.unwrap();
    assert!(matches!(
        run(peak - 1).0,
        Err(InstanceCorrespondenceErrorV1::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
}

#[test]
fn instance_scope_releases_only_its_rows_on_success_and_failure() {
    with_plan(|plan, budget| {
        let floor = budget.storage();
        for fail in [false, true] {
            let result = with_production_instance_correspondence_v1(plan, budget, |map, budget| {
                map.values.reserve(7, budget, &mut map.storage)?;
                map.values.rows.push(ValueId(1));
                budget.reserve_storage(13)?;
                if fail {
                    Err(InstanceCorrespondenceErrorV1::Control)
                } else {
                    Ok(())
                }
            });
            if fail {
                assert_eq!(result, Err(InstanceCorrespondenceErrorV1::Control));
            } else {
                result.unwrap();
            }
            assert_eq!(budget.storage(), floor + 13);
            budget.release_storage(13).unwrap();
        }
    });
}
