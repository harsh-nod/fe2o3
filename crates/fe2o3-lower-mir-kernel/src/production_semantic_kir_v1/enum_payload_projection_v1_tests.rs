#[test]
fn semantic_result_projection_selects_only_the_downcast_variant_payload() {
    let ok_view = SemanticValueBindingV1::Value {
        id: ValueId(17),
        ty: Type::INDEX,
    };
    let error = SemanticValueBindingV1::Value {
        id: ValueId(18),
        ty: Type::Scalar(ScalarType::U32),
    };
    let payloads = BTreeMap::from([(0, vec![ok_view]), (1, vec![error])]);

    assert!(matches!(
        project_enum_payload_field(0, payloads.clone(), 0),
        Ok(SemanticValueBindingV1::Value {
            id: ValueId(17),
            ..
        })
    ));
    assert!(matches!(
        project_enum_payload_field(1, payloads.clone(), 0),
        Ok(SemanticValueBindingV1::Value {
            id: ValueId(18),
            ..
        })
    ));
    let unavailable = project_enum_payload_field(2, payloads, 0).unwrap();
    assert!(matches!(
        unavailable,
        SemanticValueBindingV1::Unmaterialized
    ));
    assert_eq!(
        unavailable.value().unwrap_err(),
        "unmaterialized enum payload has no ordinary SSA representation"
    );
}
