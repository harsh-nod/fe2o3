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

fn literal(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
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
