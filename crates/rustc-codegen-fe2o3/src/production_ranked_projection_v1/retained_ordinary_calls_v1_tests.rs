// Included inside ordinary_helper_effect_only_tests_v1. Source construction,
// materialization, binder, optimizer and output view are all genuine here.

fn ordinary_call_coordinate_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    caller: usize,
    target: usize,
) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
        CanonicalKirOperationCoordinateV1 as Operation,
    };
    let module = view.output().module();
    module.functions[caller].body.as_ref().unwrap().blocks.iter().enumerate()
        .flat_map(|(block, body)| body.operations.iter().enumerate().map(move |(op, value)| (block, op, value)))
        .find_map(|(block, operation, value)| matches!(&value.kind,
            fe2o3_kernel_ir::OperationKind::Call { callee, .. } if *callee == module.functions[target].id)
            .then_some(Operation { block: Block { function: Function(caller as u32), block: block as u32 }, operation: operation as u32 }))
        .unwrap()
}

fn ordinary_source_owner_v1(shape: ResultShape, body: BodyShape) -> ProductionPreRankedKirOwnerV1 {
    materialize_ranked_fixture_v1(
        source_fixture(shape, body),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap()
}

#[test]
fn actual_ordinary_helpers_enter_checked_o_census_without_erasing_calls() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for (shape, body) in [
            (ResultShape::Array, BodyShape::Straight),
            (ResultShape::Tuple, BodyShape::Straight),
            (ResultShape::UnitTuple, BodyShape::Straight),
            (ResultShape::Array, BodyShape::Join),
            (ResultShape::Array, BodyShape::Loop),
        ] {
            let source = ordinary_source_owner_v1(shape, body);
            with_actual(&source, profile, |bound, checked, budget| {
                let floor = budget.storage();
                with_projected_checked_output_roots_v1(
                    &source, bound, checked, profile, &[ranked_root_input_1d(A_NAME, 247, 1)],
                    &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                    budget, |roots, view, budget| {
                        assert_eq!(roots.len(), 1);
                        assert!(std::ptr::eq(view.output(), checked.owner()));
                        let components = if matches!(shape, ResultShape::UnitTuple) { 1 } else { 2 };
                        for (caller, target) in [(0, 1), (1, 2)] {
                            let coordinate = ordinary_call_coordinate_v1(view, caller, target);
                            let floor = budget.storage();
                            let binding = view.retained_ordinary_call_bindings_v1(
                                ROOT, SemanticFunctionIdV1::from_index(caller as u32), coordinate, budget).unwrap().unwrap();
                            assert_eq!(binding.diagnostic.callee.index(), target as u32);
                            assert_eq!(binding.diagnostic.arguments, 0);
                            assert_eq!(binding.diagnostic.results, components);
                            assert_eq!(binding.results.len(), components);
                            assert!(binding.diagnostic.return_occurrences > 0);
                            for (slot, result) in binding.results.iter().enumerate() {
                                budget.charge_work(2).unwrap();
                                assert_eq!(result.output, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                                    operation: coordinate, result: slot as u32,
                                });
                            }
                            assert_eq!(budget.storage(), floor);
                        }
                        let trap_count = view.output().module().functions[0].body.as_ref().unwrap().blocks.iter()
                            .flat_map(|block| &block.operations).filter(|operation| matches!(&operation.kind,
                                fe2o3_kernel_ir::OperationKind::Call { callee, arguments }
                                if fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                                    == Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap))).count();
                        assert_eq!(trap_count, 1);
                        // The mandatory census ran before this callback. Calls,
                        // helper bodies and the real bounds-failure trap remain.
                        Ok(())
                    },
                ).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

fn ordinary_binary_call_v1(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: u32,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                whole(destination, RESULT),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn ordinary_rebuild_v1(
    old: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let function = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        abi,
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = old.kernel_entry() {
        function.with_kernel_entry(entry.clone())
    } else {
        function
    }
}

fn ordinary_shared_parameter_source_v1() -> ProductionPreRankedKirOwnerV1 {
    let seed = source_fixture(ResultShape::Array, BodyShape::Straight);
    let source = seed.source_semantic();
    let mut functions = source.functions().to_vec();
    let root = &functions[0];
    let mut root_locals = root.locals().to_vec();
    let first_result = root_locals.len() as u32 - 1;
    let second_result = root_locals.len() as u32;
    root_locals.push(local(115, RESULT, SemanticLocalRoleV1::Temporary));
    let mut root_blocks = root.blocks().to_vec();
    assert_eq!(root_blocks.len(), 3);
    root_blocks[0] = block(
        201,
        vec![],
        ordinary_binary_call_v1(
            1,
            vec![typed_operand(3, A_U32), typed_constant(A_U32, 19, 4)],
            first_result,
            3,
        ),
    );
    root_blocks.push(block(
        204,
        vec![],
        ordinary_binary_call_v1(
            2,
            vec![typed_constant(A_U32, 5, 4), typed_operand(3, A_U32)],
            second_result,
            2,
        ),
    ));
    functions[0] = ordinary_rebuild_v1(root, root.abi().clone(), root_locals, root_blocks);
    for function in &mut functions[1..] {
        let tag = if function.identity() == source.functions()[1].identity() {
            248
        } else {
            249
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            2,
            (0..2)
                .map(|_| {
                    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        A_U32,
                        SemanticAbiPassModeV1::Direct(initialized_attributes()),
                    ))
                })
                .collect(),
            SemanticAbiValueV1::new(RESULT, function.abi().return_value().mode().clone()),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
        .unwrap();
        let mut locals = vec![
            local(120, RESULT, SemanticLocalRoleV1::Return),
            local(121, A_U32, SemanticLocalRoleV1::Argument(0)),
            local(122, A_U32, SemanticLocalRoleV1::Argument(1)),
        ];
        let blocks = if tag == 248 {
            locals.push(local(123, RESULT, SemanticLocalRoleV1::Temporary));
            vec![
                block(
                    130,
                    vec![],
                    ordinary_binary_call_v1(
                        2,
                        vec![typed_operand(1, A_U32), typed_operand(2, A_U32)],
                        3,
                        1,
                    ),
                ),
                block(
                    131,
                    vec![],
                    ordinary_binary_call_v1(
                        2,
                        vec![typed_operand(2, A_U32), typed_operand(1, A_U32)],
                        0,
                        2,
                    ),
                ),
                block(132, vec![], SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![block(
                130,
                vec![typed_assignment(
                    0,
                    RESULT,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Array,
                            vec![typed_operand(1, A_U32), typed_operand(2, A_U32)],
                        )
                        .unwrap(),
                    ),
                )],
                SemanticTerminatorKindV1::Return,
            )]
        };
        *function = ordinary_rebuild_v1(function, abi, locals, blocks);
    }
    materialize_ranked_fixture_v1(
        assertion_ssa_functions(source.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap()
}

#[test]
fn admitted_unspecified_helper_ownership_refuses_summary_and_normal_projection() {
    let seed = ordinary_shared_parameter_source_v1();
    let semantic = seed.semantic_ssa().source_semantic();
    let admitted = derive_defined_callable_empty_effect_summaries_v1(
        semantic.types(),
        semantic.functions(),
        semantic.callables(),
    )
    .unwrap();
    for function in [1, 2] {
        assert!(admitted.is_exact_empty(SemanticFunctionIdV1::from_index(function)));
        assert_eq!(
            semantic.functions()[function as usize]
                .abi()
                .source_argument_ownership(),
            &[SemanticSourceArgumentOwnershipV1::ByValue; 2]
        );
    }

    let mut functions = semantic.functions().to_vec();
    let leaf = &functions[2];
    functions[2] = ordinary_rebuild_v1(
        leaf,
        leaf.abi()
            .clone()
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::Unspecified; 2])
            .unwrap(),
        leaf.locals().to_vec(),
        leaf.blocks().to_vec(),
    );
    // Fresh source admission and real SSA construction retain the missing
    // ownership evidence; no malformed KIR or substituted owner is involved.
    let ssa = assertion_ssa_functions(semantic.types().to_vec(), functions);
    let semantic = ssa.source_semantic();
    assert_eq!(
        semantic.functions()[2].abi().source_argument_ownership(),
        &[SemanticSourceArgumentOwnershipV1::Unspecified; 2]
    );
    let mut work = 0;
    let mut edges = 0;
    assert!(
        ordinary_direct_defined_callable_summary_v1(
            semantic.types(),
            &semantic.functions()[2],
            semantic.functions().len(),
            semantic.callables(),
            &mut edges,
            &mut work,
        )
        .unwrap()
        .is_none()
    );
    let summaries = derive_defined_callable_empty_effect_summaries_v1(
        semantic.types(),
        semantic.functions(),
        semantic.callables(),
    )
    .unwrap();
    for function in [1, 2] {
        assert!(!summaries.is_exact_empty(SemanticFunctionIdV1::from_index(function)));
    }
    let source =
        materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 1)]).unwrap();
    assert!(source.executable().module().functions[0].body.as_ref().unwrap()
        .blocks.iter().flat_map(|block| &block.operations).any(|operation|
            matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Call { callee, .. }
                if *callee == source.executable().module().functions[1].id)));
    assert!(matches!(
        project_and_verify_ranked_materialized_semantic_mir_v1(
            source,
            &[ranked_root_input_1d(A_NAME, 247, 1)],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        ),
        Err(
            ProductionRankedProjectionErrorV1::UnresolvedCallableEffect {
                block: 0,
                callee: 1,
                tail: false,
                ..
            }
        )
    ));
}

#[test]
fn actual_shared_nested_and_repeated_calls_keep_distinct_parameter_bindings() {
    let source = ordinary_shared_parameter_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            with_projected_checked_output_roots_v1(&source, bound, checked, profile,
                &[ranked_root_input_1d(A_NAME, 247, 1)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(), budget,
                |_, view, budget| {
                    let mut calls = 0;
                    for (caller, function) in view.output().module().functions.iter().enumerate() {
                        let Some(body) = &function.body else { continue; };
                        for (block, body) in body.blocks.iter().enumerate() {
                            for (operation, op) in body.operations.iter().enumerate() {
                                if !matches!(op.kind, fe2o3_kernel_ir::OperationKind::Call { .. }) { continue; }
                                let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(caller as u32), block: block as u32 },
                                    operation: operation as u32,
                                };
                                let Some(binding) = view.retained_ordinary_call_bindings_v1(ROOT,
                                    SemanticFunctionIdV1::from_index(caller as u32), coordinate, budget).unwrap() else { continue; };
                                calls += 1;
                                assert_eq!(binding.arguments.len(), 2);
                                assert_eq!(binding.results.len(), 2);
                                assert_ne!(binding.arguments[0].actual, binding.arguments[1].actual);
                                for (slot, argument) in binding.arguments.iter().enumerate() {
                                    budget.charge_work(3).unwrap();
                                    assert_eq!(argument.formal, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument {
                                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(binding.diagnostic.callee.index()), argument: slot as u32,
                                    });
                                    assert_eq!(argument.scalar, fe2o3_kernel_ir::ScalarType::U32);
                                }
                            }
                        }
                    }
                    assert_eq!(calls, 4);
                    Ok(())
                }).unwrap();
        });
    }
}

#[test]
fn actual_call_source_alias_and_noncall_coordinate_are_not_substitutable() {
    let source = ordinary_source_owner_v1(ResultShape::Array, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_global_view(&source, profile, |view, budget| {
            let call = ordinary_call_coordinate_v1(view, 0, 1);
            for (owner, function) in [(1, 0), (0, 1), (99, 99)] {
                let floor = budget.storage();
                assert!(
                    view.retained_ordinary_call_v1(
                        SemanticFunctionIdV1::from_index(owner),
                        SemanticFunctionIdV1::from_index(function),
                        call,
                        budget
                    )
                    .is_err()
                );
                assert_eq!(budget.storage(), floor);
            }
            let mut missing = call;
            missing.operation = u32::MAX;
            assert!(
                view.retained_ordinary_call_v1(ROOT, ROOT, missing, budget)
                    .is_err()
            );
            assert!(
                view.retained_ordinary_call_v1(ROOT, ROOT, call, budget)
                    .unwrap()
                    .is_some()
            );
        });
    }
}

#[test]
fn actual_two_return_helper_keeps_every_ordered_return_binding() {
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
        CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirUseCoordinateV1 as Use,
        ScalarType, Terminator,
    };
    let seed = ordinary_shared_parameter_source_v1();
    let semantic = seed.semantic_ssa().source_semantic();
    let mut functions = semantic.functions().to_vec();
    let leaf = &functions[2];
    let result = |first, second| {
        typed_assignment(
            0,
            RESULT,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    vec![typed_operand(first, A_U32), typed_operand(second, A_U32)],
                )
                .unwrap(),
            ),
        )
    };
    functions[2] = ordinary_rebuild_v1(
        leaf,
        leaf.abi().clone(),
        leaf.locals().to_vec(),
        vec![
            block(130, vec![], zero_switch(1, A_U32, 1, 2)),
            block(131, vec![result(1, 2)], SemanticTerminatorKindV1::Return),
            block(132, vec![result(2, 1)], SemanticTerminatorKindV1::Return),
        ],
    );
    let source = materialize_ranked_fixture_v1(
        assertion_ssa_functions(semantic.types().to_vec(), functions),
        &[ranked_root_input_1d(A_NAME, 247, 1)],
    )
    .unwrap();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            with_projected_checked_output_roots_v1(
                &source,
                bound,
                checked,
                profile,
                &[ranked_root_input_1d(A_NAME, 247, 1)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |_, view, budget| {
                    let call = ordinary_call_coordinate_v1(view, 1, 2);
                    let binding = view
                        .retained_ordinary_call_bindings_v1(
                            ROOT,
                            SemanticFunctionIdV1::from_index(1),
                            call,
                            budget,
                        )
                        .unwrap()
                        .unwrap();
                    assert_eq!(binding.diagnostic.return_occurrences, 2);
                    assert_eq!(binding.diagnostic.return_values, 4);
                    assert_eq!(binding.returns.len(), 4);
                    let input = bound.module().functions[2].body.as_ref().unwrap();
                    let output = view.output().module().functions[2].body.as_ref().unwrap();
                    let mut cursor = 0;
                    let mut returned = Vec::new();
                    for (ordinal, block) in output.blocks.iter().enumerate() {
                        let Some(Terminator::Return { values }) = &block.terminator else {
                            continue;
                        };
                        budget.charge_work(3 + 5 * values.len()).unwrap();
                        assert_eq!(values.len(), 2);
                        returned.push(values.clone());
                        let original_block = input
                            .blocks
                            .iter()
                            .position(|old| old.id == block.id)
                            .unwrap();
                        let coordinate = Block {
                            function: Function(2),
                            block: ordinal as u32,
                        };
                        for (slot, value) in values.iter().enumerate() {
                            let row = &binding.returns[cursor];
                            assert_eq!(row.block, coordinate);
                            let argument = output
                                .parameters
                                .iter()
                                .position(|parameter| parameter == value)
                                .unwrap();
                            assert_eq!(
                                row.value,
                                Some((
                                    slot as u32,
                                    Use::TerminatorOperand {
                                        block: Block {
                                            function: Function(2),
                                            block: original_block as u32
                                        },
                                        operand: slot as u32,
                                    },
                                    Use::TerminatorOperand {
                                        block: coordinate,
                                        operand: slot as u32
                                    },
                                    Definition::FunctionArgument {
                                        function: Function(2),
                                        argument: argument as u32
                                    },
                                    ScalarType::U32,
                                ))
                            );
                            cursor += 1;
                        }
                    }
                    assert_eq!(cursor, 4);
                    assert_eq!(returned.len(), 2);
                    assert_eq!(returned[0][0], returned[1][1]);
                    assert_eq!(returned[0][1], returned[1][0]);
                    assert_ne!(returned[0][0], returned[0][1]);
                    Ok(())
                },
            )
            .unwrap();
        });
    }
}

#[test]
fn actual_retained_call_queries_share_independently_derived_37_74_boundaries() {
    let source = ordinary_source_owner_v1(ResultShape::Array, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_global_view(&source, profile, |view, inherited| {
            let call = ordinary_call_coordinate_v1(view, 0, 1);
            let calls = view
                .output()
                .module()
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks)
                .flat_map(|b| &b.operations)
                .filter(|op| matches!(op.kind, fe2o3_kernel_ir::OperationKind::Call { .. }))
                .count();
            assert_eq!(calls, 3); // root ordinary, root trap, forwarding ordinary.
            // Root Call is first of three O Calls, requiring two 1+3 searches.
            // Its alias is first of two source Defined aliases: two 1+5 searches.
            // Entry/floor6 + call lookup8 + flags4 + callee2 + alias12 + result5.
            const QUERY: usize = 6 + 8 + 4 + 2 + 12 + 5;
            let floor = inherited.storage();
            for repeats in [1, 2] {
                for under in [false, true] {
                    let total = QUERY * repeats;
                    let mut work = Work::new(7 + total - usize::from(under));
                    {
                        let mut query = Budget::new(&mut work, floor);
                        query.charge_work(7).unwrap();
                        query.reserve_storage(floor).unwrap();
                        for ordinal in 0..repeats {
                            let result =
                                view.retained_ordinary_call_v1(ROOT, ROOT, call, &mut query);
                            assert_eq!(result.is_err(), under && ordinal + 1 == repeats);
                        }
                        assert_eq!(query.storage(), floor);
                        assert_eq!(query.peak_storage(), floor);
                        assert_eq!(query.work(), 7 + total - if under { 5 } else { 0 });
                        if under {
                            assert!(
                                view.retained_ordinary_call_v1(ROOT, ROOT, call, &mut query)
                                    .is_err()
                            );
                            assert_eq!(query.work(), 7 + total - 5);
                        }
                    }
                    assert_eq!(work.failed_work(), under.then_some(7 + total));
                }
            }
        });
    }
}

#[test]
fn changed_actual_o_calls_and_returns_cannot_borrow_the_original_transition() {
    use fe2o3_kernel_ir::{
        AddressSpace, MemoryAccess, Operation, OperationKind, ScalarType, Terminator, Type,
        ValueDef, ValueId,
    };
    let source = ordinary_shared_parameter_source_v1();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            for mutation in 0..5 {
                let floor = budget.storage();
                let mut changed = checked.owner().module().clone();
                let leaf_id = changed.functions[2].id.clone();
                if mutation < 2 {
                    let call = changed.functions[0]
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.operations)
                        .find(|op| {
                            matches!(&op.kind,
                            OperationKind::Call { arguments, .. } if arguments.len() == 2)
                        })
                        .unwrap();
                    let OperationKind::Call { callee, arguments } = &mut call.kind else {
                        unreachable!()
                    };
                    if mutation == 0 {
                        *callee = leaf_id;
                    } else {
                        arguments.swap(0, 1);
                    }
                } else if mutation == 2 {
                    let body = changed.functions[2].body.as_mut().unwrap();
                    let block = body
                        .blocks
                        .iter_mut()
                        .find(|block| matches!(block.terminator, Some(Terminator::Return { .. })))
                        .unwrap();
                    let Some(Terminator::Return { values }) = &mut block.terminator else {
                        unreachable!()
                    };
                    values.swap(0, 1);
                } else {
                    let body = changed.functions[1].body.as_mut().unwrap();
                    let next = body
                        .parameters
                        .iter()
                        .chain(
                            body.blocks
                                .iter()
                                .flat_map(|block| &block.parameters)
                                .map(|def| &def.id),
                        )
                        .chain(
                            body.blocks
                                .iter()
                                .flat_map(|block| &block.operations)
                                .flat_map(|op| &op.results)
                                .map(|def| &def.id),
                        )
                        .map(|value| value.0)
                        .max()
                        .unwrap()
                        + 1;
                    if mutation == 3 {
                        let mut duplicate = body.blocks[0]
                            .operations
                            .iter()
                            .find(|op| matches!(op.kind, OperationKind::Call { .. }))
                            .unwrap()
                            .clone();
                        for (slot, result) in duplicate.results.iter_mut().enumerate() {
                            result.id = ValueId(next + slot as u32);
                        }
                        body.blocks[0].operations.push(duplicate);
                    } else {
                        let pointer = ValueId(next);
                        body.blocks[0].operations.push(Operation::new(
                            vec![ValueDef::new(
                                pointer,
                                Type::pointer(
                                    Type::Scalar(ScalarType::U32),
                                    AddressSpace::Private,
                                    fe2o3_kernel_ir::AccessMode::ReadWrite,
                                ),
                            )],
                            OperationKind::Alloca {
                                element: Type::Scalar(ScalarType::U32),
                                count: None,
                                address_space: AddressSpace::Private,
                                alignment: 4,
                            },
                        ));
                        body.blocks[0].operations.push(Operation::new(
                            vec![],
                            OperationKind::Store {
                                pointer,
                                value: body.parameters[0],
                                access: MemoryAccess::new(AddressSpace::Private, 4),
                            },
                        ));
                    }
                }
                let (changed, changed_storage) =
                    Owner::from_module_ref_with_verification_budget_v12(&changed, budget).unwrap();
                budget
                    .reserve_storage(changed_storage.retained_storage())
                    .unwrap();
                let (input, input_storage) =
                    fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(bound, budget).unwrap();
                budget
                    .reserve_storage(input_storage.retained_storage())
                    .unwrap();
                let (output, output_storage) =
                    fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&changed, budget)
                        .unwrap();
                budget
                    .reserve_storage(output_storage.retained_storage())
                    .unwrap();
                let check_floor = budget.storage();
                assert!(
                    fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                        &input,
                        &output,
                        checked.occurrences().candidate(),
                        budget
                    )
                    .is_err(),
                    "mutation {mutation}"
                );
                assert_eq!(budget.storage(), check_floor);
                drop(output);
                budget
                    .release_storage(output_storage.retained_storage())
                    .unwrap();
                drop(input);
                budget
                    .release_storage(input_storage.retained_storage())
                    .unwrap();
                drop(changed);
                budget
                    .release_storage(changed_storage.retained_storage())
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            }
        });
    }
}

#[test]
fn actual_unit_helper_without_return_is_not_reported_as_a_returning_call() {
    for divergent in [false, true] {
        let seed = source_fixture(ResultShape::Array, BodyShape::Straight);
        let semantic = seed.source_semantic();
        let mut functions = semantic.functions().to_vec();
        let mut root_blocks = functions[0].blocks().to_vec();
        root_blocks[0] = block(
            201,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new(
                    SemanticFunctionIdV1::from_index(1),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        whole(0, A_UNIT),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, 2),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        );
        functions[0] = ordinary_rebuild_v1(
            &functions[0],
            functions[0].abi().clone(),
            functions[0].locals().to_vec(),
            root_blocks,
        );
        for index in [1, 2] {
            let old = &functions[index];
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256(bytes(247 + index as u8)),
                SemanticLayoutIdentityV1::from_sha256(bytes(250)),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                0,
                vec![],
                SemanticAbiValueV1::new(A_UNIT, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap();
            let blocks = if index == 1 {
                vec![
                    block(
                        130,
                        vec![],
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new(
                                SemanticFunctionIdV1::from_index(2),
                                vec![],
                                Some(SemanticCallDestinationV1::new(
                                    whole(0, A_UNIT),
                                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                                )),
                                SemanticUnwindActionV1::Unreachable,
                            )
                            .unwrap(),
                        ),
                    ),
                    block(131, vec![], SemanticTerminatorKindV1::Return),
                ]
            } else {
                vec![block(
                    130,
                    vec![],
                    if divergent {
                        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 0))
                    } else {
                        SemanticTerminatorKindV1::Return
                    },
                )]
            };
            functions[index] = ordinary_rebuild_v1(
                old,
                abi,
                vec![local(120, A_UNIT, SemanticLocalRoleV1::Return)],
                blocks,
            );
        }
        let source = materialize_ranked_fixture_v1(
            assertion_ssa_functions(semantic.types().to_vec(), functions),
            &[ranked_root_input_1d(A_NAME, 247, 1)],
        )
        .unwrap();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_global_view(&source, profile, |view, budget| {
                let coordinate = ordinary_call_coordinate_v1(view, 1, 2);
                let binding = view
                    .retained_ordinary_call_bindings_v1(
                        ROOT,
                        SemanticFunctionIdV1::from_index(1),
                        coordinate,
                        budget,
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(binding.diagnostic.results, 0);
                assert_eq!(
                    binding.diagnostic.return_occurrences,
                    usize::from(!divergent)
                );
                assert_eq!(binding.diagnostic.return_values, 0);
                assert_eq!(binding.returns.len(), usize::from(!divergent));
                if divergent {
                    assert!(view.output().module().functions[2].body.as_ref().unwrap().blocks.iter()
                        .any(|block| matches!(block.terminator, Some(fe2o3_kernel_ir::Terminator::Branch { target, .. }) if target == block.id)));
                }
                // This is a genuine source/O call diagnostic, not proof of
                // termination or permission to erase the caller's execution.
            });
        }
    }
}

#[test]
fn a_different_admitted_source_cannot_supply_retained_call_evidence() {
    let source = ordinary_source_owner_v1(ResultShape::Array, BodyShape::Straight);
    let changed_source = ordinary_source_owner_v1(ResultShape::Array, BodyShape::Join);
    assert_ne!(
        source.executable().canonical().canonical_bytes(),
        changed_source.executable().canonical().canonical_bytes()
    );
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            let floor = budget.storage();
            let changed_bytes = changed_source.executable_storage().retained_storage()
                + changed_source.assert_origin_storage().payload_storage();
            budget.reserve_storage(changed_bytes).unwrap();
            let (coordinates, storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    bound,
                    profile,
                    budget,
                )
                .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let reserved = budget.storage();
            assert!(matches!(
                derive_source_output_occurrences_v1(&changed_source, &coordinates, checked, budget),
                Err(ProductionSourceOutputErrorV1::InputCustody)
            ));
            assert_eq!(budget.storage(), reserved);
            drop(coordinates);
            budget.release_storage(storage.retained_storage()).unwrap();
            budget.release_storage(changed_bytes).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}
