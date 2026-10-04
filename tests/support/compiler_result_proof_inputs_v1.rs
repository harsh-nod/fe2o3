#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum ResultWrapperMutationV1 {
    None,
    Computes,
    CrossCall,
    CrossTailCall,
}

#[allow(dead_code, clippy::too_many_lines)]
pub(crate) fn wrapped_result_owner_v1(
    seed: u8,
    mutation: ResultWrapperMutationV1,
) -> ProductionSemanticMirOwnerV1 {
    let base = ordinary_aggregate_result_owner_v1(seed, 2);
    let semantic = base.semantic();
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let result = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    let variant = |index| {
        SemanticEnumVariantLayoutV1::from_rustc(
            index,
            8,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
        )
        .unwrap()
    };
    let mut types = semantic.types().to_vec();
    types[2] = SemanticTypeDeclV1::new(
        types[2].identity(),
        types[2].layout_identity(),
        SemanticTypeLayoutV1::enum_layout(
            8,
            4,
            SemanticEnumLayoutV1::new(
                vec![variant(0), variant(1)],
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            u32_ty,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![unit]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![u32_ty]).unwrap()),
            ],
        )
        .unwrap(),
    );
    let old = &semantic.functions()[0];
    let abi = SemanticFunctionAbiV1::from_rustc(
        old.abi().identity(),
        old.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(
            result,
            SemanticAbiPassModeV1::Cast {
                pad_i32: false,
                cast: SemanticAbiCastV1::new(
                    [None; 8],
                    None,
                    SemanticAbiUniformV1::new(
                        SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                        8,
                    )
                    .unwrap(),
                    SemanticAbiValueAttributesV1::plain(),
                ),
            },
        ),
    )
    .unwrap();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let unit_value = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            unit,
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    let returned = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(0, result),
            SemanticRvalueV1::new(
                result,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::EnumVariant(0),
                    vec![unit_value()],
                )
                .unwrap(),
            ),
        )),
    );
    let call = |callee, destination| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(callee),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(destination, result),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let rebuild = |old: &SemanticFunctionDeclV1, identity, abi, blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(identity, seed)),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            abi,
            old.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let body_blocks = vec![block(
        214,
        seed,
        vec![returned],
        SemanticTerminatorKindV1::Return,
    )];
    let mut first_body = body_blocks.clone();
    if matches!(mutation, ResultWrapperMutationV1::CrossCall) {
        first_body = vec![
            block(214, seed, vec![], call(2, 0)),
            block(215, seed, vec![], SemanticTerminatorKindV1::Return),
        ];
    } else if matches!(mutation, ResultWrapperMutationV1::CrossTailCall) {
        first_body = vec![block(
            214,
            seed,
            vec![],
            SemanticTerminatorKindV1::TailCall(
                SemanticDirectTailCallV1::new(
                    SemanticFunctionIdV1::from_index(2),
                    vec![],
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        )];
    }
    let mut statements = vec![];
    if matches!(mutation, ResultWrapperMutationV1::Computes) {
        statements.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(0, unit),
                SemanticRvalueV1::new(unit, SemanticRvalueKindV1::Use(unit_value())),
            )),
        ));
    }
    let root = &semantic.functions()[1];
    let wrapper_blocks = |callee, statements| {
        vec![
            block(10, seed, statements, call(callee, 1)),
            block(11, seed, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let mut functions = vec![
        rebuild(old, 1, abi.clone(), first_body),
        rebuild(root, 2, root.abi().clone(), wrapper_blocks(0, statements))
            .with_kernel_entry(root.kernel_entry().unwrap().clone()),
    ];
    let mut roots = vec![SemanticFunctionIdV1::from_index(1)];
    if matches!(
        mutation,
        ResultWrapperMutationV1::CrossCall | ResultWrapperMutationV1::CrossTailCall
    ) {
        functions.push(rebuild(old, 3, abi, body_blocks));
        let entry = SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(format!("proof_result_second_{seed}").into_bytes()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(bytes(218, seed)),
            root.kernel_entry().unwrap().source_contract(),
        );
        functions.push(
            rebuild(root, 4, root.abi().clone(), wrapper_blocks(2, vec![]))
                .with_kernel_entry(entry),
        );
        roots.push(SemanticFunctionIdV1::from_index(3));
    }
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        roots,
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}
