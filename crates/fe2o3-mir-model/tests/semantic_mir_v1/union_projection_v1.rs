use super::*;

const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const WIDE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const UNION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

fn field(index: u32, ty: SemanticTypeIdV1) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()
}

fn fixture(
    input: SemanticTypeIdV1,
    projections: Vec<SemanticProjectionV1>,
    union_fields: u64,
) -> InertSemanticMirRequestV1 {
    let pair = SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, WORD]).unwrap()),
    );
    let union = SemanticTypeDeclV1::new(
        type_identity(4),
        layout_identity(4),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            8,
            SemanticFieldsShapeV1::union(union_fields).unwrap(),
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
        SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![WORD, WIDE, PAIR]).unwrap()),
    );
    let mut types = vec![u32_type(1), u64_type(2), pair, union];
    if input == POINTER {
        types.push(pointer_kind_type(
            5,
            UNION,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            0,
        ));
    }
    let result = projections.last().unwrap().result_type();
    let direct = SemanticAbiPassModeV1::Direct(noundef_attributes(SemanticAbiExtensionV1::None));
    let argument = if input == UNION {
        SemanticAbiPassModeV1::cast(
            false,
            SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                    8,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        )
    } else {
        direct.clone()
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(input, argument)],
        SemanticAbiValueV1::new(result, direct),
    )
    .unwrap();
    let input_place =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, result).unwrap();
    let destination =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], result).unwrap();
    request(
        types,
        vec![],
        vec![function(
            1,
            abi,
            vec![
                local(1, result, SemanticLocalRoleV1::Return),
                local(2, input, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![block(
                1,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        destination,
                        SemanticRvalueV1::new(
                            result,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(input_place)),
                        ),
                    )),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    )
}

fn projection_refusal(request: InertSemanticMirRequestV1) {
    let result = request.admit(SemanticMirLimitsV1::default());
    assert!(
        matches!(result, Err(SemanticMirErrorV1::InvalidTypeOperation {
        operation: SemanticTypeOperationV1::Projection,
        location: SemanticMirLocationV1::Statement { function, block, statement: 0 },
    }) if function == SemanticFunctionIdV1::from_index(0)
        && block == SemanticBlockIdV1::from_index(0)),
        "{result:?}"
    );
}

#[test]
fn semantic_union_field_projection_accepts_each_exact_source_field_type() {
    for (index, ty) in [(0, WORD), (1, WIDE)] {
        let admitted = fixture(UNION, vec![field(index, ty)], 3)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        assert!(matches!(
            admitted.types()[UNION.index() as usize].layout().fields(),
            SemanticFieldsShapeV1::Union { field_count: 3 }
        ));
        let decoded = AdmittedInertSemanticMirV1::decode_minimal_compatible_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    }
}

#[test]
fn semantic_union_field_projection_rejects_out_of_range_ordinal() {
    for index in [3, u32::MAX] {
        projection_refusal(fixture(UNION, vec![field(index, WORD)], 3));
    }
}

#[test]
fn semantic_union_field_projection_rejects_wrong_result_type() {
    for (index, expected, actual) in [(0, WORD, WIDE), (1, WIDE, WORD)] {
        let result =
            fixture(UNION, vec![field(index, actual)], 3).admit(SemanticMirLimitsV1::default());
        assert!(
            matches!(result, Err(SemanticMirErrorV1::TypeMismatch {
            expected: found_expected, actual: found_actual,
            location: SemanticMirLocationV1::Statement { function, block, statement: 0 },
        }) if found_expected == expected && found_actual == actual
            && function == SemanticFunctionIdV1::from_index(0)
            && block == SemanticBlockIdV1::from_index(0)),
            "{result:?}"
        );
    }
}

#[test]
fn semantic_union_field_projection_composes_with_dereference_and_tuple_fields() {
    let path = vec![
        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, UNION).unwrap(),
        field(2, PAIR),
        field(1, WORD),
    ];
    fixture(POINTER, path, 3)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    fixture(UNION, vec![field(2, PAIR), field(0, WORD)], 3)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    projection_refusal(fixture(UNION, vec![field(2, PAIR), field(2, WORD)], 3));
    projection_refusal(fixture(WORD, vec![field(0, WORD)], 3));
}

#[test]
fn semantic_union_field_projection_keeps_physical_layout_cardinality_validation() {
    let result = fixture(UNION, vec![field(0, WORD)], 2).admit(SemanticMirLimitsV1::default());
    assert!(matches!(result, Err(SemanticMirErrorV1::InvalidTypeLayout)));
}

#[test]
fn semantic_union_field_projection_pays_original_exact_projection_count() {
    let limits = |count| {
        SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Projections, count)
            .unwrap()
    };
    let request = || fixture(UNION, vec![field(2, PAIR), field(1, WORD)], 3);
    request().admit(limits(2)).unwrap();
    let result = request().admit(limits(1));
    assert!(matches!(
        result,
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Projections,
            actual: 2,
            max: 1,
        })
    ));
}
