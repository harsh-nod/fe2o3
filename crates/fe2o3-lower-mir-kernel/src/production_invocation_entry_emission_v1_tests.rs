fn capture_cyclic_invocation(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    capture(source, instances, emitted, slots, budget)?;
    let index = instances
        .instances()
        .iter()
        .position(|row| row.function().index() == 2)
        .unwrap();
    let instance = instances.instance(instances.id_at(index).unwrap()).unwrap();
    let original = instance.declaration();
    assert_eq!(original.entry().index(), 0);
    assert_eq!(original.blocks().len(), 3);
    assert!(matches!(
        original.blocks()[0].terminator().kind(),
        SemanticTerminatorKindV1::Call(_)
    ));
    let lowered = emitted[index].as_ref().unwrap();
    let relation = lowered
        .invocation_entry
        .as_ref()
        .expect("cyclic entry requires its own retained relation");
    assert_eq!(
        relation.subject.unwrap().instance,
        instances.id_at(index).unwrap()
    );
    assert!(relation.subject.unwrap().source == slots.source);
    let body = lowered.function.body.as_ref().unwrap();
    let preheader = relation.layout.preheader.unwrap();
    assert_eq!(preheader.0, relation.layout.first_block + 3);
    assert_eq!(body.blocks[0].id, preheader);
    assert_eq!(body.blocks[1].id, relation.layout.source_entry);
    assert!(body.blocks[0].parameters.is_empty());
    assert!(body.blocks[0].operations.is_empty());
    assert_eq!(relation.span.operation_count, 0);
    assert_eq!(relation.arguments.len(), 1);
    assert_eq!(
        relation.arguments[0].original,
        instance.ssa().plan().entry_arguments()[0]
    );
    assert_eq!(relation.arguments[0].original.variable().get(), 2);
    assert_eq!(relation.arguments[0].first_component, 0);
    assert_eq!(relation.arguments[0].component_count, 1);
    let [component] = relation.components.as_slice() else {
        panic!("one scalar entry component")
    };
    assert_eq!(component.original, body.parameters[0]);
    assert_eq!(component.parameter, body.blocks[1].parameters[0].id);
    assert_ne!(component.original, component.parameter);
    assert_eq!(component.transported, component.original);
    assert_eq!(component.conversion, None);
    assert!(matches!(&body.blocks[0].terminator,
        Some(Terminator::Branch { target, arguments })
        if *target == relation.layout.source_entry && arguments.as_slice() == [component.original]));
    assert_eq!(lowered.blocks.len(), original.blocks().len());
    assert!(
        lowered
            .blocks
            .iter()
            .all(|row| row.kernel_ir_block != preheader)
    );
    assert!(
        lowered
            .statement_operation_spans
            .iter()
            .all(|row| row.kernel_ir_block != preheader)
    );
    assert!(
        lowered
            .terminator_operation_spans
            .iter()
            .all(|row| row.kernel_ir_block != preheader)
    );
    let latch = body
        .blocks
        .iter()
        .find(|block| block.id.0 == relation.layout.first_block + 1)
        .unwrap();
    let mut backedges = 0;
    latch
        .terminator
        .as_ref()
        .unwrap()
        .try_visit_edges_v1(|target, arguments| {
            assert_ne!(target, preheader);
            if target == relation.layout.source_entry {
                backedges += 1;
                assert_eq!(arguments.len(), 1);
                assert_ne!(arguments[0], component.original);
            }
            Ok::<_, ProductionSemanticKirErrorV1>(())
        })?;
    assert_eq!(backedges, 1);
    assert_eq!(
        invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget)?,
        (1, 0)
    );
    Ok(())
}

fn reject_cyclic_invocation_faults(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    capture_cyclic_invocation(source, instances, emitted, slots, budget)?;
    let index = instances
        .instances()
        .iter()
        .position(|row| row.function().index() == 2)
        .unwrap();
    let instance = instances.instance(instances.id_at(index).unwrap()).unwrap();
    let lowered = emitted[index].as_mut().unwrap();
    let check = |lowered: &LoweredFunctionResultV1, budget: &mut ArgumentBudgetV1<'_>| {
        assert!(matches!(
            invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "invocation entry differs from its original source SSA plan",
                ..
            })
        ));
    };
    let saved = lowered.invocation_entry.take();
    check(lowered, budget);
    lowered.invocation_entry = saved;

    let original = lowered.invocation_entry.as_ref().unwrap().arguments[0];
    lowered.invocation_entry.as_mut().unwrap().arguments[0].original = SsaArgumentV1::new(
        original.original.variable(),
        SsaValueV1::BlockArgument {
            block: SsaBlockIdV1::new(0),
            variable: original.original.variable(),
        },
    );
    check(lowered, budget);
    lowered.invocation_entry.as_mut().unwrap().arguments[0] = original;
    lowered
        .invocation_entry
        .as_mut()
        .unwrap()
        .arguments
        .push(original);
    check(lowered, budget);
    lowered.invocation_entry.as_mut().unwrap().arguments.pop();

    let component = lowered.invocation_entry.as_ref().unwrap().components[0];
    lowered
        .invocation_entry
        .as_mut()
        .unwrap()
        .components
        .push(component);
    check(lowered, budget);
    lowered.invocation_entry.as_mut().unwrap().components.pop();
    lowered.invocation_entry.as_mut().unwrap().components[0].original = component.parameter;
    lowered.invocation_entry.as_mut().unwrap().components[0].transported = component.parameter;
    let Some(Terminator::Branch { arguments, .. }) =
        &mut lowered.function.body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    arguments[0] = component.parameter;
    check(lowered, budget);
    lowered.invocation_entry.as_mut().unwrap().components[0] = component;
    let Some(Terminator::Branch { arguments, .. }) =
        &mut lowered.function.body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    arguments[0] = component.transported;

    let subject = lowered.invocation_entry.as_ref().unwrap().subject.unwrap();
    lowered
        .invocation_entry
        .as_mut()
        .unwrap()
        .subject
        .as_mut()
        .unwrap()
        .instance = instances.root();
    check(lowered, budget);
    lowered.invocation_entry.as_mut().unwrap().subject = Some(subject);
    lowered
        .invocation_entry
        .as_mut()
        .unwrap()
        .span
        .operation_count = 1;
    check(lowered, budget);
    lowered
        .invocation_entry
        .as_mut()
        .unwrap()
        .span
        .operation_count = 0;

    let terminator = lowered.function.body.as_mut().unwrap().blocks[0]
        .terminator
        .take();
    check(lowered, budget);
    lowered.function.body.as_mut().unwrap().blocks[0].terminator = terminator;
    let preheader = lowered
        .invocation_entry
        .as_ref()
        .unwrap()
        .layout
        .preheader
        .unwrap();
    let header = lowered
        .invocation_entry
        .as_ref()
        .unwrap()
        .layout
        .source_entry;
    let Some(Terminator::Branch { target, .. }) =
        &mut lowered.function.body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    *target = preheader;
    check(lowered, budget);
    let Some(Terminator::Branch { target, .. }) =
        &mut lowered.function.body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    *target = header;
    lowered.function.body.as_mut().unwrap().blocks.swap(0, 1);
    check(lowered, budget);
    lowered.function.body.as_mut().unwrap().blocks.swap(0, 1);
    let block = lowered.function.body.as_ref().unwrap().blocks[0].clone();
    lowered.function.body.as_mut().unwrap().blocks.push(block);
    check(lowered, budget);
    lowered.function.body.as_mut().unwrap().blocks.pop();
    lowered.function.body.as_mut().unwrap().blocks[0]
        .parameters
        .push(ValueDef::new(
            ValueId(u32::MAX - 1),
            Type::Scalar(ScalarType::U32),
        ));
    check(lowered, budget);
    lowered.function.body.as_mut().unwrap().blocks[0]
        .parameters
        .clear();
    assert_eq!(
        invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget)?,
        (1, 0)
    );
    Ok(())
}

#[test]
fn actual_cyclic_invocation_rejects_missing_foreign_duplicated_and_reordered_receipts() {
    let result = run_suffix(
        SuffixCase::CyclicEntry,
        reject_cyclic_invocation_faults,
        LIMIT,
        LIMIT,
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 1);
}

#[test]
fn actual_cyclic_invocation_retains_exact_limits_and_cleans_up_late_failure() {
    let (result, work, storage) = run_suffix(
        SuffixCase::CyclicEntry,
        capture_cyclic_invocation,
        LIMIT,
        LIMIT,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(
        run_suffix(
            SuffixCase::CyclicEntry,
            capture_cyclic_invocation,
            work,
            storage
        )
        .0
        .is_ok()
    );
    for (work, storage, work_failure) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let result = run_suffix(
            SuffixCase::CyclicEntry,
            capture_cyclic_invocation,
            work,
            storage,
        )
        .0;
        assert_resource(
            result.as_ref().err().expect("one short must fail"),
            work_failure,
        );
    }
    fn fail(
        source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        slots: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        capture_cyclic_invocation(source, instances, emitted, slots, budget)?;
        Err(invocation_entry_error_v1())
    }
    assert!(
        run_suffix(SuffixCase::CyclicEntry, fail, LIMIT, LIMIT)
            .0
            .is_err()
    );
    assert!(
        run_suffix(
            SuffixCase::CyclicEntry,
            capture_cyclic_invocation,
            LIMIT,
            LIMIT
        )
        .0
        .is_ok()
    );
}

#[test]
fn invocation_input_map_has_independent_exact_and_one_short_scratch_limits() {
    let owner = suffix_owner(SuffixCase::CyclicEntry);
    let function = &owner.source_semantic().functions()[2];
    assert_eq!(function.locals().len(), 4);
    let inputs = [
        InvocationInputRowV1 {
            local: 1,
            ty: function.locals()[1].ty(),
            source_argument: 0,
            tuple_field: None,
            first_parameter: 0,
            parameter_count: 0,
        },
        InvocationInputRowV1 {
            local: 2,
            ty: U32,
            source_argument: 1,
            tuple_field: None,
            first_parameter: 0,
            parameter_count: 1,
        },
    ];
    // One allocation visit (3), initialization plus final local scan (2*4),
    // and five checks per original input row (5*2). No output allocation.
    let expected_work = 3 + 2 * 4 + 5 * 2;
    let bytes = std::mem::size_of::<Vec<bool>>() + 4 * std::mem::size_of::<bool>();
    for floor in [0, 73] {
        for (work_limit, storage_limit, success) in [
            (7 + expected_work, floor + bytes, true),
            (7 + expected_work - 1, floor + bytes, false),
            (7 + expected_work, floor + bytes - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(7).unwrap();
            let result = invocation_check_inputs_v1(function, &inputs, 1, &mut budget);
            assert_eq!(result.is_ok(), success, "{result:?}");
            assert_eq!(budget.storage(), floor);
            if success {
                assert_eq!(budget.peak_storage(), floor + bytes);
                assert_eq!(budget.failed_storage(), None);
            } else {
                assert_resource(result.as_ref().unwrap_err(), work_limit < 7 + expected_work);
                if storage_limit < floor + bytes {
                    assert_eq!(budget.failed_storage(), Some(floor + bytes));
                }
            }
            drop(budget);
            if success {
                assert_eq!(work.work(), 7 + expected_work);
            }
            if work_limit < 7 + expected_work {
                assert_eq!(work.failed_work(), Some(7 + expected_work));
            }
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1000);
    budget.reserve_storage(73).unwrap();
    assert!(budget.reserve_storage(2000).is_err());
    assert!(budget.charge_work(2000).is_err());
    invocation_check_inputs_v1(function, &inputs, 1, &mut budget).unwrap();
    for fault in 0..5 {
        let mut hostile = inputs;
        match fault {
            0 => hostile.swap(0, 1),
            1 => hostile[1].first_parameter = 1,
            2 => hostile[1].local = 1,
            3 => hostile[1].source_argument = 0,
            4 => hostile[1].tuple_field = Some(0),
            _ => unreachable!(),
        }
        assert!(invocation_check_inputs_v1(function, &hostile, 1, &mut budget).is_err());
        assert_eq!(budget.storage(), 73);
    }
    assert_eq!(budget.failed_storage(), Some(2073));
    drop(budget);
    assert_eq!(work.failed_work(), Some(2000));
}

#[test]
fn actual_cyclic_input_map_preserves_zero_width_nominal_parameter() {
    fn observe(
        source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        slots: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        capture_cyclic_invocation(source, instances, emitted, slots, budget)?;
        let index = instances
            .instances()
            .iter()
            .position(|row| row.function().index() == 2)
            .unwrap();
        let original = instances.instance(instances.id_at(index).unwrap()).unwrap();
        let lowered = emitted[index].as_mut().unwrap();
        let inputs = &lowered.invocation_entry.as_ref().unwrap().inputs;
        assert_eq!(inputs.len(), 2);
        assert_eq!(
            (
                inputs[0].local,
                inputs[0].first_parameter,
                inputs[0].parameter_count
            ),
            (1, 0, 0)
        );
        assert_eq!(
            (
                inputs[1].local,
                inputs[1].first_parameter,
                inputs[1].parameter_count
            ),
            (2, 0, 1)
        );
        let saved = lowered.invocation_entry.as_mut().unwrap().inputs.remove(0);
        assert!(
            invocation_checked_prefix_v1(original, slots.source.root, lowered, budget).is_err()
        );
        lowered
            .invocation_entry
            .as_mut()
            .unwrap()
            .inputs
            .insert(0, saved);
        assert_eq!(
            invocation_checked_prefix_v1(original, slots.source.root, lowered, budget)?,
            (1, 0)
        );
        Ok(())
    }
    let result = run_suffix(SuffixCase::CyclicEntry, observe, LIMIT, LIMIT).0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 1);
}

#[derive(Clone, Copy, Debug)]
enum InvocationInputCaseV1 {
    Scalars,
    Aggregate,
    Reference,
}

fn repeated_invocation_input_owner(case: InvocationInputCaseV1) -> ProductionSemanticSsaOwnerV1 {
    let original = suffix_owner(SuffixCase::Ordinary);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let input = match case {
        InvocationInputCaseV1::Scalars => U32,
        InvocationInputCaseV1::Aggregate => {
            let SemanticBackendReprV1::Scalar(scalar) =
                *types[U32.index() as usize].layout().backend_repr()
            else {
                panic!("scalar fixture");
            };
            aggregate(
                &mut types,
                vec![U32, U32],
                vec![0, 4],
                8,
                4,
                SemanticBackendReprV1::scalar_pair(scalar, scalar),
                None,
            )
        }
        InvocationInputCaseV1::Reference => {
            reference(&mut types, U32, SemanticMutabilityV1::Immutable, false)
        }
    };
    let projected = match case {
        InvocationInputCaseV1::Scalars => place(1, U32),
        InvocationInputCaseV1::Aggregate => SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32).unwrap()],
            U32,
        )
        .unwrap(),
        InvocationInputCaseV1::Reference => SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap(),
    };
    let ownership = if matches!(case, InvocationInputCaseV1::Reference) {
        SemanticSourceArgumentOwnershipV1::SharedBorrow
    } else {
        SemanticSourceArgumentOwnershipV1::ByValue
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([231; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(value_abi(&types, input)),
            SemanticAbiArgumentV1::source(direct(U32)),
        ],
        direct(U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership, SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    functions[3] = function(
        230,
        SemanticFunctionRoleV1::InternalHelper,
        abi,
        vec![
            local(232, U32, SemanticLocalRoleV1::Return),
            local(233, input, SemanticLocalRoleV1::Argument(0)),
            local(234, U32, SemanticLocalRoleV1::Argument(1)),
        ],
        vec![
            block(
                235,
                vec![
                    assign(
                        place(0, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
                    ),
                    assign(
                        place(1, input),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, input))),
                    ),
                    assign(
                        place(2, U32),
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Add,
                            left: SemanticOperandV1::Copy(place(2, U32)),
                            right: literal(1),
                        },
                    ),
                ],
                switch(SemanticOperandV1::Copy(place(2, U32)), 1, 0),
            ),
            block(236, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    if !matches!(case, InvocationInputCaseV1::Scalars) {
        let callback = &functions[2];
        let mut locals = callback.locals().to_vec();
        assert_eq!(locals.len(), 4);
        locals.push(local(237, input, SemanticLocalRoleV1::Temporary));
        let initialize = match case {
            InvocationInputCaseV1::Aggregate => SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![SemanticOperandV1::Copy(place(2, U32)), literal(31)],
            )
            .unwrap(),
            InvocationInputCaseV1::Reference => SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(2, U32),
            },
            InvocationInputCaseV1::Scalars => unreachable!(),
        };
        let call = |destination, next, control| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(3),
                    vec![SemanticOperandV1::Copy(place(4, input)), literal(control)],
                    Some(SemanticCallDestinationV1::new(
                        place(destination, U32),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(next),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        functions[2] = function(
            110,
            callback.role(),
            callback.abi().clone(),
            locals,
            vec![
                block(
                    115,
                    vec![assign(place(4, input), initialize)],
                    call(3, 1, 7),
                ),
                block(117, vec![], call(0, 2, 11)),
                block(118, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![ROOT],
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

fn check_repeated_invocation_inputs(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let indices = instances
        .instances()
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.function().index() == 3).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(indices.len(), 2);
    let first_subject = emitted[indices[0]]
        .as_ref()
        .unwrap()
        .invocation_entry
        .as_ref()
        .unwrap()
        .subject;
    let second_subject = emitted[indices[1]]
        .as_ref()
        .unwrap()
        .invocation_entry
        .as_ref()
        .unwrap()
        .subject;
    assert!(first_subject != second_subject);
    let mut first_parameters = Vec::new();
    for (ordinal, &index) in indices.iter().enumerate() {
        let instance = instances.instance(instances.id_at(index).unwrap()).unwrap();
        let lowered = emitted[index].as_mut().unwrap();
        let relation = lowered.invocation_entry.as_ref().unwrap();
        assert_eq!(relation.inputs.len(), 2);
        assert_eq!(relation.inputs[0].local, 1);
        assert_eq!(relation.inputs[1].local, 2);
        assert_eq!(relation.inputs[0].source_argument, 0);
        assert_eq!(relation.inputs[1].source_argument, 1);
        assert_eq!(relation.inputs[0].tuple_field, None);
        assert_eq!(relation.inputs[1].tuple_field, None);
        assert_eq!(relation.inputs[0].first_parameter, 0);
        assert_eq!(
            relation.inputs[1].first_parameter,
            relation.inputs[0].parameter_count
        );
        assert_eq!(relation.inputs[1].parameter_count, 1);
        let input_ty = instance.declaration().locals()[1].ty();
        let input = &instances.owner().source_semantic().types()[input_ty.index() as usize];
        match input.shape() {
            SemanticTypeShapeV1::Aggregate(_) => assert_eq!(relation.inputs[0].parameter_count, 2),
            SemanticTypeShapeV1::Pointer(_) => {
                assert_eq!(relation.inputs[0].parameter_count, 1);
                // The Rust reference is a promoted referent input, not an ABI
                // pointer guessed from equal physical types.
                assert_eq!(
                    lowered.function.signature.parameters[0],
                    Type::Scalar(ScalarType::U32)
                );
            }
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                assert_eq!(relation.inputs[0].parameter_count, 1)
            }
            _ => panic!("unexpected source input shape"),
        }
        let body = lowered.function.body.as_ref().unwrap();
        assert_eq!(
            body.parameters.len(),
            relation
                .inputs
                .iter()
                .map(|input| input.parameter_count)
                .sum::<usize>()
        );
        assert_eq!(relation.arguments.len(), 2);
        assert_eq!(relation.components.len(), body.parameters.len());
        if ordinal == 0 {
            first_parameters = body.parameters.clone();
        } else {
            assert!(
                body.parameters
                    .iter()
                    .all(|value| !first_parameters.contains(value))
            );
        }
        assert_eq!(
            invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget)?,
            (1, 0)
        );
        let original = lowered.invocation_entry.as_ref().unwrap().inputs.clone();
        for fault in 0..6 {
            let inputs = &mut lowered.invocation_entry.as_mut().unwrap().inputs;
            match fault {
                0 => inputs[0].first_parameter = 1,
                1 => inputs.swap(0, 1),
                2 => {
                    inputs.push(inputs[0]);
                }
                3 => inputs[0].parameter_count += 1,
                4 => inputs[1].local = inputs[0].local,
                5 => inputs[0].source_argument = 1,
                _ => unreachable!(),
            }
            assert!(matches!(
                invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "invocation entry differs from its original source SSA plan",
                    ..
                })
            ));
            lowered.invocation_entry.as_mut().unwrap().inputs = original.clone();
        }
        let subject = lowered.invocation_entry.as_ref().unwrap().subject;
        lowered.invocation_entry.as_mut().unwrap().subject = if ordinal == 0 {
            second_subject
        } else {
            first_subject
        };
        assert!(
            invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget).is_err()
        );
        lowered.invocation_entry.as_mut().unwrap().subject = subject;
        if ordinal == 1 {
            let component = lowered.invocation_entry.as_ref().unwrap().components[0];
            lowered.invocation_entry.as_mut().unwrap().components[0].original = first_parameters[0];
            assert!(
                invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget).is_err()
            );
            lowered.invocation_entry.as_mut().unwrap().components[0] = component;
        }
        assert_eq!(
            invocation_checked_prefix_v1(instance, slots.source.root, lowered, budget)?,
            (1, 0)
        );
    }
    REACHED.set(REACHED.get() + 1);
    Ok(())
}

#[test]
fn real_repeated_cyclic_inputs_preserve_scalar_aggregate_and_promoted_reference_installation() {
    for case in [
        InvocationInputCaseV1::Scalars,
        InvocationInputCaseV1::Aggregate,
        InvocationInputCaseV1::Reference,
    ] {
        let result = run_suffix_owner(
            || repeated_invocation_input_owner(case),
            check_repeated_invocation_inputs,
            LIMIT,
            LIMIT,
            |output, source, budget| {
                assert!(
                    output
                        .pending
                        .function
                        .body
                        .as_ref()
                        .is_some_and(|body| !body.blocks.is_empty())
                );
                let table_floor = budget.storage();
                let result = with_scoped_source_test_layouts_v29(
                    source,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                    |demands, layouts, budget| {
                        let floor = budget.storage();
                        let persistent = layouts.persistent_storage_for_test();
                        let mut replay = crate::with_checked_context_root_v29(
                            source.owner,
                            source.launch,
                            root_input(source.owner),
                            budget,
                            |checked, budget| {
                                let limits = ProductionSemanticKirLimitsV1::default();
                                Ok(emit_pending_scoped_root_v29(
                                    &checked,
                                    source,
                                    demands,
                                    layouts,
                                    limits,
                                    &mut ReachableClosureBudgetV1::new(limits.max_blocks),
                                    &mut PrivateArrayLazyBudgetV1::new(1, limits.max_operations),
                                    PrivateArrayPayloadV1 {
                                        occupied: 0,
                                        capacity: 0,
                                    },
                                    budget,
                                ))
                            },
                        )
                        .map_err(|error| match error {
                            crate::ProductionContextRootErrorV29::Resource(error) => error.into(),
                            _ => execution_lifecycle_error_v29(),
                        })??;
                        let schema_growth = layouts.persistent_storage_for_test() - persistent;
                        assert_eq!(
                            budget.storage() - floor,
                            replay.retained_emission_storage + schema_growth
                        );
                        assert_eq!(
                            output.pending.sidecars.rows.len(),
                            replay.pending.sidecars.rows.len()
                        );
                        let mut checked = 0;
                        for (original, fresh) in output
                            .pending
                            .sidecars
                            .rows
                            .iter()
                            .zip(&mut replay.pending.sidecars.rows)
                        {
                            let (Some(original), Some(fresh)) =
                                (&original.invocation_entry, &mut fresh.invocation_entry)
                            else {
                                continue;
                            };
                            assert!(scoped_replay_metadata_v29::matches_invocation_for_test_v1(
                                original, fresh, budget
                            )?);
                            let saved = fresh.inputs[0];
                            fresh.inputs[0].first_parameter += 1;
                            assert!(!scoped_replay_metadata_v29::matches_invocation_for_test_v1(
                                original, fresh, budget
                            )?);
                            fresh.inputs[0] = saved;
                            assert!(scoped_replay_metadata_v29::matches_invocation_for_test_v1(
                                original, fresh, budget
                            )?);
                            checked += 1;
                        }
                        assert_eq!(checked, 2);
                        let retained = replay.retained_emission_storage;
                        drop(replay);
                        budget.release_storage(retained)?;
                        assert_eq!(budget.storage(), floor + schema_growth);
                        Ok(())
                    },
                );
                assert_eq!(budget.storage(), table_floor);
                result
            },
        )
        .0;
        assert!(result.is_ok(), "{case:?}: {result:?}");
        assert_eq!(REACHED.get(), 2);
    }
}
