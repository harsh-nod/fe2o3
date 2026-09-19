fn build(
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
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

pub(in super::super) fn initialization_array_move_owner(
    projected: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let original = initialization_array_owner(true);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let array = helper.locals()[2].ty();
    let result_type = if projected { U32 } else { array };
    let mut locals = helper.locals().to_vec();
    locals.push(local(136, result_type, SemanticLocalRoleV1::Temporary));
    let mut initial = helper.blocks()[0].statements().to_vec();
    let source = if projected {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 2,
                        from_end: false,
                    },
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap()
    } else {
        place(2, array)
    };
    initial.push(assign(
        place(3, result_type),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(source)),
    ));
    let blocks = vec![
        block(140, initial, helper.blocks()[0].terminator().kind().clone()),
        block(
            141,
            vec![assign(
                place(0, U32),
                SemanticRvalueKindV1::Use(literal(99)),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    functions[3] = function(130, helper.role(), helper.abi().clone(), locals, blocks);
    build(
        semantic.types().to_vec(),
        functions,
        semantic.callables().to_vec(),
    )
}

fn literal(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

pub(in super::super) fn initialization_owner(
    config: InitializationFixtureV29,
) -> ProductionSemanticSsaOwnerV1 {
    let original = repeated_slot_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let mut locals = helper.locals().to_vec();
    locals.push(local(136, U32, SemanticLocalRoleV1::Temporary));
    locals.push(local(137, U32, SemanticLocalRoleV1::Temporary));
    let mut exit = Vec::new();
    let read_place = if config.address_read {
        assert!(!config.copy_read);
        let pointer = reference(&mut types, U32, SemanticMutabilityV1::Mutable, true);
        locals.push(local(138, pointer, SemanticLocalRoleV1::Temporary));
        exit.push(assign(
            place(5, pointer),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(2, U32),
            },
        ));
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(5),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap()
    } else {
        place(2, U32)
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    let switch = |local, left, right| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(local, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, left),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, right),
        )
        .unwrap(),
    };
    let statement =
        |kind| SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind);
    let store = || {
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(2, U32),
            literal(99),
            SemanticVolatilityV1::NonVolatile,
            None,
        )))
    };
    let mut changed = Vec::new();
    if config.looping {
        changed.push(store());
        changed.push(assign(place(3, U32), SemanticRvalueKindV1::Use(literal(1))));
    }
    if let Some(kill) = config.kill {
        if matches!(kill, InitializationKillV29::StorageDeadLive) {
            changed.push(statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )));
        }
        changed.push(match kill {
            InitializationKillV29::StorageLive | InitializationKillV29::StorageDeadLive => {
                statement(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(2),
                ))
            }
            InitializationKillV29::StorageDead => statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )),
            InitializationKillV29::Deinitialize => {
                statement(SemanticStatementKindV1::Deinitialize(place(2, U32)))
            }
            InitializationKillV29::Move => assign(
                place(4, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, U32))),
            ),
            InitializationKillV29::SelfMove => assign(
                place(2, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, U32))),
            ),
        });
    }
    if config.reinitialize {
        changed.push(store());
    }
    if config.alias_move {
        assert!(config.address_read);
        exit.push(assign(
            place(4, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(read_place.clone())),
        ));
    }
    let read = if config.copy_read {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, U32)))
    } else {
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            read_place,
            if config.volatile {
                SemanticVolatilityV1::Volatile
            } else {
                SemanticVolatilityV1::NonVolatile
            },
            None,
        ))
    };
    exit.push(assign(place(0, U32), read));
    let mut entry = helper.blocks()[0].statements()[..2].to_vec();
    if config.looping {
        entry.push(assign(place(3, U32), SemanticRvalueKindV1::Use(literal(0))));
    }
    let blocks = vec![
        block(
            140,
            entry,
            if config.looping {
                goto(1)
            } else {
                switch(1, 1, 2)
            },
        ),
        block(
            141,
            vec![],
            if config.looping {
                switch(3, 2, 3)
            } else {
                goto(3)
            },
        ),
        block(142, changed, goto(if config.looping { 1 } else { 3 })),
        block(143, exit, SemanticTerminatorKindV1::Return),
    ];
    functions[3] = function(130, helper.role(), helper.abi().clone(), locals, blocks);
    build(types, functions, semantic.callables().to_vec())
}

pub(in super::super) fn initialization_array_owner(whole: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let array = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            4,
            SemanticFieldsShapeV1::array(4, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: U32,
            length: 2,
        },
        None,
    );
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let mut locals = helper.locals().to_vec();
    locals.push(local(135, array, SemanticLocalRoleV1::Temporary));
    let element = |offset| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length: 2,
                        from_end: false,
                    },
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap()
    };
    let mut initial = vec![];
    if whole {
        initial.push(assign(
            place(2, array),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    vec![literal(11), literal(12)],
                )
                .unwrap(),
            ),
        ));
    }
    initial.push(assign(element(0), SemanticRvalueKindV1::Use(literal(99))));
    let blocks = vec![
        block(
            140,
            initial,
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(1),
            )),
        ),
        block(
            141,
            vec![assign(
                place(0, U32),
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                    element(1),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    functions[3] = function(130, helper.role(), helper.abi().clone(), locals, blocks);
    build(types, functions, semantic.callables().to_vec())
}

pub(in super::super) fn repeated_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = lifecycle_owner(false);
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    let scalar = SemanticFunctionIdV1::from_index(3);
    callables.insert(3, SemanticCallableDeclV1::defined(scalar));
    for (function_index, tag) in [(0, 80), (1, 100)] {
        let prior = &functions[function_index];
        let mut blocks = prior.blocks().to_vec();
        let SemanticTerminatorKindV1::Call(call) = blocks[0].terminator().kind() else {
            unreachable!()
        };
        let shifted = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(call.callee().index() + 1),
            call.arguments().to_vec(),
            call.destination().cloned(),
            call.unwind(),
        )
        .unwrap();
        blocks[0] = block(
            if function_index == 0 { 85 } else { 90 },
            blocks[0].statements().to_vec(),
            SemanticTerminatorKindV1::Call(shifted),
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
        functions[function_index] = replacement;
    }
    let callback = &functions[CALLBACK.index() as usize];
    let mut locals = callback.locals().to_vec();
    locals.push(local(116, U32, SemanticLocalRoleV1::Temporary));
    let call = |operand, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(3),
                vec![operand],
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
    functions[CALLBACK.index() as usize] = function(
        110,
        SemanticFunctionRoleV1::InternalHelper,
        callback.abi().clone(),
        locals,
        vec![
            block(
                115,
                vec![],
                call(SemanticOperandV1::Copy(place(2, U32)), 3, 1),
            ),
            block(
                117,
                vec![],
                call(SemanticOperandV1::Move(place(3, U32)), 0, 2),
            ),
            block(118, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([131; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct(U32))],
        direct(U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    functions.push(function(
        130,
        SemanticFunctionRoleV1::InternalHelper,
        abi,
        vec![
            local(132, U32, SemanticLocalRoleV1::Return),
            local(133, U32, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            134,
            vec![assign(
                place(0, U32),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: SemanticOperandV1::Copy(place(1, U32)),
                    right: literal(1),
                },
            )],
            SemanticTerminatorKindV1::Return,
        )],
    ));
    build(semantic.types().to_vec(), functions, callables)
}

pub(in super::super) fn array_owner(branches: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = lifecycle_owner(branches);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let array = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            4,
            4,
            SemanticFieldsShapeV1::array(4, 1),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: U32,
            length: 1,
        },
        None,
    );
    let mut functions = semantic.functions().to_vec();
    for (function_index, tag) in [(1, 100), (2, 110)] {
        let prior = &functions[function_index];
        let mut locals = prior.locals().to_vec();
        let array_local = locals.len() as u32;
        let index_local = array_local + 1;
        locals.push(local(140, array, SemanticLocalRoleV1::Temporary));
        locals.push(local(141, U32, SemanticLocalRoleV1::Temporary));
        let indexed = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(array_local),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index_local)),
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap();
        let mut blocks = prior.blocks().to_vec();
        let mut statements = vec![
            assign(
                place(index_local, U32),
                SemanticRvalueKindV1::Use(literal(0)),
            ),
            assign(
                place(array_local, array),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        vec![literal(11)],
                    )
                    .unwrap(),
                ),
            ),
            assign(indexed, SemanticRvalueKindV1::Use(literal(99))),
        ];
        statements.extend_from_slice(blocks[0].statements());
        blocks[0] = block(
            if function_index == 1 { 90 } else { 115 },
            statements,
            blocks[0].terminator().kind().clone(),
        );
        functions[function_index] =
            function(tag, prior.role(), prior.abi().clone(), locals, blocks);
    }
    build(types, functions, semantic.callables().to_vec())
}

pub(in super::super) fn repeated_slot_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = repeated_owner();
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let mut locals = helper.locals().to_vec();
    locals.push(local(135, U32, SemanticLocalRoleV1::Temporary));
    let mut statements = helper.blocks()[0].statements().to_vec();
    statements.push(SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(2, U32),
            SemanticOperandV1::Copy(place(0, U32)),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    statements.push(assign(
        place(0, U32),
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            place(2, U32),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    functions[3] = function(
        130,
        helper.role(),
        helper.abi().clone(),
        locals,
        vec![block(134, statements, SemanticTerminatorKindV1::Return)],
    );
    build(
        semantic.types().to_vec(),
        functions,
        semantic.callables().to_vec(),
    )
}
