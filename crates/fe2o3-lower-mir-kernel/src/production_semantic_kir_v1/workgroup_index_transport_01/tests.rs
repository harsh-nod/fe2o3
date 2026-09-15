fn fixture() -> (
    WorkgroupIndexTransportV1,
    Vec<SemanticTypeDeclV1>,
    KernelContextTypeV1,
) {
    let id = |tag| SemanticTypeIdentityV1::from_sha256([tag; 32]);
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([1; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
        id(3),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
    )
    .unwrap();
    // An inert type-table slot tests transport custody only. The registered
    // source callback separately exercises the actual rustc witness layout.
    let types = vec![SemanticTypeDeclV1::new(
        id(9),
        SemanticLayoutIdentityV1::from_sha256([9; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )];
    let transport = WorkgroupIndexTransportV1 {
        semantic: SemanticTypeIdV1::from_index(0),
        source: ExecutionTypeIdentityV1::new([9; 32]),
        provenance,
        brand: id(7),
        epoch: id(8),
    };
    (
        transport,
        types,
        KernelContextTypeV1::new("root", [3; 32], [4; 32], [5; 32]),
    )
}

#[test]
fn scoped_index_transport_round_trip_retains_existing_ssa_and_exact_root() {
    let (transport, types, context) = fixture();
    let expected = transport
        .types(&types, transport.semantic, Some(&context))
        .unwrap();
    let values = vec![ValueDef::new(ValueId(17), expected[0].clone())];
    let descriptor =
        SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::WorkgroupIndex(transport));
    let binding = descriptor
        .binding_from_transport(&types, transport.semantic, &values, &expected)
        .unwrap();
    assert_eq!(
        descriptor.transport_values(&binding, &expected).unwrap(),
        vec![(ValueId(17), expected[0].clone())]
    );
    assert!(transport.types(&types, transport.semantic, None).is_err());
    for mutate in [
        (|c: &mut ExecutionCapabilityTypeV1| c.provenance.root = FunctionId::new("other"))
            as fn(&mut ExecutionCapabilityTypeV1),
        |c| c.provenance.issuance = [77; 32],
        |c| c.provenance.target_brand = [77; 32],
        |c| c.workgroup_brand = Some([77; 32]),
        |c| c.epoch = Some([77; 32]),
        |c| c.source_type = ExecutionTypeIdentityV1::new([77; 32]),
        |c| c.role = ExecutionCapabilityRoleV1::Workgroup,
    ] {
        let Type::ExecutionCapability(mut wrong) = expected[0].clone() else {
            unreachable!()
        };
        mutate(&mut wrong);
        let wrong = Type::ExecutionCapability(wrong);
        let binding = SemanticValueBindingV1::Value {
            id: ValueId(17),
            ty: wrong.clone(),
        };
        assert!(descriptor.transport_values(&binding, &expected).is_err());
        assert!(
            descriptor
                .binding_from_transport(
                    &types,
                    transport.semantic,
                    &[ValueDef::new(ValueId(17), wrong)],
                    &expected
                )
                .is_err()
        );
    }
}

#[test]
fn scoped_index_enum_custody_requires_exact_variant_and_live_typed_payload() {
    let (transport, types, context) = fixture();
    let ty = transport
        .types(&types, transport.semantic, Some(&context))
        .unwrap()
        .remove(0);
    let payload = SemanticValueBindingV1::Value { id: ValueId(1), ty };
    let wrap = |binding, variant| SemanticValueBindingV1::Enum {
        discriminant: ValueId(2),
        discriminant_ty: Type::Scalar(ScalarType::U32),
        semantic_type: SemanticTypeIdV1::from_index(1),
        variant,
        payloads: BTreeMap::from([(1, vec![binding])]),
    };
    let exact = wrap(payload.clone(), Some(1));
    assert!(exact_workgroup_index_enum_custody_v1(&exact));
    assert!(exact_workgroup_index_enum_custody_v1(&wrap(exact, Some(1))));
    assert!(!exact_workgroup_index_enum_custody_v1(&wrap(
        payload.clone(),
        None
    )));
    assert!(!exact_workgroup_index_enum_custody_v1(&wrap(
        payload,
        Some(0)
    )));
    assert!(!exact_workgroup_index_enum_custody_v1(&wrap(
        SemanticValueBindingV1::Unmaterialized,
        Some(1)
    )));
    assert!(!exact_workgroup_index_enum_custody_v1(&wrap(
        SemanticValueBindingV1::Value {
            id: ValueId(1),
            ty: Type::INDEX
        },
        Some(1)
    )));
    let mut deep = SemanticValueBindingV1::Unit;
    for _ in 0..66 {
        deep = wrap(deep, Some(1));
    }
    assert!(!exact_workgroup_index_enum_custody_v1(&deep));
}
