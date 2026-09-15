use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

pub(super) const PAIR: u32 = 1;
pub(super) const VALUE: u32 = 2;
pub(super) const ENUM_COPY: u32 = 3;
pub(super) const PAIR_COPY: u32 = 4;

pub(super) fn whole(local: u32) -> SemanticPlaceV1 {
    test_typed_place(
        local,
        match local {
            PAIR | PAIR_COPY => 3,
            VALUE => 1,
            ENUM_COPY => 2,
            _ => panic!("closed fixture local roster"),
        },
    )
}

pub(super) fn enumeration(field: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(PAIR),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(field),
                SemanticTypeIdV1::from_index(2),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(2),
    )
    .unwrap()
}

pub(super) fn payload(field: u32) -> SemanticPlaceV1 {
    let mut projections = enumeration(field).projections().to_vec();
    projections.push(
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::Downcast(1),
            SemanticTypeIdV1::from_index(2),
        )
        .unwrap(),
    );
    projections.push(
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::Field(0),
            SemanticTypeIdV1::from_index(1),
        )
        .unwrap(),
    );
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(PAIR),
        projections,
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap()
}

pub(super) fn scalar(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

pub(super) fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

pub(super) fn value(
    destination: SemanticPlaceV1,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    let ty = destination.ty();
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(ty, kind),
    )))
}

pub(super) fn construct(destination: SemanticPlaceV1) -> SemanticStatementV1 {
    value(
        destination,
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::EnumVariant(1),
                vec![scalar(7)],
            )
            .unwrap(),
        ),
    )
}

pub(super) fn initialized() -> Vec<SemanticStatementV1> {
    vec![
        construct(whole(ENUM_COPY)),
        value(
            whole(PAIR),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(whole(ENUM_COPY)),
                        SemanticOperandV1::Move(whole(ENUM_COPY)),
                    ],
                )
                .unwrap(),
            ),
        ),
    ]
}

pub(super) fn moved(field: u32) -> SemanticStatementV1 {
    test_assign_to(whole(VALUE), SemanticOperandV1::Move(payload(field)))
}

pub(super) fn tag(field: u32) -> SemanticStatementV1 {
    value(
        whole(VALUE),
        SemanticRvalueKindV1::Discriminant(enumeration(field)),
    )
}

pub(super) fn types(overlap: bool) -> Vec<SemanticTypeDeclV1> {
    let mut types = admitted_single_function_semantic().types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(160)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(161)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    let offset = if overlap { 0 } else { 4 };
    let variants = (0..2)
        .map(|index| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                8,
                4,
                SemanticFieldsShapeV1::arbitrary(vec![offset], vec![0]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                100 + u64::from(index),
                SemanticAggregateLayoutV1::new(vec![offset], vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(162)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(163)),
        SemanticTypeLayoutV1::enum_layout(
            8,
            4,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            SemanticTypeIdV1::from_index(1),
            (0..2)
                .map(|index| {
                    SemanticEnumVariantV1::new(
                        index,
                        SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(1)])
                            .unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(164)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(165)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(2); 2]).unwrap(),
        ),
    ));
    types
}

pub(super) fn request(
    blocks: Vec<SemanticBasicBlockV1>,
    overlap: bool,
) -> InertSemanticMirRequestV1 {
    let base = admitted_single_function_semantic();
    let original = &base.functions()[0];
    let mut locals = original.locals().to_vec();
    for (id, ty) in [3, 1, 2, 3].into_iter().enumerate() {
        locals.push(test_local(
            145 + id as u8,
            ty,
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new(
        base.target(),
        types(overlap),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

pub(super) fn admit(blocks: Vec<SemanticBasicBlockV1>) -> AdmittedInertSemanticMirV1 {
    request(blocks, false)
        .admit_current_production(SemanticMirLimitsV1::default())
        .expect("the exact enum layout, typed places, ABI and sorted fixture identities must admit")
}

pub(super) fn linear(statements: Vec<SemanticStatementV1>) -> AdmittedInertSemanticMirV1 {
    admit(vec![test_block(
        170,
        statements,
        SemanticTerminatorKindV1::Return,
    )])
}

pub(super) fn plan(
    mir: &AdmittedInertSemanticMirV1,
) -> Result<ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaErrorV1> {
    plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &mir.functions()[0],
        mir.types(),
        mir.callables(),
        ProductionSemanticSsaLimitsV1::default(),
    )
}
