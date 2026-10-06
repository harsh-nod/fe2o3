#[derive(Clone, Copy, Debug)]
enum LayoutReaderCase {
    Straight,
    Branch,
    Loop,
}

fn mixed_instance_layout_owner(
    case: LayoutReaderCase,
    reverse: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let original = scoped_root_tests::fixtures::repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let reference_type = reference(&mut types, U32, SemanticMutabilityV1::Mutable, false);
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    assert!(callables[..4].iter().enumerate().all(|(index, row)|
        matches!(row, SemanticCallableDeclV1::Defined { function } if function.index() as usize == index)));
    callables.insert(
        4,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(4)),
    );
    callables.insert(
        5,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(5)),
    );
    // The original provider/derive callable identities are preserved. Only their
    // indices move when two genuine ordinary definitions enter the roster.
    for (ordinal, tag) in [(0, 80), (1, 100)] {
        let prior = &functions[ordinal];
        let mut blocks = prior.blocks().to_vec();
        let SemanticTerminatorKindV1::Call(call) = blocks[0].terminator().kind() else {
            unreachable!()
        };
        assert!(call.callee().index() >= 4);
        blocks[0] = block(
            if ordinal == 0 { 85 } else { 90 },
            blocks[0].statements().to_vec(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(call.callee().index() + 2),
                    call.arguments().to_vec(),
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        );
        let mut replacement = function(
            tag,
            prior.role(),
            prior.abi().clone(),
            prior.locals().to_vec(),
            blocks,
        );
        if let Some(entry) = prior.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        functions[ordinal] = replacement;
    }
    let make_call = |callee, argument, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                vec![argument],
                Some(SemanticCallDestinationV1::new(
                    place(destination, U32),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let prior = &functions[CALLBACK.index() as usize];
    functions[CALLBACK.index() as usize] = function(
        110,
        prior.role(),
        prior.abi().clone(),
        prior.locals().to_vec(),
        vec![
            block(
                115,
                vec![],
                make_call(
                    if reverse { 5 } else { 3 },
                    SemanticOperandV1::Copy(place(2, U32)),
                    3,
                    1,
                ),
            ),
            block(
                117,
                vec![],
                make_call(
                    if reverse { 3 } else { 5 },
                    SemanticOperandV1::Move(place(3, U32)),
                    0,
                    2,
                ),
            ),
            block(118, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let dereference = |local| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap()
    };
    let worker_abi = functions[3].abi().clone();
    let worker = |tag, mutates| {
        let mut statements = vec![assign(
            place(2, reference_type),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(1, U32),
            },
        )];
        if mutates {
            statements.push(store(dereference(2), literal(91)));
        }
        function(
            tag,
            SemanticFunctionRoleV1::InternalHelper,
            worker_abi.clone(),
            vec![
                local(tag + 2, U32, SemanticLocalRoleV1::Return),
                local(tag + 3, U32, SemanticLocalRoleV1::Argument(0)),
                local(tag + 4, reference_type, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    tag + 5,
                    statements,
                    make_call(4, SemanticOperandV1::Move(place(2, reference_type)), 0, 1),
                ),
                block(tag + 6, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
    };
    functions[3] = worker(130, false);
    let reader_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([141; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(value_abi(
            &types,
            reference_type,
        ))],
        direct(U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow])
    .unwrap();
    let mut reader_locals = vec![
        local(142, U32, SemanticLocalRoleV1::Return),
        local(143, reference_type, SemanticLocalRoleV1::Argument(0)),
    ];
    let read = || {
        assign(
            place(0, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference(1))),
        )
    };
    let goto = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    let reader_blocks = match case {
        LayoutReaderCase::Straight => {
            vec![block(145, vec![read()], SemanticTerminatorKindV1::Return)]
        }
        LayoutReaderCase::Branch => vec![
            block(
                145,
                vec![read()],
                switch(SemanticOperandV1::Copy(place(0, U32)), 1, 2),
            ),
            block(146, vec![], SemanticTerminatorKindV1::Return),
            block(147, vec![], SemanticTerminatorKindV1::Return),
        ],
        LayoutReaderCase::Loop => {
            reader_locals.push(local(144, U32, SemanticLocalRoleV1::Temporary));
            vec![
                block(
                    145,
                    vec![assign(place(2, U32), SemanticRvalueKindV1::Use(literal(1)))],
                    goto(1),
                ),
                block(
                    146,
                    vec![read()],
                    switch(SemanticOperandV1::Copy(place(2, U32)), 3, 2),
                ),
                block(
                    147,
                    vec![assign(place(2, U32), SemanticRvalueKindV1::Use(literal(0)))],
                    goto(1),
                ),
                block(148, vec![], SemanticTerminatorKindV1::Return),
            ]
        }
    };
    functions.push(function(
        140,
        SemanticFunctionRoleV1::InternalHelper,
        reader_abi,
        reader_locals,
        reader_blocks,
    ));
    functions.push(worker(150, true));
    assert_eq!(functions.len(), 6);
    assert_eq!(functions[3].abi(), functions[5].abi());
    for index in [0, 1] {
        assert_eq!(functions[index].abi(), semantic.functions()[index].abi());
        assert_eq!(
            functions[index].identity(),
            semantic.functions()[index].identity()
        );
    }
    scoped_root_tests::fixtures::build(types, functions, callables)
}

thread_local! {
    static INSTANCE_LAYOUT_EMITTED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

fn observe_instance_layout_emission(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(instances.instances().len(), 7);
    assert_eq!(emitted.len(), 7);
    let mut promoted = 0;
    let mut addressable = 0;
    let mut original_objects = BTreeSet::new();
    for ordinal in 0..emitted.len() {
        let id = instances.id_at(ordinal).unwrap();
        let original = instances.instance(id).unwrap();
        let output = emitted[ordinal].as_ref().unwrap();
        assert_eq!(output.source_call_instance, Some(id));
        assert_eq!(instances.instance_reachable(id), Some(true));
        if original.function().index() != 4 {
            continue;
        }
        let incoming = instances.incoming(id).unwrap();
        assert_eq!(incoming.child(), Some(id));
        let caller = incoming.occurrence().caller;
        let worker = instances.instance(caller).unwrap();
        assert!(original_objects.insert((caller.index(), 1u32)));
        assert_eq!(incoming.source().arguments().len(), 1);
        assert_eq!(original.declaration().abi().source_input_types().len(), 1);
        let caller_output = emitted[caller.index()].as_ref().unwrap();
        let calls: Vec<_> = caller_output
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::Call { callee, arguments } if callee == &output.function.id => {
                    Some((operation, arguments))
                }
                _ => None,
            })
            .collect();
        assert_eq!(calls.len(), 1);
        let (call, arguments) = calls[0];
        assert_eq!(arguments.len(), 1);
        assert_eq!(call.results.len(), 1);
        assert_eq!(call.results[0].ty, Type::Scalar(ScalarType::U32));
        assert_eq!(
            output.function.signature.results,
            [Type::Scalar(ScalarType::U32)]
        );
        let expected = if worker.function().index() == 3 {
            promoted += 1;
            Type::Scalar(ScalarType::U32)
        } else {
            assert_eq!(worker.function().index(), 5);
            addressable += 1;
            let slot = caller_output
                .scoped_slot_origins
                .as_ref()
                .unwrap()
                .iter()
                .find(|slot| slot.legacy_local().unwrap() == 1)
                .unwrap();
            assert_eq!(arguments[0], slot.pointer);
            assert!(caller_output.function.body.as_ref().unwrap().blocks.iter()
                .flat_map(|block| &block.operations).any(|operation|
                    matches!(operation.kind, OperationKind::Store { pointer, .. } if pointer == slot.pointer)));
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )
        };
        assert_eq!(output.function.signature.parameters, [expected.clone()]);
        assert!(
            caller_output
                .execution_observation
                .as_ref()
                .unwrap()
                .bindings
                .values()
                .any(
                    |binding| matches!(binding, SemanticValueBindingV1::SourceReference(reference)
                if reference.values.len() == 1 && reference.values[0].id == arguments[0]
                && reference.values[0].ty == expected)
                )
        );
        let archive = output.execution_observation.as_ref().unwrap();
        for entry in original.ssa().plan().entry_arguments() {
            archive.lookup_original_v29(instances, id, entry.value(), budget)?;
        }
    }
    assert_eq!(
        original_objects.len(),
        2,
        "distinct invocations never merge the two referents"
    );
    assert_eq!((promoted, addressable), (1, 1));
    INSTANCE_LAYOUT_EMITTED.set((promoted, addressable));
    Ok(())
}

#[test]
fn actual_root_repeated_helper_uses_original_instance_layouts_across_branches_and_loops() {
    for case in [
        LayoutReaderCase::Straight,
        LayoutReaderCase::Branch,
        LayoutReaderCase::Loop,
    ] {
        for reverse in [false, true] {
            INSTANCE_LAYOUT_EMITTED.set((0, 0));
            let mut completed = false;
            let (result, _, _) = run_suffix_owner(
                || mixed_instance_layout_owner(case, reverse),
                observe_instance_layout_emission,
                LIMIT,
                LIMIT,
                |output, _, _| {
                    let pending = &output.pending;
                    assert_eq!(pending.coordinates.sources.rows.len(), 7);
                    assert_eq!(pending.sidecars.rows.len(), 7);
                    assert_eq!(pending.coordinates.seeds.rows.len(), 7);
                    assert!(
                        pending
                            .coordinates
                            .anchors
                            .rows
                            .iter()
                            .all(|row| row.removed)
                    );
                    for row in &pending.sidecars.rows {
                        let id = row.source_call_instance.unwrap();
                        assert_eq!(pending.coordinates.sources.rows[id.index()].instance, id);
                        assert!(row.execution_observation.is_some());
                    }
                    completed = true;
                    Ok(())
                },
            );
            assert!(result.is_ok(), "{case:?}, reverse={reverse}: {result:?}");
            assert!(completed);
            assert_eq!(INSTANCE_LAYOUT_EMITTED.get(), (1, 1));
        }
    }
}
