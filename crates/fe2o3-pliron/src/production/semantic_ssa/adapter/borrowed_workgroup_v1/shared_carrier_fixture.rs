use super::*;

pub(super) const CARRIER: u32 = 10;
pub(super) const CARRIER_REF: u32 = 11;
pub(super) const CARRIER_LOCAL: u32 = 6;

fn aggregate_type(
    index: u8,
    fields: Vec<u32>,
    offsets: Vec<u64>,
    size: u64,
    align: u64,
    repr: SemanticBackendReprV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([200 + index; 32]),
        SemanticLayoutIdentityV1::from_sha256([200 + index; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(size),
            align,
            repr,
            false,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(fields.into_iter().map(ty).collect()).unwrap(),
        ),
    )
}

fn helper_abi() -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([184; 32]),
        SemanticLayoutIdentityV1::from_sha256([185; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![ty(CARRIER_REF)],
        ty(9),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(CARRIER_REF),
            SemanticAbiPassModeV1::Direct(attributes(true, false)),
        ))],
        SemanticAbiValueV1::new(
            ty(9),
            SemanticAbiPassModeV1::Direct(attributes(false, true)),
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap()
}

pub(super) fn terminal() -> SemanticCallableDeclV1 {
    let contract = SemanticExecutionCapabilityContractV1::new(
        E::SubgroupDeriveBorrowed {
            workgroup_reference: ty(5),
            workgroup: ty(4),
            subgroup: ty(9),
            width: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(5)], ty(9)).unwrap(),
        provenance(),
        SemanticTypeIdentityV1::from_sha256([166; 32]),
        SemanticTypeIdentityV1::from_sha256([167; 32]),
        None,
        SemanticFunctionIdentityV1::from_sha256([190; 32]),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            contract.source_identity(),
            SemanticItemDefinitionIdentityV1::from_sha256([190; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([190; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([190; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([190; 32]),
            source(),
            reference_abi(9),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([190; 32]),
    }
}

pub(super) fn rebuild(
    body: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let result = SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        locals,
        body.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = body.kernel_entry() {
        result.with_kernel_entry(entry.clone())
    } else {
        result
    }
}

pub(super) fn request() -> InertSemanticMirRequestV1 {
    let base = epoch_source(true);
    let mut types = base.types().to_vec();
    let scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([207; 32]),
        SemanticLayoutIdentityV1::from_sha256([207; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    types.push(aggregate_type(
        8,
        vec![7, 2, 2, 3],
        vec![0, 4, 4, 4],
        4,
        4,
        SemanticBackendReprV1::scalar(scalar),
    ));
    types.push(aggregate_type(
        9,
        vec![8, 2, 3],
        vec![0, 4, 4],
        4,
        4,
        SemanticBackendReprV1::scalar(scalar),
    ));
    types.push(aggregate_type(
        10,
        vec![5, 1],
        vec![0, 8],
        16,
        8,
        SemanticBackendReprV1::memory(true),
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([211; 32]),
            SemanticLayoutIdentityV1::from_sha256([211; 32]),
            types[5].layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(CARRIER),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(types[5].abi_properties()),
    );
    let root = &base.functions()[0];
    let mut locals = root.locals().to_vec();
    locals.extend([
        local(6, CARRIER, SemanticLocalRoleV1::Temporary),
        local(7, CARRIER_REF, SemanticLocalRoleV1::Temporary),
        local(8, 9, SemanticLocalRoleV1::Temporary),
        local(9, CARRIER_REF, SemanticLocalRoleV1::Temporary),
        local(10, 9, SemanticLocalRoleV1::Temporary),
        local(11, CARRIER, SemanticLocalRoleV1::Temporary),
    ]);
    let mut statements = root.blocks()[0].statements().to_vec();
    statements.push(assign(
        CARRIER_LOCAL,
        CARRIER,
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Aggregate,
            vec![
                SemanticOperandV1::Copy(place(2, 5)),
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty(1),
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 8).unwrap()),
                )),
            ],
        )
        .unwrap(),
    ));
    statements.push(borrow(
        7,
        CARRIER_REF,
        place(CARRIER_LOCAL, CARRIER),
        SemanticBorrowKindV1::Shared,
    ));
    let blocks = vec![
        block(
            0,
            statements,
            call(
                3,
                vec![SemanticOperandV1::Copy(place(7, CARRIER_REF))],
                place(8, 9),
                2,
            ),
        ),
        root.blocks()[1].clone(),
        block(
            2,
            vec![borrow(
                9,
                CARRIER_REF,
                place(CARRIER_LOCAL, CARRIER),
                SemanticBorrowKindV1::Shared,
            )],
            call(
                3,
                vec![SemanticOperandV1::Copy(place(9, CARRIER_REF))],
                place(10, 9),
                3,
            ),
        ),
        block(3, vec![], root.blocks()[0].terminator().kind().clone()),
    ];
    let mut functions = base.functions().to_vec();
    functions[0] = rebuild(root, locals, blocks);
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(CARRIER)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(5)).unwrap(),
        ],
        ty(5),
    )
    .unwrap();
    functions.push(function(
        184,
        helper_abi(),
        vec![
            local(0, 9, SemanticLocalRoleV1::Return),
            local(1, CARRIER_REF, SemanticLocalRoleV1::Argument(0)),
            local(2, 5, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                0,
                vec![assign(
                    2,
                    5,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
                )],
                call(
                    4,
                    vec![SemanticOperandV1::Copy(place(2, 5))],
                    place(0, 9),
                    1,
                ),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
    ));
    let mut callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect::<Vec<_>>();
    callables.push(terminal());
    InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        base.roots().to_vec(),
    )
    .unwrap()
}

pub(super) fn admitted() -> AdmittedInertSemanticMirV1 {
    request().admit_exact_v20(Default::default()).unwrap()
}

pub(super) fn original_borrows(
    body: &SemanticFunctionDeclV1,
) -> Vec<(SemanticTransparentBorrowSiteV1, &SemanticPlaceV1)> {
    body.blocks()
        .iter()
        .enumerate()
        .flat_map(|(b, block)| {
            block
                .statements()
                .iter()
                .enumerate()
                .filter_map(move |(s, statement)| {
                    let SemanticStatementKindV1::Assign(a) = statement.kind() else {
                        return None;
                    };
                    let SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    } = a.value().kind()
                    else {
                        return None;
                    };
                    (a.destination().ty() == ty(CARRIER_REF)
                        && place.ty() == ty(CARRIER)
                        && place.projections().is_empty())
                    .then_some((
                        SemanticTransparentBorrowSiteV1 {
                            block: b as u32,
                            statement: s as u32,
                        },
                        place,
                    ))
                })
        })
        .collect()
}
