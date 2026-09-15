use super::*;
#[path = "ssa_fixture.rs"]
mod fixture;

fn setup() -> (
    Vec<SemanticTypeDeclV1>,
    MathTransportV1,
    KernelContextTypeV1,
) {
    let source = fixture::captured_source(false, false, false, false);
    let SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract },
        ..
    } = &source.callables()[7]
    else {
        panic!("fixture consumer")
    };
    let projection = SemanticProjectionV1::new(
        SemanticProjectionKindV1::Field(0),
        SemanticTypeIdV1::from_index(10),
    )
    .unwrap();
    let path = MathCapturePathV1::checked(
        source.types(),
        SemanticTypeIdV1::from_index(12),
        &[projection],
        contract.types().bound_reference,
    )
    .unwrap();
    let provenance = contract.provenance();
    // Deliberately inert component root, not a source/launch qualification.
    let context = KernelContextTypeV1::new(
        "component_math_capture",
        *provenance.kernel_marker().as_bytes(),
        *provenance.target_brand().as_bytes(),
        *provenance.launch_brand().as_bytes(),
    );
    (
        source.types().to_vec(),
        MathTransportV1 {
            contract: *contract,
            bound: true,
            capture: Some(path),
        },
        context,
    )
}

#[test]
fn capture_path_requires_exact_field_and_nominal_shared_reference() {
    let (types, transport, _) = setup();
    let carrier = transport.semantic_type();
    let reference = transport.contract.types().bound_reference;
    for (kind, result) in [
        (SemanticProjectionKindV1::Field(1), reference),
        (SemanticProjectionKindV1::Field(3), reference),
        (
            SemanticProjectionKindV1::Field(0),
            transport.contract.types().math_reference,
        ),
        (SemanticProjectionKindV1::Dereference, reference),
    ] {
        let projection = SemanticProjectionV1::new(kind, result).unwrap();
        assert!(MathCapturePathV1::checked(&types, carrier, &[projection], reference).is_err());
    }
    let projection =
        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), reference).unwrap();
    assert!(MathCapturePathV1::checked(&types, carrier, &[], reference).is_err());
    assert!(MathCapturePathV1::checked(&types, carrier, &[projection; 17], reference).is_err());
    assert!(MathCapturePathV1::checked(&types, reference, &[projection], reference).is_err());
}

#[test]
fn capture_transport_roundtrip_preserves_capability_and_ordinary_components() {
    let (types, transport, context) = setup();
    let expected = transport
        .kernel_types(&types, transport.semantic_type(), Some(&context))
        .unwrap();
    assert_eq!(expected.len(), 3);
    assert_eq!(
        &expected[1..],
        &[Type::Scalar(ScalarType::F32), Type::Scalar(ScalarType::F32)]
    );
    let values = expected
        .iter()
        .enumerate()
        .map(|(index, ty)| ValueDef::new(ValueId(71 + index as u32), ty.clone()))
        .collect::<Vec<_>>();
    let restored = transport
        .from_values(&types, transport.semantic_type(), &values, &expected)
        .unwrap();
    assert_eq!(
        transport.values(&restored, &expected).unwrap(),
        values
            .iter()
            .map(|value| (value.id, value.ty.clone()))
            .collect::<Vec<_>>()
    );
    let SemanticValueBindingV1::Aggregate(fields) = restored else {
        panic!("retained aggregate")
    };
    assert_eq!(fields.len(), 3);
    assert_eq!(
        fields[0].values().unwrap(),
        [(ValueId(71), expected[0].clone())]
    );
}

#[test]
fn capture_transport_rejects_changed_root_missing_field_and_replaced_live_value() {
    let (types, transport, context) = setup();
    let expected = transport
        .kernel_types(&types, transport.semantic_type(), Some(&context))
        .unwrap();
    let values = expected
        .iter()
        .enumerate()
        .map(|(index, ty)| ValueDef::new(ValueId(71 + index as u32), ty.clone()))
        .collect::<Vec<_>>();
    let original = transport
        .from_values(&types, transport.semantic_type(), &values, &expected)
        .unwrap();
    let SemanticValueBindingV1::Aggregate(fields) = original else {
        panic!("aggregate")
    };
    let mut changed = fields.clone();
    let SemanticValueBindingV1::Value {
        ty: Type::ExecutionCapability(capability),
        ..
    } = &mut changed[0]
    else {
        panic!("capability")
    };
    capability.provenance.root = "different_root".into();
    assert!(
        transport
            .values(&SemanticValueBindingV1::Aggregate(changed), &expected)
            .is_err()
    );
    assert!(
        transport
            .values(
                &SemanticValueBindingV1::Aggregate(fields[1..].to_vec()),
                &expected
            )
            .is_err()
    );
    let mut changed = fields.clone();
    changed[0] = SemanticValueBindingV1::Unmaterialized;
    assert!(
        transport
            .values(&SemanticValueBindingV1::Aggregate(changed), &expected)
            .is_err()
    );
    assert!(
        transport
            .from_values(&types, transport.semantic_type(), &values[..2], &expected)
            .is_err()
    );
    assert!(
        transport
            .from_values(&types, transport.contract.types().bound, &values, &expected)
            .is_err()
    );
    assert!(math_capture_binding_field(&fields[0], [0]).is_err());
}

#[test]
fn capture_descriptor_does_not_turn_other_fields_into_math_authority() {
    let (types, transport, context) = setup();
    let expected = transport
        .kernel_types(&types, transport.semantic_type(), Some(&context))
        .unwrap();
    let fields = vec![
        SemanticValueBindingV1::Value {
            id: ValueId(71),
            ty: expected[0].clone(),
        },
        SemanticValueBindingV1::Value {
            id: ValueId(72),
            ty: expected[0].clone(),
        },
        SemanticValueBindingV1::Value {
            id: ValueId(73),
            ty: expected[2].clone(),
        },
    ];
    assert!(
        transport
            .values(&SemanticValueBindingV1::Aggregate(fields), &expected)
            .is_err()
    );
}

#[test]
fn live_capture_requires_the_same_leaf_ssa_value_not_just_its_type() {
    let (types, transport, context) = setup();
    let expected = transport
        .kernel_types(&types, transport.semantic_type(), Some(&context))
        .unwrap();
    let original = SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Value {
        id: ValueId(71),
        ty: expected[0].clone(),
    }]);
    let changed = SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Value {
        id: ValueId(72),
        ty: expected[0].clone(),
    }]);
    let leaf = math_capture_binding_field(&original, [0]).unwrap();
    assert!(math_reference_binding_matches(leaf, leaf));
    assert!(!math_reference_binding_matches(
        math_capture_binding_field(&changed, [0]).unwrap(),
        leaf
    ));
    assert!(!math_reference_binding_matches(
        &SemanticValueBindingV1::Unit,
        leaf
    ));
    assert!(!math_reference_binding_matches(
        &SemanticValueBindingV1::Unmaterialized,
        leaf
    ));
    assert!(math_capture_binding_field(&SemanticValueBindingV1::Aggregate(vec![]), [0]).is_err());
}

#[test]
fn capture_structure_budget_counts_zero_sized_siblings_cumulatively() {
    let (mut types, transport, _) = setup();
    for fields_count in [MAX_SSA_VALUE_COMPONENTS_V1 - 1, MAX_SSA_VALUE_COMPONENTS_V1] {
        let mut fields = vec![SemanticTypeIdV1::from_index(0); fields_count];
        fields[0] = transport.contract.types().bound_reference;
        let mut offsets = vec![8; fields_count];
        offsets[0] = 0;
        types[12] = SemanticTypeDeclV1::new(
            types[12].identity(),
            types[12].layout_identity(),
            fe2o3_mir_model::semantic_mir_v1::SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateLayoutV1::new(offsets, vec![])
                    .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1::new(fields).unwrap(),
            ),
        );
        let result = math_capture_shape_work(&types, transport.semantic_type(), &[0]);
        if fields_count == MAX_SSA_VALUE_COMPONENTS_V1 - 1 {
            assert_eq!(result.unwrap(), MAX_SSA_VALUE_COMPONENTS_V1);
        } else {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "Math capture exceeds the existing SSA structural limit")
            );
        }
    }
}
