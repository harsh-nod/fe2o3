use super::*;

// These descriptors test private type helpers, not source admission or owner custody.
fn fact_declaration(
    tag: u32,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    let mut identity = [0; 32];
    identity[..4].copy_from_slice(&tag.to_le_bytes());
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(identity),
        SemanticLayoutIdentityV1::from_sha256(identity),
        layout,
        shape,
    )
}

fn fact_scalar(uninhabited: bool, validity: bool) -> SemanticTypeDeclV1 {
    let scalar = SemanticScalarTypeV1::Integer {
        signed: false,
        bits: 32,
    };
    fact_declaration(
        300,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::memory(true),
            uninhabited,
        )
        .unwrap(),
        if validity {
            SemanticTypeShapeV1::ValidityScalar(
                SemanticValidityScalarTypeV1::new(
                    scalar,
                    vec![SemanticScalarValidityRangeV1::new(1, u32::MAX.into())],
                )
                .unwrap(),
            )
        } else {
            SemanticTypeShapeV1::Scalar(scalar)
        },
    )
}

fn fact_wrapper(
    tag: u32,
    field: SemanticTypeIdV1,
    size: u64,
    alignment: u64,
    uninhabited: bool,
) -> SemanticTypeDeclV1 {
    fact_declaration(
        tag,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(size),
            alignment,
            SemanticBackendReprV1::memory(true),
            uninhabited,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![field]).unwrap()),
    )
}

fn fact_pointer(
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    address_space: u32,
    width: u16,
    metadata: SemanticPointerMetadataV1,
    layout: SemanticTypeLayoutV1,
) -> SemanticTypeDeclV1 {
    fact_declaration(
        900,
        layout,
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                mutability,
                address_space,
                width,
                metadata,
            )
            .unwrap(),
        ),
    )
}

fn fact_chain(wrappers: u32) -> (Vec<SemanticTypeDeclV1>, SemanticTypeIdV1) {
    let mut declarations = vec![fact_scalar(false, false)];
    let mut pointee = SemanticTypeIdV1::from_index(0);
    for ordinal in 0..wrappers {
        declarations.push(fact_wrapper(400 + ordinal, pointee, 4, 4, false));
        pointee = SemanticTypeIdV1::from_index(ordinal + 1);
    }
    let pointer = SemanticTypeIdV1::from_index(declarations.len() as u32);
    declarations.push(fact_pointer(
        pointee,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        1,
        64,
        SemanticPointerMetadataV1::None,
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
    ));
    (declarations, pointer)
}

fn charged_slot(
    declarations: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<PrivateRetainedSlotFactsV1> {
    let mut work = PrivateArrayRecorderBudgetV1::new(1, 10_000).unwrap();
    private_retained_slot_facts_v1(declarations, ty, &mut work).unwrap()
}

fn assert_slot_equivalent(
    declarations: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    expected: Option<(Type, u32)>,
) {
    let facts = charged_slot(declarations, ty)
        .map(|facts| (facts.element.into_owned_type(), facts.alignment));
    assert_eq!(facts, expected);
    assert_eq!(retained_local_slot_type_v1(declarations, ty), expected);
}

fn assert_memory_equivalent(
    declarations: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    expected: Option<ScalarType>,
) {
    let mut work = PrivateArrayRecorderBudgetV1::new(1, 10_000).unwrap();
    assert_eq!(
        private_memory_scalar_fact_v1(declarations, ty, &mut work).unwrap(),
        expected
    );
    // This unchanged allocating helper remains an independent accepted-set oracle.
    match (lower_memory_element_type(declarations, ty), expected) {
        (Ok(actual), Some(scalar)) => assert_eq!(actual, Type::Scalar(scalar)),
        (Err(ProductionSemanticKirErrorV1::ScalarTypeUnavailable { semantic_type, .. }), None) => {
            assert_eq!(semantic_type, ty.index())
        }
        (actual, expected) => panic!("memory result {actual:?}, expected {expected:?}"),
    }
}

#[test]
fn private_array_facts_match_owned_scalar_validity_and_thin_pointer_types() {
    let zero = SemanticTypeIdV1::from_index(0);
    for validity in [false, true] {
        let declarations = vec![fact_scalar(false, validity)];
        assert_slot_equivalent(
            &declarations,
            zero,
            Some((Type::Scalar(ScalarType::U32), 4)),
        );
        assert_memory_equivalent(&declarations, zero, Some(ScalarType::U32));
    }
    for kind in [SemanticPointerKindV1::Raw, SemanticPointerKindV1::Reference] {
        for (mutability, access) in [
            (SemanticMutabilityV1::Immutable, AccessMode::ReadOnly),
            (SemanticMutabilityV1::Mutable, AccessMode::ReadWrite),
        ] {
            for (space, lowered) in [
                (0, AddressSpace::Global),
                (1, AddressSpace::Global),
                (3, AddressSpace::Workgroup),
                (4, AddressSpace::Constant),
                (5, AddressSpace::Private),
            ] {
                let declarations = vec![
                    fact_scalar(false, false),
                    fact_pointer(
                        zero,
                        kind,
                        mutability,
                        space,
                        64,
                        SemanticPointerMetadataV1::None,
                        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                    ),
                ];
                assert_slot_equivalent(
                    &declarations,
                    SemanticTypeIdV1::from_index(1),
                    Some((
                        Type::pointer(Type::Scalar(ScalarType::U32), lowered, access),
                        8,
                    )),
                );
            }
        }
    }
}

#[test]
fn private_array_facts_preserve_direct_scalar_pointee_inhabitedness_order() {
    let zero = SemanticTypeIdV1::from_index(0);
    for validity in [false, true] {
        let mut declarations = vec![fact_scalar(true, validity)];
        // An outer retained scalar rejects uninhabited layout, but the old memory
        // helper's direct Scalar/ValidityScalar probe precedes that layout check.
        assert_slot_equivalent(&declarations, zero, None);
        assert_memory_equivalent(&declarations, zero, Some(ScalarType::U32));
        declarations.push(fact_pointer(
            zero,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            1,
            64,
            SemanticPointerMetadataV1::None,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        ));
        assert_slot_equivalent(
            &declarations,
            SemanticTypeIdV1::from_index(1),
            Some((
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                8,
            )),
        );
        declarations.push(fact_wrapper(401, zero, 4, 4, false));
        assert_memory_equivalent(&declarations, SemanticTypeIdV1::from_index(2), None);
    }
    let declarations = vec![
        fact_scalar(false, false),
        fact_wrapper(402, zero, 4, 4, true),
    ];
    assert_memory_equivalent(&declarations, SemanticTypeIdV1::from_index(1), None);
}

#[test]
fn private_array_facts_preserve_invalid_layout_metadata_and_wrapper_rejections() {
    let zero = SemanticTypeIdV1::from_index(0);
    for (space, width, metadata, size, alignment, uninhabited) in [
        (42, 64, SemanticPointerMetadataV1::None, 8, 8, false),
        (1, 24, SemanticPointerMetadataV1::None, 8, 8, false),
        (1, 64, SemanticPointerMetadataV1::SliceLength, 8, 8, false),
        (1, 64, SemanticPointerMetadataV1::VTable, 8, 8, false),
        (1, 64, SemanticPointerMetadataV1::None, 4, 4, false),
        (1, 64, SemanticPointerMetadataV1::None, 16, 16, false),
        (1, 64, SemanticPointerMetadataV1::None, 8, 8, true),
    ] {
        let declarations = vec![
            fact_scalar(false, false),
            fact_pointer(
                zero,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                space,
                width,
                metadata,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(size),
                    alignment,
                    SemanticBackendReprV1::memory(true),
                    uninhabited,
                )
                .unwrap(),
            ),
        ];
        assert_slot_equivalent(&declarations, SemanticTypeIdV1::from_index(1), None);
    }
    for (field, size, alignment) in [
        (zero, 8, 4),
        (zero, 4, 2),
        (SemanticTypeIdV1::from_index(9), 4, 4),
        (SemanticTypeIdV1::from_index(1), 4, 4),
    ] {
        let declarations = vec![
            fact_scalar(false, false),
            fact_wrapper(403, field, size, alignment, false),
        ];
        assert_memory_equivalent(&declarations, SemanticTypeIdV1::from_index(1), None);
    }
    let declarations = vec![fact_scalar(false, false)];
    assert_slot_equivalent(&declarations, SemanticTypeIdV1::from_index(9), None);
    assert_memory_equivalent(&declarations, SemanticTypeIdV1::from_index(9), None);
}

#[test]
fn private_array_facts_depth_and_work_boundaries_are_source_derived() {
    let scalar = vec![fact_scalar(false, false)];
    let (direct, direct_id) = fact_chain(0);
    let (one, one_id) = fact_chain(1);
    let (deep, deep_id) = fact_chain(255);
    let (too_deep, too_deep_id) = fact_chain(256);
    for (declarations, ty, accepted, full, prefix) in [
        (&scalar, SemanticTypeIdV1::from_index(0), true, 11, 7),
        (&direct, direct_id, true, 16, 12),
        (&one, one_id, true, 32, 28),
        (&deep, deep_id, true, 3588, 3584),
        (&too_deep, too_deep_id, false, 3591, 3588),
    ] {
        let mut exact = PrivateArrayRecorderBudgetV1::new(1, full).unwrap();
        assert_eq!(
            private_retained_slot_facts_v1(declarations, ty, &mut exact)
                .unwrap()
                .is_some(),
            accepted
        );
        assert_eq!(exact.work.work(), full);
        let mut short = PrivateArrayRecorderBudgetV1::new(1, full - 1).unwrap();
        assert!(matches!(
            private_retained_slot_facts_v1(declarations, ty, &mut short),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual,
                limit,
            }) if actual == full && limit == full - 1
        ));
        assert_eq!(short.work.work(), prefix);
        assert!(matches!(
            short.charge_private_array_work(0),
            Err(ProductionSemanticKirErrorV1::ResourceLimit { actual, limit, .. })
                if actual == full && limit == full - 1
        ));
    }
    let pointee = SemanticTypeIdV1::from_index(255);
    assert_memory_equivalent(&deep, pointee, Some(ScalarType::U32));
    assert_slot_equivalent(
        &deep,
        deep_id,
        Some((
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            8,
        )),
    );
    // Wrapper256 fetches and checks its terminal field layout, but there is no
    // loop257 terminal scalar probe. Both old and new helper paths reject.
    assert_memory_equivalent(&too_deep, SemanticTypeIdV1::from_index(256), None);
    assert_slot_equivalent(&too_deep, too_deep_id, None);
}
