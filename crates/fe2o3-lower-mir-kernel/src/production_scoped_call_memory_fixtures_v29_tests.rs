pub(in super::super) fn call_destinations_owner(
    projected: bool,
    retained_address: bool,
    indexed: bool,
) -> ProductionSemanticSsaOwnerV1 {
    assert!(!retained_address || projected);
    assert!(!indexed || !projected);
    let original = repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    let callback = &functions[CALLBACK.index() as usize];
    let mut locals = callback.locals().to_vec();
    let store = |id, ty, value| {
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(id, ty),
                value,
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )
    };
    let mut setup = vec![store(0, U32, literal(0)), store(3, U32, literal(0))];
    let destination = if indexed {
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
        locals.push(local(145, array, SemanticLocalRoleV1::Temporary));
        locals.push(local(148, U32, SemanticLocalRoleV1::Temporary));
        setup.push(store(5, U32, literal(0)));
        setup.push(assign(
            place(4, array),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Array, vec![literal(0)])
                    .unwrap(),
            ),
        ));
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(5)),
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap()
    } else if projected {
        let pointer = reference(&mut types, U32, SemanticMutabilityV1::Mutable, true);
        assert_eq!(locals.len(), 4);
        locals.push(local(145, pointer, SemanticLocalRoleV1::Temporary));
        setup.push(assign(
            place(4, pointer),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(3, U32),
            },
        ));
        if retained_address {
            setup.push(store(
                4,
                pointer,
                SemanticOperandV1::Copy(place(4, pointer)),
            ));
        }
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap()
    } else {
        place(3, U32)
    };
    let mut blocks = callback.blocks().to_vec();
    let SemanticTerminatorKindV1::Call(first) = blocks[0].terminator().kind() else {
        unreachable!();
    };
    let first = SemanticDirectCallV1::new_callable(
        first.callee(),
        first.arguments().to_vec(),
        Some(SemanticCallDestinationV1::new(
            destination,
            first.destination().unwrap().edge(),
        )),
        first.unwind(),
    )
    .unwrap();
    blocks[0] = block(115, setup, SemanticTerminatorKindV1::Call(first));
    let intrinsic = SemanticCallableIdV1::from_index(callables.len() as u32);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([146; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        direct(U32),
    )
    .unwrap();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([146; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([146; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([146; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([146; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([146; 32]),
            source(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([146; 32]),
    });
    blocks[2] = block(
        118,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                intrinsic,
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(3, U32),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(3),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    blocks.push(block(147, vec![], SemanticTerminatorKindV1::Return));
    functions[CALLBACK.index() as usize] =
        function(110, callback.role(), callback.abi().clone(), locals, blocks);
    build(types, functions, callables)
}
