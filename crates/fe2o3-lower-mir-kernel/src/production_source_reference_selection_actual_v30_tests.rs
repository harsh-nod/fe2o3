fn actual_selection_fixture(
    graph: &SourceReferenceSelectionGraphV29,
    recurrence: bool,
) -> (Function, SourceReferenceSelectionActualV30) {
    let pointer_type = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut entry = BasicBlock::new(BlockId(10));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(11),
        then_arguments: vec![],
        else_target: BlockId(12),
        else_arguments: vec![],
    });
    let mut first = BasicBlock::new(BlockId(11));
    first.terminator = Some(if recurrence {
        Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(13),
            then_arguments: vec![ValueId(0)],
            else_target: BlockId(13),
            else_arguments: vec![ValueId(0)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(13),
            arguments: vec![ValueId(0)],
        }
    });
    let mut second = BasicBlock::new(BlockId(12));
    second.terminator = Some(Terminator::Branch {
        target: BlockId(13),
        arguments: vec![ValueId(1)],
    });
    let mut selected = BasicBlock::new(BlockId(13));
    selected
        .parameters
        .push(ValueDef::new(ValueId(3), pointer_type.clone()));
    selected.terminator = Some(if recurrence {
        Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(13),
            then_arguments: vec![ValueId(3)],
            else_target: BlockId(14),
            else_arguments: vec![],
        }
    } else {
        Terminator::Branch {
            target: BlockId(14),
            arguments: vec![],
        }
    });
    let mut end = BasicBlock::new(BlockId(14));
    end.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "source-selection-structural-fixture",
        fe2o3_kernel_ir::Signature::new(
            vec![pointer_type.clone(), pointer_type, Type::BOOL],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, first, second, selected, end],
    );
    let nodes = graph
        .nodes
        .iter()
        .map(|original| {
            let pointer = match original.step {
                SourceReferenceSelectionStepV29::Leaf(
                    SourceExternalReferenceOriginV29::Issued { recipe, .. },
                ) => ValueId(match recipe.block.index() {
                    1 => 0,
                    5 => 1,
                    _ => panic!("exact fixture issuer"),
                }),
                SourceReferenceSelectionStepV29::Parameter { .. }
                | SourceReferenceSelectionStepV29::Alias { .. } => ValueId(3),
                other => panic!("unexpected original selection step: {other:?}"),
            };
            SourceReferenceSelectionActualNodeV30 {
                original: *original,
                pointer,
                element: ScalarType::U32,
                space: AddressSpace::Global,
                access: AccessMode::ReadWrite,
                parameter: matches!(
                    original.step,
                    SourceReferenceSelectionStepV29::Parameter { .. }
                )
                .then_some((BlockId(13), 0)),
                invocation: None,
            }
        })
        .collect();
    let edges = graph
        .edges
        .iter()
        .map(|original| SourceReferenceSelectionActualEdgeV30 {
            original: *original,
            source: BlockId(match original.edge.source().get() {
                3 => 11,
                7 => 12,
                10 if recurrence => 13,
                _ => panic!("exact original predecessor"),
            }),
            ordinal: original.edge.ordinal() as usize,
            target: BlockId(13),
            parameter: 0,
            argument: None,
        })
        .collect();
    (function, SourceReferenceSelectionActualV30 { nodes, edges })
}

fn assert_actual_selection_error(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "reference selection differs from its exact emitted SSA edges",
            })
        ),
        "{result:?}"
    );
}

fn actual_entry_selection_fixture(
    graph: &SourceReferenceSelectionGraphV29,
) -> (Function, SourceReferenceSelectionActualV30) {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut invocation = BasicBlock::new(BlockId(10));
    invocation.terminator = Some(Terminator::Branch {
        target: BlockId(13),
        arguments: vec![ValueId(0)],
    });
    let mut header = BasicBlock::new(BlockId(13));
    header
        .parameters
        .push(ValueDef::new(ValueId(3), ty.clone()));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(13),
        then_arguments: vec![ValueId(3)],
        else_target: BlockId(14),
        else_arguments: vec![],
    });
    let mut end = BasicBlock::new(BlockId(14));
    end.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "source-selection-invocation-structural-fixture",
        fe2o3_kernel_ir::Signature::new(vec![ty, Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![invocation, header, end],
    );
    let nodes = graph
        .nodes
        .iter()
        .map(|original| {
            let selected = matches!(
                original.step,
                SourceReferenceSelectionStepV29::Parameter { .. }
            ) || (original.value.instance == ProductionCallInstanceIdV1(1)
                && matches!(original.step, SourceReferenceSelectionStepV29::Alias { .. }));
            let invocation = match original.step {
                SourceReferenceSelectionStepV29::Parameter {
                    invocation: Some(input),
                    ..
                } => Some(SourceReferenceSelectionActualInvocationV30 {
                    input,
                    source: BlockId(10),
                    original: ValueId(0),
                    argument: None,
                }),
                _ => None,
            };
            SourceReferenceSelectionActualNodeV30 {
                original: *original,
                pointer: ValueId(if selected { 3 } else { 0 }),
                element: ScalarType::U32,
                space: AddressSpace::Global,
                access: AccessMode::ReadWrite,
                parameter: matches!(
                    original.step,
                    SourceReferenceSelectionStepV29::Parameter { .. }
                )
                .then_some((BlockId(13), 0)),
                invocation,
            }
        })
        .collect();
    let edges = graph
        .edges
        .iter()
        .map(|original| {
            assert_eq!(original.edge, SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0));
            SourceReferenceSelectionActualEdgeV30 {
                original: *original,
                source: BlockId(13),
                ordinal: 0,
                target: BlockId(13),
                parameter: 0,
                argument: None,
            }
        })
        .collect();
    (function, SourceReferenceSelectionActualV30 { nodes, edges })
}

#[test]
fn actual_entry_reference_selection_keeps_invocation_distinct_from_recurrence() {
    with_plan(&entry_loop_selection_owner(), |plan, budget| {
        let child = ProductionCallInstanceIdV1(1);
        let source = original_borrow(plan, child, 0);
        with_source_reference_selection_v29(
            plan,
            child,
            site(0),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, budget| {
                for fault in 0..6 {
                    with_canonical_call_scratch_v1(budget, |budget| {
                        let (function, mut bound) = actual_entry_selection_fixture(graph);
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        source_reference_selection_check_transport_v30(
                            &function, &actual, &mut bound, budget,
                        )?;
                        assert_eq!(
                            bound.nodes[0].invocation.unwrap().argument,
                            Some(ValueId(0))
                        );
                        assert_eq!(bound.edges[0].argument, Some(ValueId(3)));
                        let (mut function, mut bound) = actual_entry_selection_fixture(graph);
                        match fault {
                            0 => bound.nodes[0].invocation = None,
                            1 => bound.nodes[0].invocation.as_mut().unwrap().source = BlockId(13),
                            2 => {
                                bound.nodes[0].invocation.as_mut().unwrap().input =
                                    bound.edges[0].original.input
                            }
                            3 => bound.nodes[0].invocation.as_mut().unwrap().original = ValueId(3),
                            4 => bound.edges[0].source = BlockId(10),
                            5 => {
                                function.body.as_mut().unwrap().blocks[0].terminator =
                                    Some(Terminator::Branch {
                                        target: BlockId(14),
                                        arguments: vec![],
                                    })
                            }
                            _ => unreachable!(),
                        }
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        assert_actual_selection_error(
                            source_reference_selection_check_transport_v30(
                                &function, &actual, &mut bound, budget,
                            ),
                        );
                        let (function, mut restored) = actual_entry_selection_fixture(graph);
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        source_reference_selection_check_transport_v30(
                            &function,
                            &actual,
                            &mut restored,
                            budget,
                        )
                    })?;
                }
                Ok(())
            },
        )
        .unwrap();
    });
}

#[test]
fn actual_reference_selection_replays_ordered_diamond_parallel_edges_and_loop_arguments() {
    for recurrence in [false, true] {
        with_plan(&selection_owner(recurrence, recurrence), |plan, budget| {
            let source = original_borrow(plan, plan.root, 8);
            with_source_reference_selection_v29(
                plan,
                plan.root,
                site(8),
                ExecutionOperandV29::RvaluePlace,
                source,
                budget,
                |graph, budget| {
                    with_canonical_call_scratch_v1(budget, |budget| {
                        let (function, mut bound) = actual_selection_fixture(graph, recurrence);
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        source_reference_selection_check_transport_v30(
                            &function, &actual, &mut bound, budget,
                        )?;
                        assert_eq!(
                            bound
                                .edges
                                .iter()
                                .map(|edge| (edge.source.0, edge.ordinal, edge.argument.unwrap().0))
                                .collect::<Vec<_>>(),
                            if recurrence {
                                vec![(11, 0, 0), (11, 1, 0), (12, 0, 1), (13, 0, 3)]
                            } else {
                                vec![(11, 0, 0), (12, 0, 1)]
                            }
                        );
                        assert_eq!(bound.nodes[0].pointer, ValueId(3));
                        Ok(())
                    })
                },
            )
            .unwrap();
        });
    }
}

#[test]
fn actual_reference_selection_rejects_swapped_missing_duplicate_wrong_typed_and_bypass_rows() {
    with_plan(&selection_owner(false, false), |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, budget| {
                for fault in 0..8 {
                    with_canonical_call_scratch_v1(budget, |budget| {
                        let (mut function, mut bound) = actual_selection_fixture(graph, false);
                        match fault {
                            0 => {
                                let first = bound.edges[0].original.input;
                                bound.edges[0].original.input = bound.edges[1].original.input;
                                bound.edges[1].original.input = first;
                            }
                            1 => {
                                bound.edges.pop();
                            }
                            2 => bound.edges[1].source = bound.edges[0].source,
                            3 => bound.edges[0].ordinal = 1,
                            4 => bound.nodes[0].space = AddressSpace::Generic,
                            5 => bound.nodes[0].access = AccessMode::ReadOnly,
                            6 => bound.edges[0].target = BlockId(14),
                            7 => {
                                function.body.as_mut().unwrap().blocks[2].terminator =
                                    Some(Terminator::Branch {
                                        target: BlockId(13),
                                        arguments: vec![ValueId(0)],
                                    })
                            }
                            _ => unreachable!(),
                        }
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        assert_actual_selection_error(
                            source_reference_selection_check_transport_v30(
                                &function, &actual, &mut bound, budget,
                            ),
                        );
                        let (function, mut restored) = actual_selection_fixture(graph, false);
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        source_reference_selection_check_transport_v30(
                            &function,
                            &actual,
                            &mut restored,
                            budget,
                        )
                    })?;
                }
                Ok(())
            },
        )
        .unwrap();
    });
}

thread_local! {
    static ACTUAL_SELECTION_PHASES: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) };
}

fn observe_actual_source_selection(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    observe_original(pending, instances, plan, budget)?;
    let observation = SOURCE_EMISSION_OBSERVATION_V29.get().unwrap();
    assert_eq!(
        observation.owner,
        std::ptr::from_ref(instances.owner()) as usize
    );
    let phase = match observation.phase {
        SourceEmissionPhaseV29::Admission => 0,
        SourceEmissionPhaseV29::ConstructionReplay => 1,
        SourceEmissionPhaseV29::ConsumerReplay => panic!("materialization phase"),
    };
    let floor = budget.storage();
    let index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
    let child = ProductionCallInstanceIdV1(1);
    let source = original_borrow(plan, child, 0);
    with_source_reference_selection_actual_v30(
        plan,
        &index,
        child,
        site(0),
        ExecutionOperandV29::RvaluePlace,
        source,
        budget,
        |graph, actual, _| {
            assert_eq!(actual.nodes.len(), graph.nodes.len());
            assert_eq!(actual.edges.len(), graph.edges.len());
            assert!(matches!(
                actual.nodes[0].original.step,
                SourceReferenceSelectionStepV29::CallArgument { .. }
            ));
            assert!(actual.nodes.iter().any(|node| matches!(
                node.original.step,
                SourceReferenceSelectionStepV29::Leaf(
                    SourceExternalReferenceOriginV29::Issued { .. }
                )
            )));
            let mut counts = ACTUAL_SELECTION_PHASES.get();
            counts[phase] += 1;
            ACTUAL_SELECTION_PHASES.set(counts);
            Ok(())
        },
    )?;
    index.discard(budget)?;
    assert_eq!(budget.storage(), floor);
    Ok(())
}

fn install_actual_source_selection_observer(
    _: &ExecutionInstancesV29<'_>,
    _: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    _: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(Some(observe_actual_source_selection));
    Ok(())
}

#[test]
fn original_helper_reference_selection_rejoins_exact_source_archive_and_actual_call_parameter() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(install_actual_source_selection_observer)),
    );
    for (read, write, counts) in [
        (true, false, (0, 1, 1)),
        (false, true, (1, 0, 1)),
        (true, true, (1, 1, 1)),
    ] {
        ACTUAL_SELECTION_PHASES.set([0; 2]);
        run_original_owner_counts(0, counts, helper_owner(read, write, false));
        assert_eq!(ACTUAL_SELECTION_PHASES.get(), [1, 1]);
    }
}

#[test]
fn selected_source_edge_ordinals_preserve_boolean_polarity_and_assert_success() {
    let owner = selection_owner(false, false);
    let mut types = owner.source_semantic().types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(declaration(
        99,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let actual = Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(1),
        else_arguments: vec![],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    for explicit in [0, 1] {
        let original = SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(0, boolean)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    explicit,
                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
            )
            .unwrap(),
        };
        for (ordinal, role) in [
            (0, SemanticEdgeRoleV1::SwitchValue),
            (1, SemanticEdgeRoleV1::SwitchOtherwise),
        ] {
            assert_eq!(
                source_reference_selection_edge_ordinal_v30(
                    &types,
                    &original,
                    role,
                    ordinal,
                    &actual,
                    &mut budget
                )
                .unwrap(),
                if explicit == 1 { ordinal } else { 1 - ordinal }
            );
            assert_actual_selection_error(
                source_reference_selection_edge_ordinal_v30(
                    &types,
                    &original,
                    role,
                    1 - ordinal,
                    &actual,
                    &mut budget,
                )
                .map(|_| ()),
            );
        }
    }
    for expected in [false, true] {
        let original = SemanticTerminatorKindV1::Assert {
            condition: SemanticOperandV1::Copy(place(0, boolean)),
            expected,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: SemanticOperandV1::Copy(place(1, U32)),
                index: SemanticOperandV1::Copy(place(2, U32)),
            },
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
            unwind: SemanticUnwindActionV1::Unreachable,
        };
        assert_eq!(
            source_reference_selection_edge_ordinal_v30(
                &types,
                &original,
                SemanticEdgeRoleV1::AssertSuccess,
                0,
                &actual,
                &mut budget
            )
            .unwrap(),
            usize::from(!expected)
        );
        assert_actual_selection_error(
            source_reference_selection_edge_ordinal_v30(
                &types,
                &original,
                SemanticEdgeRoleV1::AssertUnwind,
                0,
                &actual,
                &mut budget,
            )
            .map(|_| ()),
        );
    }
}

#[test]
fn selected_actual_transport_headers_have_an_independent_typed_frame_oracle() {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = h::<SourceReferenceSelectionActualV30>()
        + h::<SourceReferenceSelectionActualNodeV30>()
        + h::<SourceReferenceSelectionActualEdgeV30>()
        + h::<Option<SourceReferenceSelectionActualInvocationV30>>()
        + h::<InvocationArgumentRowV1>()
        + h::<InvocationComponentRowV1>()
        + h::<ScopedEmittedPointsV29<'_, '_, '_>>()
        + h::<Option<(BlockId, u32)>>()
        + std::mem::size_of::<Result<Option<(BlockId, u32)>, ScopedTileFailureKindV29>>()
        + h::<Vec<SourceReferenceSelectionControlV30>>()
        + h::<SourceReferenceSelectionControlV30>()
        + h::<Vec<SourceReferenceSelectionIncomingV30>>()
        + h::<SourceReferenceSelectionIncomingV30>()
        + h::<(BlockId, usize, BlockId, usize)>()
        + h::<Vec<(usize, u32, usize)>>()
        + h::<(usize, u32, usize)>()
        + h::<Vec<SourceReferenceSelectionInvocationScopeV30>>()
        + h::<SourceReferenceSelectionInvocationScopeV30>()
        + h::<Vec<(BlockId, usize)>>()
        + h::<Vec<(ValueId, BlockId, usize)>>()
        + h::<Vec<usize>>()
        + h::<&SemanticValueBindingV1>()
        + h::<Option<(ScalarType, AddressSpace, AccessMode)>>()
        + h::<Option<(BlockId, usize)>>()
        + h::<&SourceIssuedActualV29<'_>>()
        + h::<&Function>()
        + h::<&Terminator>()
        + h::<Type>()
        + h::<usize>()
        + h::<bool>()
        + h::<()>()
        + std::mem::size_of::<fe2o3_kernel_ir::FunctionControlFlowParameterV1>()
        + std::mem::size_of::<fe2o3_kernel_ir::FunctionControlFlowParameterInputV1>()
        + std::mem::size_of::<
            Result<
                fe2o3_kernel_ir::FunctionControlFlowParameterV1,
                fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1,
            >,
        >()
        + std::mem::size_of::<
            Result<
                fe2o3_kernel_ir::FunctionControlFlowParameterInputV1,
                fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1,
            >,
        >()
        + std::mem::size_of::<Result<(), fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1>>();
    assert_eq!(
        source_reference_selection_actual_headers_v30().unwrap(),
        expected
    );
    for short in [false, true] {
        let limit = 17 + expected - usize::from(short);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result =
            budget.reserve_storage(source_reference_selection_actual_headers_v30().unwrap());
        if short {
            assert!(
                matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 17 + expected && error.limit() == limit)
            );
            assert_eq!(budget.failed_storage(), Some(17 + expected));
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn selected_actual_transport_has_exact_one_short_resources_and_preserves_failure_history() {
    with_plan(&selection_owner(true, true), |plan, budget| {
        let source = original_borrow(plan, plan.root, 8);
        with_source_reference_selection_v29(
            plan,
            plan.root,
            site(8),
            ExecutionOperandV29::RvaluePlace,
            source,
            budget,
            |graph, _| {
                let run = |work_limit, storage_limit| {
                    let (function, mut bound) = actual_selection_fixture(graph, true);
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
                    budget.reserve_storage(17).unwrap();
                    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
                        budget.reserve_storage(
                            source_reference_selection_actual_headers_v30()?
                                + bound.nodes.capacity()
                                    * std::mem::size_of::<SourceReferenceSelectionActualNodeV30>()
                                + bound.edges.capacity()
                                    * std::mem::size_of::<SourceReferenceSelectionActualEdgeV30>(),
                        )?;
                        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                        source_reference_selection_check_transport_v30(
                            &function, &actual, &mut bound, budget,
                        )
                    });
                    (
                        result,
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_work(),
                        budget.failed_storage(),
                    )
                };
                let (result, work, held, peak, failed_work, failed_storage) =
                    run(usize::MAX, usize::MAX);
                result.unwrap();
                assert_eq!((held, failed_work, failed_storage), (17, None, None));
                let (result, exact_work, exact_held, exact_peak, failed_work, failed_storage) =
                    run(work, peak);
                result.unwrap();
                assert_eq!(
                    (
                        exact_work,
                        exact_held,
                        exact_peak,
                        failed_work,
                        failed_storage
                    ),
                    (work, held, peak, None, None)
                );
                for short_work in [true, false] {
                    let limit_work = work - usize::from(short_work);
                    let limit_storage = peak - usize::from(!short_work);
                    let (result, _, held, _, failed_work, failed_storage) =
                        run(limit_work, limit_storage);
                    match result {
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(error),
                        )) if short_work => {
                            assert_eq!(
                                (error.actual(), error.limit(), failed_work, failed_storage),
                                (work, limit_work, Some(work), None)
                            );
                        }
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(error),
                        )) if !short_work => {
                            assert_eq!(
                                (error.actual(), error.limit(), failed_work, failed_storage),
                                (peak, limit_storage, None, Some(peak))
                            );
                        }
                        other => panic!("selected transport resource refusal: {other:?}"),
                    }
                    assert_eq!(held, 17);
                }
                Ok(())
            },
        )
        .unwrap();
    });
}
