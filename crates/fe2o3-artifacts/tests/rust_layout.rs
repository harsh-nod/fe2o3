use fe2o3_artifacts::{
    MAX_RUST_LAYOUT_ALIGNMENT, MAX_RUST_LAYOUT_BYTES, MAX_RUST_LAYOUT_COMPONENTS, PointerWidth,
    RustDisjointIndexSpaceV1, RustLayoutEvidenceError, RustLayoutEvidenceV1,
    RustPhysicalComponentKindV1, RustPhysicalComponentV1, RustPointerMutabilityV1,
    RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1, RustZstRoleV1,
    RustcAbiClassV1,
};

fn pointer(
    offset: u64,
    width: u64,
    alignment: u32,
    mutability: RustPointerMutabilityV1,
    pointee: RustScalarElementTypeV1,
) -> RustPhysicalComponentV1 {
    RustPhysicalComponentV1::new(
        offset,
        width,
        alignment,
        RustPhysicalComponentKindV1::Pointer {
            mutability,
            pointee,
        },
    )
    .unwrap()
}

fn usize_component(offset: u64, width: u64, alignment: u32) -> RustPhysicalComponentV1 {
    RustPhysicalComponentV1::new(offset, width, alignment, RustPhysicalComponentKindV1::Usize)
        .unwrap()
}

fn index_1d_zst(offset: u64) -> RustPhysicalComponentV1 {
    RustPhysicalComponentV1::new(
        offset,
        0,
        1,
        RustPhysicalComponentKindV1::Zst(RustZstRoleV1::DisjointIndexSpace(
            RustDisjointIndexSpaceV1::Index1D,
        )),
    )
    .unwrap()
}

fn shared_slice(
    element: RustScalarElementTypeV1,
    pointer_width: PointerWidth,
) -> RustLayoutEvidenceV1 {
    let width = pointer_width.bytes();
    let alignment = width as u32;
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::shared_slice(element)),
        RustcAbiClassV1::ScalarPair,
        pointer_width,
        width * 2,
        alignment,
        vec![
            pointer(0, width, alignment, RustPointerMutabilityV1::Const, element),
            usize_component(width, width, alignment),
        ],
    )
    .unwrap()
}

fn disjoint_slice(
    element: RustScalarElementTypeV1,
    pointer_width: PointerWidth,
) -> RustLayoutEvidenceV1 {
    let width = pointer_width.bytes();
    let size = width * 2;
    let alignment = width as u32;
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::disjoint_slice(
            element,
            RustDisjointIndexSpaceV1::Index1D,
        )),
        RustcAbiClassV1::Aggregate,
        pointer_width,
        size,
        alignment,
        vec![
            pointer(0, width, alignment, RustPointerMutabilityV1::Mut, element),
            usize_component(width, width, alignment),
            index_1d_zst(size),
        ],
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn exact_vecadd_evidence_has_stable_golden_encodings_and_identities() {
    let shared = shared_slice(RustScalarElementTypeV1::F32, PointerWidth::Bits64);
    let disjoint = disjoint_slice(RustScalarElementTypeV1::F32, PointerWidth::Bits64);

    assert_eq!(
        hex(&shared.rust_type().canonical_bytes()),
        "1c0000004645324f332f525553542d545950452d45564944454e43452f563100010002000000010a"
    );
    assert_eq!(
        hex(&shared.canonical_bytes()),
        concat!(
            "1e0000004645324f332f525553542d4c41594f55542d45564944454e43452f563100010028000000",
            "1c0000004645324f332f525553542d545950452d45564944454e43452f563100010002000000010a",
            "020210000000000000000800000002000000170000000000000000000000080000000000000008",
            "00000001010a15000000080000000000000008000000000000000800000002"
        )
    );
    assert_eq!(
        hex(shared.type_identity().rust_type().bytes().as_bytes()),
        "f83042f005ac664ce0b7db51ac20b97ec0bc7b84973b4ac87fb68d6cda76e2fd"
    );
    assert_eq!(
        hex(shared.type_identity().layout().bytes().as_bytes()),
        "ab5caf6e3317ef750fbb80e27c837da3800541adcf0f324baf59681c23740956"
    );

    assert_eq!(
        hex(&disjoint.rust_type().canonical_bytes()),
        "1c0000004645324f332f525553542d545950452d45564944454e43452f563100010003000000020a01"
    );
    assert_eq!(
        hex(&disjoint.canonical_bytes()),
        concat!(
            "1e0000004645324f332f525553542d4c41594f55542d45564944454e43452f563100010029000000",
            "1c0000004645324f332f525553542d545950452d45564944454e43452f563100010003000000020a",
            "010302100000000000000008000000030000001700000000000000000000000800000000000000",
            "0800000001020a150000000800000000000000080000000000000008000000021700000010000000",
            "00000000000000000000000001000000030101"
        )
    );
    assert_eq!(
        hex(disjoint.type_identity().rust_type().bytes().as_bytes()),
        "703f1c0c127467c3ec189a806662ba37dd153ecd0b81b45c9b6aa8cb0f695ebf"
    );
    assert_eq!(
        hex(disjoint.type_identity().layout().bytes().as_bytes()),
        "ec1831ff672840d58d46ecfddca0f04e3d4058577968bf117bad0a5b9491e654"
    );
}

#[test]
fn construction_is_deterministic_and_exposes_validated_evidence() {
    let first = shared_slice(RustScalarElementTypeV1::F32, PointerWidth::Bits64);
    let second = shared_slice(RustScalarElementTypeV1::F32, PointerWidth::Bits64);

    assert_eq!(first, second);
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.type_identity(), second.type_identity());
    assert_eq!(first.abi_class(), RustcAbiClassV1::ScalarPair);
    assert_eq!(first.pointer_width(), PointerWidth::Bits64);
    assert_eq!(first.size(), 16);
    assert_eq!(first.abi_alignment(), 8);
    assert_eq!(first.components().len(), 2);
    assert_eq!(first.rust_type().source_type().element().size_bytes(), 4);
}

#[test]
fn every_meaningful_valid_mutation_changes_the_appropriate_identity() {
    let shared_64 = shared_slice(RustScalarElementTypeV1::F32, PointerWidth::Bits64);
    let shared_32 = shared_slice(RustScalarElementTypeV1::F32, PointerWidth::Bits32);
    let shared_u32 = shared_slice(RustScalarElementTypeV1::U32, PointerWidth::Bits64);
    let disjoint_64 = disjoint_slice(RustScalarElementTypeV1::F32, PointerWidth::Bits64);

    assert_eq!(
        shared_64.type_identity().rust_type(),
        shared_32.type_identity().rust_type()
    );
    assert_ne!(
        shared_64.type_identity().layout(),
        shared_32.type_identity().layout()
    );
    assert_ne!(shared_64.type_identity(), shared_u32.type_identity());
    assert_ne!(shared_64.type_identity(), disjoint_64.type_identity());
}

#[test]
fn component_constructor_rejects_malformed_values() {
    let pointer_kind = RustPhysicalComponentKindV1::Pointer {
        mutability: RustPointerMutabilityV1::Const,
        pointee: RustScalarElementTypeV1::F32,
    };
    assert!(matches!(
        RustPhysicalComponentV1::new(0, 8, 0, pointer_kind),
        Err(RustLayoutEvidenceError::InvalidAlignment { .. })
    ));
    assert!(matches!(
        RustPhysicalComponentV1::new(0, 8, 3, pointer_kind),
        Err(RustLayoutEvidenceError::InvalidAlignment { .. })
    ));
    assert!(matches!(
        RustPhysicalComponentV1::new(0, 8, MAX_RUST_LAYOUT_ALIGNMENT * 2, pointer_kind),
        Err(RustLayoutEvidenceError::InvalidAlignment { .. })
    ));
    assert!(matches!(
        RustPhysicalComponentV1::new(4, 8, 8, pointer_kind),
        Err(RustLayoutEvidenceError::MisalignedOffset { .. })
    ));
    assert!(matches!(
        RustPhysicalComponentV1::new(0, 0, 8, pointer_kind),
        Err(RustLayoutEvidenceError::InvalidComponent(_))
    ));
    assert!(matches!(
        RustPhysicalComponentV1::new(
            0,
            1,
            1,
            RustPhysicalComponentKindV1::Zst(RustZstRoleV1::DisjointIndexSpace(
                RustDisjointIndexSpaceV1::Index1D,
            )),
        ),
        Err(RustLayoutEvidenceError::InvalidComponent(_))
    ));
    assert!(matches!(
        RustPhysicalComponentV1::new(0, 1, 2, RustPhysicalComponentKindV1::Padding),
        Err(RustLayoutEvidenceError::InvalidComponent(_))
    ));
    assert!(matches!(
        RustPhysicalComponentV1::new(
            MAX_RUST_LAYOUT_BYTES,
            8,
            8,
            RustPhysicalComponentKindV1::Usize,
        ),
        Err(RustLayoutEvidenceError::BoundExceeded { .. })
    ));
}

#[test]
fn layout_constructor_enforces_bounds_and_alignment() {
    let rust_type = RustTypeEvidenceV1::new(RustSourceTypeShapeV1::shared_slice(
        RustScalarElementTypeV1::F32,
    ));
    let valid_components = || {
        vec![
            pointer(
                0,
                8,
                8,
                RustPointerMutabilityV1::Const,
                RustScalarElementTypeV1::F32,
            ),
            usize_component(8, 8, 8),
        ]
    };

    for alignment in [0, 3, MAX_RUST_LAYOUT_ALIGNMENT * 2] {
        assert!(matches!(
            RustLayoutEvidenceV1::new(
                rust_type,
                RustcAbiClassV1::ScalarPair,
                PointerWidth::Bits64,
                16,
                alignment,
                valid_components(),
            ),
            Err(RustLayoutEvidenceError::InvalidAlignment { .. })
        ));
    }
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            rust_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            0,
            8,
            valid_components(),
        ),
        Err(RustLayoutEvidenceError::BoundExceeded { .. })
    ));
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            rust_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            MAX_RUST_LAYOUT_BYTES + 8,
            8,
            valid_components(),
        ),
        Err(RustLayoutEvidenceError::BoundExceeded { .. })
    ));
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            rust_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            15,
            8,
            valid_components(),
        ),
        Err(RustLayoutEvidenceError::InvalidLayout(_))
    ));
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            rust_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            vec![],
        ),
        Err(RustLayoutEvidenceError::EmptyComponents)
    ));

    let too_many = vec![index_1d_zst(0); MAX_RUST_LAYOUT_COMPONENTS + 1];
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            rust_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            too_many,
        ),
        Err(RustLayoutEvidenceError::TooManyComponents { .. })
    ));
}

#[test]
fn layout_constructor_requires_order_non_overlap_and_full_coverage() {
    let rust_type = RustTypeEvidenceV1::new(RustSourceTypeShapeV1::shared_slice(
        RustScalarElementTypeV1::F32,
    ));
    let make = |components| {
        RustLayoutEvidenceV1::new(
            rust_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            components,
        )
    };

    let gap = vec![
        pointer(
            0,
            8,
            8,
            RustPointerMutabilityV1::Const,
            RustScalarElementTypeV1::F32,
        ),
        usize_component(12, 4, 4),
    ];
    assert!(matches!(
        make(gap),
        Err(RustLayoutEvidenceError::InvalidLayout(_))
    ));

    let overlap = vec![
        pointer(
            0,
            8,
            8,
            RustPointerMutabilityV1::Const,
            RustScalarElementTypeV1::F32,
        ),
        usize_component(4, 4, 4),
        RustPhysicalComponentV1::new(8, 8, 1, RustPhysicalComponentKindV1::Padding).unwrap(),
    ];
    assert!(matches!(
        make(overlap),
        Err(RustLayoutEvidenceError::InvalidLayout(_))
    ));

    let incomplete = vec![
        pointer(
            0,
            8,
            8,
            RustPointerMutabilityV1::Const,
            RustScalarElementTypeV1::F32,
        ),
        index_1d_zst(8),
    ];
    assert!(matches!(
        make(incomplete),
        Err(RustLayoutEvidenceError::InvalidLayout(_))
    ));

    let out_of_bounds = vec![
        pointer(
            0,
            8,
            8,
            RustPointerMutabilityV1::Const,
            RustScalarElementTypeV1::F32,
        ),
        usize_component(8, 16, 8),
    ];
    assert!(matches!(
        make(out_of_bounds),
        Err(RustLayoutEvidenceError::InvalidLayout(_))
    ));
}

#[test]
fn semantic_validation_rejects_descriptive_but_inconsistent_evidence() {
    let shared_type = RustTypeEvidenceV1::new(RustSourceTypeShapeV1::shared_slice(
        RustScalarElementTypeV1::F32,
    ));
    let shared_components = || {
        vec![
            pointer(
                0,
                8,
                8,
                RustPointerMutabilityV1::Const,
                RustScalarElementTypeV1::F32,
            ),
            usize_component(8, 8, 8),
        ]
    };
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            shared_type,
            RustcAbiClassV1::Aggregate,
            PointerWidth::Bits64,
            16,
            8,
            shared_components(),
        ),
        Err(RustLayoutEvidenceError::SemanticMismatch(_))
    ));

    let wrong_mutability = vec![
        pointer(
            0,
            8,
            8,
            RustPointerMutabilityV1::Mut,
            RustScalarElementTypeV1::F32,
        ),
        usize_component(8, 8, 8),
    ];
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            shared_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            wrong_mutability,
        ),
        Err(RustLayoutEvidenceError::SemanticMismatch(_))
    ));

    let wrong_pointee = vec![
        pointer(
            0,
            8,
            8,
            RustPointerMutabilityV1::Const,
            RustScalarElementTypeV1::U32,
        ),
        usize_component(8, 8, 8),
    ];
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            shared_type,
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            wrong_pointee,
        ),
        Err(RustLayoutEvidenceError::SemanticMismatch(_))
    ));

    let disjoint_type = RustTypeEvidenceV1::new(RustSourceTypeShapeV1::disjoint_slice(
        RustScalarElementTypeV1::F32,
        RustDisjointIndexSpaceV1::Index1D,
    ));
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            disjoint_type,
            RustcAbiClassV1::Aggregate,
            PointerWidth::Bits64,
            16,
            8,
            vec![
                pointer(
                    0,
                    8,
                    8,
                    RustPointerMutabilityV1::Mut,
                    RustScalarElementTypeV1::F32,
                ),
                usize_component(8, 8, 8),
            ],
        ),
        Err(RustLayoutEvidenceError::SemanticMismatch(_))
    ));

    let misaligned_marker = RustPhysicalComponentV1::new(
        16,
        0,
        2,
        RustPhysicalComponentKindV1::Zst(RustZstRoleV1::DisjointIndexSpace(
            RustDisjointIndexSpaceV1::Index1D,
        )),
    )
    .unwrap();
    assert!(matches!(
        RustLayoutEvidenceV1::new(
            disjoint_type,
            RustcAbiClassV1::Aggregate,
            PointerWidth::Bits64,
            16,
            8,
            vec![
                pointer(
                    0,
                    8,
                    8,
                    RustPointerMutabilityV1::Mut,
                    RustScalarElementTypeV1::F32,
                ),
                usize_component(8, 8, 8),
                misaligned_marker,
            ],
        ),
        Err(RustLayoutEvidenceError::SemanticMismatch(_))
    ));
}
