#[allow(
    dead_code,
    reason = "shared support includes consumers without parameter-shape tests"
)]
pub(crate) fn ordinary_aggregate_parameter_owner_v1(seed: u8) -> ProductionSemanticMirOwnerV1 {
    let original = ordinary_aggregate_result_owner_v1(seed, 1);
    let semantic = original.semantic();
    let helper = &semantic.functions()[0];
    let root = &semantic.functions()[1];
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let wrapper = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let place =
        |id, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(id), vec![], ty).unwrap();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag, seed)),
            ty,
            role,
            source,
        )
    };
    let assign = |destination, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let rebuild = |old: &SemanticFunctionDeclV1, abi, locals, blocks| {
        SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let mode = helper.abi().return_value().mode().clone();
    let abi = SemanticFunctionAbiV1::from_rustc(
        helper.abi().identity(),
        helper.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            wrapper,
            mode.clone(),
        ))],
        SemanticAbiValueV1::new(scalar, mode),
    )
    .unwrap();
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), scalar).unwrap()],
        scalar,
    )
    .unwrap();
    let helper = rebuild(
        helper,
        abi,
        vec![
            local(213, scalar, SemanticLocalRoleV1::Return),
            local(214, wrapper, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            214,
            seed,
            vec![assign(
                place(0, scalar),
                scalar,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let root = rebuild(
        root,
        root.abi().clone(),
        vec![
            local(213, unit, SemanticLocalRoleV1::Return),
            local(214, wrapper, SemanticLocalRoleV1::Temporary),
            local(215, scalar, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                214,
                seed,
                vec![assign(
                    place(1, wrapper),
                    wrapper,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![
                                SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    scalar,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(9, 4).unwrap(),
                                    ),
                                )),
                                SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    unit,
                                    SemanticConstantValueV1::ZeroSized,
                                )),
                            ],
                        )
                        .unwrap(),
                    ),
                )],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![SemanticOperandV1::Move(place(1, wrapper))],
                        Some(SemanticCallDestinationV1::new(
                            place(2, scalar),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(215, seed, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![helper, root],
        vec![SemanticFunctionIdV1::from_index(1)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}
